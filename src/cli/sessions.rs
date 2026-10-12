use super::interactive::*;
use super::*;

/// 一次已解析的 `sessions` 调用。由动词决定读哪些字段。
#[derive(Debug, Default)]
pub(crate) struct SessionsArgs {
    pub(crate) verb: String,
    pub(crate) id: Option<String>,
    /// `sessions title` 的那一段文本：`None` 与空串不同 —— 前者是「没给」，
    /// 后者是「撤回」（决策票 01 §3）。
    pub(crate) title: Option<String>,
    pub(crate) all: bool,
    pub(crate) files: bool,
    pub(crate) only_error: bool,
    pub(crate) json: bool,
    pub(crate) limit: Option<usize>,
    pub(crate) round: Option<u32>,
    pub(crate) speaker: Option<String>,
    pub(crate) kind: Option<String>,
    pub(crate) tool: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) config: Option<PathBuf>,
    pub(crate) cwd: Option<PathBuf>,
}

pub(crate) fn parse_sessions(args: &[String]) -> Result<SessionsArgs, String> {
    let mut parsed = SessionsArgs::default();
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        match arg {
            "--all" => parsed.all = true,
            "--files" => parsed.files = true,
            "--only-error" => parsed.only_error = true,
            "--json" => parsed.json = true,
            "--round" | "--speaker" | "--kind" | "--tool" | "--model" | "--config" | "--cwd"
            | "--limit" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| render::wording::needs_value(arg))?;
                match arg {
                    "--round" => {
                        parsed.round = Some(
                            value
                                .parse()
                                .map_err(|_| render::wording::needs_number("--round", value))?,
                        )
                    }
                    "--speaker" => parsed.speaker = Some(value.clone()),
                    "--kind" => parsed.kind = Some(value.clone()),
                    "--tool" => parsed.tool = Some(value.clone()),
                    "--model" => parsed.model = Some(value.clone()),
                    "--config" => parsed.config = Some(PathBuf::from(value)),
                    "--cwd" => parsed.cwd = Some(PathBuf::from(value)),
                    "--limit" => {
                        parsed.limit = Some(
                            value
                                .parse()
                                .map_err(|_| render::wording::needs_number("--limit", value))?,
                        )
                    }
                    _ => unreachable!(),
                }
            }
            other if other.starts_with('-') => {
                return Err(render::wording::unknown_argument(other));
            }
            other if parsed.verb.is_empty() => parsed.verb = other.to_owned(),
            other if parsed.id.is_none() => parsed.id = Some(other.to_owned()),
            // `sessions title <id> <text>` 是唯一带第二个位置参数的动词。
            other if parsed.verb == "title" && parsed.title.is_none() => {
                parsed.title = Some(other.to_owned())
            }
            other => return Err(render::wording::extra_argument(other)),
        }
        index += 1;
    }
    Ok(parsed)
}

/// `sessions` 子命令一族：`ls`、`show`、`replay`、`stats`。
///
/// 不管哪个动词，stdout 都承载结果、stderr 承载诊断；每个视图都有 `--json` 形式，好让管道能读它。
/// 没有索引：找一个会话靠扫它的桶，而 `--continue` 的「最新在前」就是 `ls` 显示的顺序。
pub fn run_sessions(
    args: &[String],
    env: &EnvMap,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> ExitCode {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_sessions_help(out);
        return ExitCode::SUCCESS;
    }
    let parsed = match parse_sessions(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            let _ = writeln!(err, "heng: {message}");
            print_sessions_help(err);
            return ExitCode::FAILURE;
        }
    };

    let result = match parsed.verb.as_str() {
        "ls" => sessions_ls(&parsed, env, out, err),
        "show" => sessions_show(&parsed, env, out),
        "replay" => sessions_replay(&parsed, env, out),
        "title" => sessions_title(&parsed, env, out),
        "stats" => sessions_stats(&parsed, env, out),
        "" => {
            let _ = writeln!(err, "heng: {}", render::wording::sessions_needs_verb());
            print_sessions_help(err);
            return ExitCode::FAILURE;
        }
        other => {
            let _ = writeln!(
                err,
                "heng: {}",
                render::wording::unknown_sessions_verb(other)
            );
            print_sessions_help(err);
            return ExitCode::FAILURE;
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            let _ = writeln!(err, "heng: {message}");
            ExitCode::FAILURE
        }
    }
}

/// 会话 store，或者「没有可看的地方」这句拒绝。
pub(crate) fn open_store(env: &EnvMap) -> Result<SessionStore, String> {
    let root = config::sessions_dir(env)
        .ok_or_else(|| render::wording::startup_no_session_store().to_owned())?;
    Ok(SessionStore::new(root))
}

/// 先搜哪个工作区的桶：`--cwd`，否则当前目录。
pub(crate) fn search_cwd(parsed: &SessionsArgs) -> Result<PathBuf, String> {
    match &parsed.cwd {
        Some(path) => Ok(path.clone()),
        None => std::env::current_dir()
            .map_err(|error| render::wording::startup_cwd(&error.to_string())),
    }
}

/// 这一趟打开的那个会话，以及它落在哪里。
#[derive(Debug)]
pub(crate) struct SessionChoice {
    pub(crate) stored: StoredSession,
    /// 这一趟的工作目录。
    pub(crate) cwd: PathBuf,
    /// 那个会话在**别的工作区**里（工作目录因此跟着它走）—— 前端要说一句。
    pub(crate) elsewhere: bool,
}

/// 这一趟打开哪个会话，以及它落在哪个工作目录里。
///
/// `-c` 的三种含义都在这里：不带 `resume` 是新开一个会话；`Latest` 是这个工作区最近写下的那个会话；
/// `Session(id)` 是**指名**续哪个会话 —— 先在本桶找，找不到就全 store 找（id 全局唯一，
/// `sessions show <id>` 一直是这么做的）。
///
/// 按 id 续上别处的那个会话时，工作目录取**那个会话自己的**（它流里 `SessionStarted` 记的那个），
/// 而不是命令行上那个：续写要发生在它自己的目录里，否则工具的相对路径、前缀缓存与横幅全不对。
/// 读不出 cwd（流坏了）就退回请求的那个。
pub(crate) fn choose_session(
    store: &SessionStore,
    cwd: &Path,
    resume: Option<&Resume>,
) -> Result<SessionChoice, String> {
    let fresh = |stored: StoredSession| SessionChoice {
        stored,
        cwd: cwd.to_path_buf(),
        elsewhere: false,
    };
    match resume {
        None => match store.create(cwd) {
            Ok(stored) => Ok(fresh(stored)),
            Err(error) => Err(render::wording::startup_store_create(&error.to_string())),
        },
        Some(Resume::Latest) => match store.latest(cwd) {
            Ok(Some(stored)) => Ok(fresh(stored)),
            Ok(None) => Err(render::wording::startup_no_session_to_continue(
                &cwd.display().to_string(),
            )),
            Err(error) => Err(render::wording::startup_store_read(&error.to_string())),
        },
        Some(Resume::Session(id)) => {
            let stored = find_session(store, cwd, id)?;
            // 它在不在**请求的那个工作区**里：在就沿用请求的目录。读不出桶就当它在本桶 —— 保守：
            // 宁可少换一次目录，也不凭一次读失败去改这一趟的工作目录。
            let here = store.is_in_bucket(cwd, &stored.id).unwrap_or(true);
            // 续写要发生在它自己的目录里。读不出它自己的 cwd（流坏了）就老实退回请求的那个，而且
            // **不说**那句「跟着走了」—— 那时它说的会是假话。
            let own = if here {
                None
            } else {
                crate::session::store::session_cwd(&stored)
            };
            Ok(SessionChoice {
                cwd: own
                    .as_deref()
                    .map_or_else(|| cwd.to_path_buf(), PathBuf::from),
                elsewhere: own.is_some(),
                stored,
            })
        }
    }
}

/// 按 id（或按它目录的路径）找一个会话。
pub(crate) fn find_session(
    store: &SessionStore,
    cwd: &Path,
    id: &str,
) -> Result<StoredSession, String> {
    let direct = PathBuf::from(id);
    if direct.join(crate::session::store::LOG_FILE).is_file() {
        return Ok(session_at(direct));
    }
    // id 是全局唯一的，但拥有它的那个桶才是快路径；全 store 扫一遍是让 `sessions show` 在任何目录
    // 下都能用的东西。
    let mut candidates = store.list(cwd).unwrap_or_default();
    candidates.extend(store.list_all().unwrap_or_default());
    candidates
        .into_iter()
        .find(|session| session.id.as_str() == id)
        .ok_or_else(|| render::wording::no_session(id, &store_list_label(cwd)))
}

pub(crate) fn store_list_label(cwd: &Path) -> String {
    render::wording::session_search_label(&cwd.display().to_string())
}

pub(crate) fn session_at(dir: PathBuf) -> StoredSession {
    let id = dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown");
    StoredSession {
        id: SessionId::new(id),
        outputs_dir: dir.join(crate::session::store::OUTPUTS_DIR),
        log_path: dir.join(crate::session::store::LOG_FILE),
        dir,
    }
}

/// 解析 `--speaker`：`user`、`system`、`executor:<id>`，或某位讨论者的名字。
pub(crate) fn parse_speaker(raw: &str) -> SpeakerId {
    match raw {
        "user" => SpeakerId::User,
        "system" => SpeakerId::System,
        other => match other.strip_prefix("executor:") {
            Some(id) => SpeakerId::Executor(id.into()),
            None => SpeakerId::Debater(other.into()),
        },
    }
}

pub(crate) fn sessions_ls(
    parsed: &SessionsArgs,
    env: &EnvMap,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), String> {
    let store = open_store(env)?;
    let cwd = search_cwd(parsed)?;
    let listings = observe::list(&store, (!parsed.all).then_some(cwd.as_path()))
        .map_err(|error| render::wording::startup_store_read(&error.to_string()))?;
    let mut listings: Vec<Listing> = listings;
    if let Some(limit) = parsed.limit {
        listings.truncate(limit);
    }

    if parsed.json {
        return write_json(out, &listings);
    }
    if listings.is_empty() {
        let _ = writeln!(err, "heng: {}", render::wording::no_sessions());
        return Ok(());
    }
    // 表头是按**显示列宽**补齐的：一个中文列名按字符数算比按列宽算要窄，所以 `{:<36}` 会让它相对
    // 它标注的那一行左移两列。
    let columns = render::wording::ls_columns();
    let _ = writeln!(
        out,
        "{}  {}  {}  {}  {}  {}  {}",
        pad_end(columns[0], 36),
        pad_end(columns[1], 28),
        pad_end(columns[2], 20),
        pad_start(columns[3], 10),
        pad_start(columns[4], 6),
        pad_start(columns[5], 5),
        columns[6],
    );
    for listing in listings {
        let _ = writeln!(
            out,
            "{:<36}  {:<28}  {:<20}  {:>10}  {:>6}  {:>5}  {}",
            listing.id,
            listing.cwd.as_deref().unwrap_or("-"),
            listing
                .started
                .map(|at| at.format("%Y-%m-%d %H:%M:%SZ").to_string())
                .unwrap_or_else(|| "-".to_owned()),
            listing.tokens,
            listing.rounds,
            listing.messages,
            listing
                .ended
                .map(render::wording::stop_reason)
                .unwrap_or_else(render::wording::open_session),
        );
    }
    Ok(())
}

/// 把 `text` 右侧补齐到 `width` 个**显示列**。
pub(crate) fn pad_end(text: &str, width: usize) -> String {
    let used = text.cell_width() as usize;
    if used >= width {
        return text.to_owned();
    }
    format!("{text}{}", " ".repeat(width - used))
}

/// 把 `text` 左侧补齐到 `width` 个**显示列**。
pub(crate) fn pad_start(text: &str, width: usize) -> String {
    let used = text.cell_width() as usize;
    if used >= width {
        return text.to_owned();
    }
    format!("{}{text}", " ".repeat(width - used))
}

/// `sessions title <id> <text>`：给一个会话改名。
///
/// 写下的是**一条新事件**（`SessionTitled`），所以改名可重放、也能被 `-c` 带上 —— 标题不是
/// 界面里的一份临时状态（`.scratch/session-tab/spec.md` §4）。空串（或归一化之后为空）就是
/// **撤回**：那一条照样要写，否则最后一条手动标题仍然生效（决策票 01 §3）。
///
/// **已知限制**：别在一个会话正跑着时用 CLI 改它的名。`EventLog::append` 的 `next_seq` 是打开
/// 时按 `events.len() + 1` 算的，两个进程各持自己的那个数就会写出重复 `seq`。本图不为改名
/// 引入文件锁（决策票 01 §9）。
pub(crate) fn sessions_title(
    parsed: &SessionsArgs,
    env: &EnvMap,
    out: &mut dyn Write,
) -> Result<(), String> {
    let store = open_store(env)?;
    let cwd = search_cwd(parsed)?;
    let id = parsed
        .id
        .as_deref()
        .ok_or_else(render::wording::title_needs_id)?;
    let raw = parsed
        .title
        .as_deref()
        .ok_or_else(render::wording::title_needs_text)?;
    let session = find_session(&store, &cwd, id)?;
    let title = titles::normalize(raw).unwrap_or_default();
    let mut log = EventLog::open(&session.log_path).map_err(|error| {
        render::wording::cannot_read(&session.log_path.display().to_string(), &error.to_string())
    })?;
    log.append(
        SpeakerId::User,
        EventPayload::SessionTitled {
            title: title.clone(),
        },
    )
    .map_err(|error| {
        render::wording::cannot_read(&session.log_path.display().to_string(), &error.to_string())
    })?;
    let _ = writeln!(out, "{}", render::wording::title_changed(id, &title));
    Ok(())
}

pub(crate) fn sessions_show(
    parsed: &SessionsArgs,
    env: &EnvMap,
    out: &mut dyn Write,
) -> Result<(), String> {
    let store = open_store(env)?;
    let cwd = search_cwd(parsed)?;
    let id = parsed
        .id
        .as_deref()
        .ok_or_else(render::wording::show_needs_id)?;
    let session = find_session(&store, &cwd, id)?;
    let events = read_events(&session.log_path).map_err(|error| {
        render::wording::cannot_read(&session.log_path.display().to_string(), &error.to_string())
    })?;

    if parsed.files {
        let changes = filter_changes(observe::file_history(&events), parsed);
        if parsed.json {
            return write_json(out, &changes);
        }
        for change in changes {
            let _ = writeln!(
                out,
                "{:<18}  {:<14}  {}",
                change
                    .round
                    .map(render::wording::round_label)
                    .unwrap_or_else(|| render::wording::session_group().to_owned()),
                change.speaker,
                change.path,
            );
        }
        return Ok(());
    }

    let filter = Filter {
        round: parsed.round,
        speaker: parsed.speaker.as_deref().map(parse_speaker),
        kind: parsed.kind.clone(),
        tool: parsed.tool.clone(),
        only_error: parsed.only_error,
    };
    let timeline = observe::timeline(&events);
    let timeline = if filter.is_empty() {
        timeline
    } else {
        timeline.filtered(&filter)
    };
    if parsed.json {
        return write_json(out, &timeline);
    }
    print_timeline(out, &timeline, &filter);
    Ok(())
}

/// `--files` 认 `--round`、`--speaker` 与 `--tool`；按行形状来的过滤器（`--kind`、`--only-error`）
/// 对一行文件改动没有意义。
pub(crate) fn filter_changes(
    changes: Vec<observe::FileChange>,
    parsed: &SessionsArgs,
) -> Vec<observe::FileChange> {
    let speaker = parsed.speaker.as_deref().map(parse_speaker);
    changes
        .into_iter()
        .filter(|change| {
            parsed.round.is_none_or(|round| change.round == Some(round))
                && speaker
                    .as_ref()
                    .is_none_or(|speaker| &change.speaker == speaker)
                && parsed.tool.as_ref().is_none_or(|tool| &change.tool == tool)
        })
        .collect()
}

pub(crate) fn sessions_replay(
    parsed: &SessionsArgs,
    env: &EnvMap,
    out: &mut dyn Write,
) -> Result<(), String> {
    let store = open_store(env)?;
    let cwd = search_cwd(parsed)?;
    let id = parsed
        .id
        .as_deref()
        .ok_or_else(render::wording::replay_needs_id)?;
    let speaker = parsed
        .speaker
        .as_deref()
        .ok_or_else(render::wording::replay_needs_speaker)?;
    let session = find_session(&store, &cwd, id)?;
    let events = read_events(&session.log_path).map_err(|error| {
        render::wording::cannot_read(&session.log_path.display().to_string(), &error.to_string())
    })?;

    let config = load_config(parsed.config.clone(), env)?;
    let model = parsed
        .model
        .clone()
        .unwrap_or_else(|| config.default_model.clone());
    let speaker = parse_speaker(speaker);
    // 能力表跟着路由走，与那次真实调用完全一样：被路由的合成器或执行者可能用另一个模型作答，而它的
    // 能力表事实不同，拿讨论者的能力表去重现那条请求会是一个无声的谎言（spec §17、§18）。
    let routing = CostModel::new(&model, config.pricing.clone()).with_routing(&config.routing);
    let caps = caps_for(routing.model_for(&speaker)).map_err(|error| error.to_string())?;

    let messages = replay::replay(&events, &speaker, parsed.round, &caps)
        .map_err(|error| error.to_string())?;
    if parsed.json {
        return write_json(out, &messages);
    }
    print_messages(out, &messages);
    Ok(())
}

pub(crate) fn sessions_stats(
    parsed: &SessionsArgs,
    env: &EnvMap,
    out: &mut dyn Write,
) -> Result<(), String> {
    let store = open_store(env)?;
    let cwd = search_cwd(parsed)?;
    let id = parsed
        .id
        .as_deref()
        .ok_or_else(render::wording::stats_needs_id)?;
    let session = find_session(&store, &cwd, id)?;
    let events = read_events(&session.log_path).map_err(|error| {
        render::wording::cannot_read(&session.log_path.display().to_string(), &error.to_string())
    })?;

    // 钱需要一个模型，而流不携带模型，所以模型在这里点名：`--model`，否则配置里的默认值。给人看的
    // 视图会说它按哪个模型计了价，所以这个数字永远不会被无声地归到某个模型上。
    let config = load_config(parsed.config.clone(), env)?;
    let model = parsed
        .model
        .clone()
        .unwrap_or_else(|| config.default_model.clone());
    let cost = CostModel::new(model.clone(), config.pricing.clone()).with_routing(&config.routing);
    let stats = observe::stats(&events, Some(&cost));

    if parsed.json {
        return write_json(out, &stats);
    }
    print_stats(out, &stats, &model);
    Ok(())
}

pub(crate) fn write_json<T: serde::Serialize>(
    out: &mut dyn Write,
    value: &T,
) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    writeln!(out, "{text}").map_err(|error| error.to_string())
}

pub(crate) fn print_timeline(out: &mut dyn Write, timeline: &Timeline, filter: &Filter) {
    // 用量行是统计视图的素材；默认的转录跳过它们，除非有人点名要。
    let kind = filter.kind.as_deref().map(observe::canonical_kind);
    let show_usage = kind.as_deref() == Some("usagerecorded");
    for group in &timeline.groups {
        match group.round {
            Some(round) => {
                let _ = writeln!(out, "\n{}", render::wording::round_group(round, group.mode));
            }
            None => {
                let _ = writeln!(out, "\n{}", render::wording::session_group());
            }
        }
        for entry in &group.entries {
            if matches!(entry, Entry::RoundStarted { .. }) {
                continue;
            }
            if matches!(entry, Entry::Usage { .. }) && !show_usage {
                continue;
            }
            let _ = writeln!(out, "{}", render_entry(entry));
        }
    }
}

pub(crate) fn render_entry(entry: &Entry) -> String {
    let label = |speaker: &SpeakerId| render::wording::speaker_label(speaker);
    match entry {
        Entry::Message { speaker, text, .. } => format!("{} {text}", label(speaker)),
        Entry::Tool {
            speaker,
            tool,
            args,
            ok,
            output,
            error,
            hook,
            ..
        } => {
            let mut lines = vec![format!(
                "{} → {}",
                label(speaker),
                render::wording::tool_call(tool, &args.to_string())
            )];
            match (ok, output, error) {
                (Some(true), Some(output), _) => {
                    lines.push(indent(
                        &render::wording::tool_output_preview(output, PREVIEW),
                        2,
                    ));
                }
                (Some(false), _, error) => {
                    let message = error
                        .clone()
                        .unwrap_or_else(|| render::wording::no_message().to_owned());
                    lines.push(indent(&render::wording::tool_error(&message), 2));
                }
                _ => lines.push(indent(render::wording::no_tool_result(), 2)),
            }
            if let Some(hook) = hook {
                lines.push(indent(&render::wording::hook_feedback(hook), 2));
            }
            lines.join("\n")
        }
        Entry::RoundStarted { round, mode } => render::wording::round_section(*round, *mode),
        Entry::RoundEnded { round, reason } => render::wording::round_ended(*round, *reason),
        Entry::SessionEnded { reason } => render::wording::session_ended(*reason),
        Entry::TurnStarted { speaker, iteration } => {
            format!(
                "{} {}",
                label(speaker),
                render::wording::turn_started(*iteration)
            )
        }
        Entry::TurnEnded { speaker, reason } => {
            format!(
                "{} {}",
                label(speaker),
                render::wording::turn_ended(*reason)
            )
        }
        Entry::Divergence {
            topic, positions, ..
        } => {
            let mut lines = vec![format!("!! {}", render::wording::divergence(topic))];
            for position in positions {
                lines.push(indent(&format!("- {position}"), 2));
            }
            lines.join("\n")
        }
        Entry::PermissionAsked {
            speaker, request, ..
        } => format!(
            "{} {}",
            label(speaker),
            render::wording::permission_asked(
                crate::events::permission_format::tool_name(request),
                &render::transcript::summarize_permission_target(request)
            )
        ),
        Entry::PermissionDecided {
            speaker,
            decision,
            source,
            reason,
            ..
        } => format!(
            "{} {}",
            label(speaker),
            render::wording::permission_decided(*decision, *source, reason.as_deref())
        ),
        Entry::Hook {
            speaker,
            point,
            outcome,
            ..
        } => format!(
            "{} {}",
            label(speaker),
            render::wording::hook(point, outcome)
        ),
        Entry::ExecutorSpawned {
            speaker,
            executor_id,
            ..
        } => format!(
            "{} {}",
            label(speaker),
            render::wording::executor_spawned(executor_id.as_str())
        ),
        Entry::ExecutorFinished {
            executor_id,
            reason,
            summary,
            ..
        } => render::wording::executor_finished(executor_id.as_str(), *reason, summary),
        Entry::Usage { speaker, usage } => {
            format!(
                "{} {}",
                label(speaker),
                render::wording::usage_summary(usage)
            )
        }
        Entry::AgentError {
            speaker, message, ..
        } => format!(
            "{} {}",
            label(speaker),
            render::wording::agent_error(message)
        ),
        Entry::SessionError { code, detail, .. } => render::wording::session_error(code, detail),
        Entry::History {
            reason, summary, ..
        } => render::wording::history(*reason, summary.as_deref()),
        Entry::Context { source, .. } => render::wording::context_injected(source.clone()),
        Entry::Sandbox {
            mode,
            unavailable_reason,
        } => render::wording::sandbox(mode, unavailable_reason.as_deref()),
        Entry::Command { text } => render::wording::command_run(text),
    }
}

pub(crate) fn indent(text: &str, spaces: usize) -> String {
    let pad = " ".repeat(spaces);
    text.lines()
        .map(|line| format!("{pad}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `sessions show` 在略去之前打印一个工具结果的多大部分。
pub(crate) const PREVIEW: usize = 500;

pub(crate) fn print_messages(out: &mut dyn Write, messages: &[Message]) {
    for message in messages {
        match message {
            Message::System { content, .. } => {
                let _ = writeln!(out, "{}\n{content}\n", render::wording::replay_system());
            }
            Message::User { content, name, .. } => {
                let label = render::wording::replay_user(name.as_deref());
                let _ = writeln!(out, "{label}\n{content}\n");
            }
            Message::Assistant {
                content,
                tool_calls,
                ..
            } => {
                let _ = writeln!(out, "{}", render::wording::replay_assistant());
                if let Some(content) = content {
                    let _ = writeln!(out, "{content}");
                }
                for call in tool_calls {
                    let _ = writeln!(
                        out,
                        "{}",
                        render::wording::replay_tool_call(&call.name, &call.arguments)
                    );
                }
                let _ = writeln!(out);
            }
            Message::Tool {
                tool_call_id,
                content,
            } => {
                let _ = writeln!(
                    out,
                    "{}\n{content}\n",
                    render::wording::replay_tool(tool_call_id)
                );
            }
        }
    }
}

pub(crate) fn print_stats(out: &mut dyn Write, stats: &observe::Stats, model: &str) {
    let cost = match stats.session.cost {
        Some(cost) => render::wording::stats_cost(cost, model),
        None => render::wording::stats_no_cost(model),
    };
    let _ = writeln!(
        out,
        "{}",
        render::wording::stats_session(
            stats.session.tokens.total_tokens(),
            stats.session.calls,
            stats.session.messages,
            stats.session.rounds,
            &cost,
        )
    );
    for speaker in &stats.speakers {
        let hit = speaker
            .hit_rate
            .map(|rate| format!("{:.0}%", rate * 100.0))
            .unwrap_or_else(|| "-".to_owned());
        let cost = speaker
            .cost
            .map(|cost| format!("  ${cost:.6}"))
            .unwrap_or_default();
        let _ = writeln!(
            out,
            "{}",
            render::wording::stats_speaker(
                &pad_end(&speaker.speaker.to_string(), 16),
                speaker.tokens.total_tokens(),
                speaker.calls,
                &hit,
                &cost,
            )
        );
    }
    let rate = stats
        .absence
        .rate
        .map(|rate| format!(" ({:.0}%)", rate * 100.0))
        .unwrap_or_default();
    let _ = writeln!(
        out,
        "{}",
        render::wording::stats_absence(stats.absence.one_sided, stats.absence.rounds, &rate)
    );
    for agent in &stats.absence.per_speaker {
        let _ = writeln!(
            out,
            "{}",
            render::wording::stats_absent(&agent.name, agent.count)
        );
    }
    let _ = writeln!(
        out,
        "{}",
        render::wording::stats_edits(stats.edits.succeeded, stats.edits.failed_matches)
    );
    for level in &stats.edits.levels {
        let _ = writeln!(
            out,
            "{}",
            render::wording::stats_match_level(&level.name, level.count)
        );
    }
    let _ = writeln!(
        out,
        "{}",
        render::wording::stats_guards(
            stats.guards.read_before_write,
            stats.guards.invalidated_reads
        )
    );
    let _ = writeln!(
        out,
        "{}",
        render::wording::stats_executors(stats.executors.spawned, stats.executors.finished)
    );
    for reason in &stats.executors.by_reason {
        let _ = writeln!(
            out,
            "{}",
            render::wording::stats_executor_reason(
                render::wording::stop_reason_name(&reason.name),
                reason.count
            )
        );
    }
    let _ = writeln!(
        out,
        "{}",
        render::wording::stats_hooks(
            stats.hooks.executed,
            stats.hooks.pre,
            stats.hooks.post,
            stats.hooks.feedback,
            stats.hooks.failed
        )
    );
    let decisions = stats
        .permissions
        .decided
        .iter()
        .map(|count| {
            render::wording::stats_decision(
                count.count,
                render::wording::decision_name(&count.name),
            )
        })
        .collect::<Vec<_>>()
        .join("、");
    let decided = if decisions.is_empty() {
        String::new()
    } else {
        format!("，已裁决 {decisions}")
    };
    let _ = writeln!(
        out,
        "{}",
        render::wording::stats_permissions(stats.permissions.asked, &decided)
    );
    let rate = stats
        .divergence_rate
        .map(|rate| format!(" ({:.0}%)", rate * 100.0))
        .unwrap_or_default();
    let rounds = stats
        .rounds
        .iter()
        .map(|round| {
            let ended = round
                .ended
                .map(render::wording::stats_round_ended)
                .unwrap_or_default();
            render::wording::stats_round(round.round, round.mode, round.calls, &ended)
        })
        .collect::<Vec<_>>()
        .join("; ");
    let head = format!(
        "{}{}",
        render::wording::stats_divergences(stats.divergences, stats.absence.rounds, &rate),
        render::wording::stats_rounds_prefix()
    );
    let _ = writeln!(out, "{head}{rounds}");
    for stop in &stats.stops {
        let _ = writeln!(
            out,
            "{}",
            render::wording::stats_stop(render::wording::stop_reason_name(&stop.name), stop.count)
        );
    }
}

pub(crate) fn print_sessions_help(out: &mut dyn Write) {
    let _ = writeln!(out, "{}", render::wording::help_sessions());
}
