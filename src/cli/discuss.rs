use super::interactive::*;
use super::probe::*;
use super::*;

/// 一次已解析的 `heng discuss` 调用。
///
/// 刻意没有 `--model`：谁参与讨论是一件配置事实（`[discussion] debaters`），而一个能替换掉两者之
/// 一的旗标会是说同一件事的第二种方式。剩下可选的只是这次讨论怎么看。
#[derive(Debug, Default)]
pub(crate) struct DiscussArgs {
    /// 那个问题，用它被给进来时的那些词。空表示「问 stdin」。
    pub(crate) words: Vec<String>,
    /// `--debaters a,b`：池子里哪两位参与讨论。`None` 表示抽一对。
    pub(crate) debaters: Option<[String; 2]>,
    pub(crate) plain: bool,
    pub(crate) tui: bool,
    pub(crate) config: Option<PathBuf>,
    pub(crate) cwd: Option<PathBuf>,
}

pub(crate) fn parse_discuss(args: &[String]) -> Result<DiscussArgs, String> {
    let mut parsed = DiscussArgs::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--plain" => parsed.plain = true,
            "--tui" => parsed.tui = true,
            flag @ ("--config" | "--cwd" | "--debaters") => {
                let flag = flag.to_owned();
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| render::wording::needs_value(&flag))?;
                match flag.as_str() {
                    "--config" => parsed.config = Some(PathBuf::from(value)),
                    "--cwd" => parsed.cwd = Some(PathBuf::from(value)),
                    "--debaters" => parsed.debaters = Some(split_names(value)?),
                    _ => unreachable!(),
                }
            }
            // `--` 之后的一切都是问题，所以一个问题可以以横线开头。
            "--" => {
                parsed.words.extend(args[index + 1..].iter().cloned());
                break;
            }
            other if other.starts_with("--") => {
                return Err(render::wording::unknown_argument(other));
            }
            other => parsed.words.push(other.to_owned()),
        }
        index += 1;
    }
    if parsed.plain && parsed.tui {
        return Err(render::wording::renderers_mutually_exclusive().to_owned());
    }
    Ok(parsed)
}

/// 一个问题、两位讨论者、一个合成器、一个会话。
///
/// 渲染器的选法与交互式那条路完全一样，所以在终端里看一次讨论得到的就是 TUI —— 包括权限模态框，供
/// 讨论者想跑的任何东西 —— 而管道接进来的拿到 plain 转录，stdout 上只有合成器的产出。
pub(crate) async fn discuss(args: &[String], env: &EnvMap) -> ExitCode {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", render::wording::help_discuss());
        return ExitCode::SUCCESS;
    }
    let parsed = match parse_discuss(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("heng: {message}");
            println!("{}", render::wording::help_discuss());
            return ExitCode::FAILURE;
        }
    };
    let mut config = match load_config(parsed.config.clone(), env) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("heng: {}", render::wording::startup_config(&message));
            return ExitCode::FAILURE;
        }
    };
    // 没登记的模型是启动错误，绝不是悄悄降级。
    if let Err(message) = validate_models(&config) {
        eprintln!("heng: {message}");
        return ExitCode::FAILURE;
    }
    // 文件里没有名册不算错误 —— 大多数配置都是给单 agent 会话用的 —— 但对这个子命令算。
    let Some(roster) = config.discussion.clone() else {
        eprintln!("heng: {}", render::wording::discussion_no_roster());
        return ExitCode::FAILURE;
    };
    // 池子里哪两位参与讨论：命令行点了名的，或者抽出来的。
    let pair = match pick_debaters(&roster, parsed.debaters.clone(), discussion_seed()) {
        Ok(pair) => pair,
        Err(message) => {
            eprintln!("heng: {message}");
            return ExitCode::FAILURE;
        }
    };
    for line in advisory_lines(&config, &pair) {
        eprintln!("heng: {line}");
    }

    // 问题在任何东西被组装之前就要拿到：读它可能在终端上阻塞，而那时终端必须还在 cooked 模式下
    // （组装会把 TUI 切进 raw 模式，并从那一刻起占住屏幕）。
    let Some(question) = question(&parsed.words) else {
        eprintln!("heng: {}", render::wording::discuss_needs_question());
        return ExitCode::FAILURE;
    };

    let Some(root) = config::sessions_dir(env) else {
        eprintln!("heng: {}", render::wording::startup_no_session_store());
        return ExitCode::FAILURE;
    };
    let cwd = match parsed.cwd.clone() {
        Some(cwd) => cwd,
        None => match std::env::current_dir() {
            Ok(dir) => dir,
            Err(error) => {
                eprintln!("heng: {}", render::wording::startup_cwd(&error.to_string()));
                return ExitCode::FAILURE;
            }
        },
    };
    // 项目级的 MCP 配置跟着这一趟的工作目录走（spec §5）。
    if let Err(error) = config.load_project_mcp(&cwd) {
        eprintln!(
            "heng: {}",
            render::wording::startup_config(&error.to_string())
        );
        return ExitCode::FAILURE;
    }
    let stored = match SessionStore::new(root).create(&cwd) {
        Ok(session) => session,
        Err(error) => {
            eprintln!(
                "heng: {}",
                render::wording::startup_store_create(&error.to_string())
            );
            return ExitCode::FAILURE;
        }
    };
    let home = env
        .get("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);

    // 每位参与者的 provider 都在渲染器启动之前建好，所以少一个密钥是在一块正常屏幕上的启动错误，
    // 而不是一个画了一半的界面。
    let (debaters, synthesizer) = match discussion_participants(&config, &pair) {
        Ok(parts) => parts,
        Err(message) => {
            eprintln!("heng: {message}");
            return ExitCode::FAILURE;
        }
    };
    eprintln!(
        "heng: {}",
        render::wording::discussion_starting(
            &render::wording::debater_label(&pair[0].name, &pair[0].model),
            &render::wording::debater_label(&pair[1].name, &pair[1].model),
            &question,
        )
    );

    let (console, port, mut events) = render::console();
    // 同一个键盘也回答讨论者的权限询问：一次讨论仍然是一个带着工具的会话。
    let asker = Arc::new(ConsoleAsker::from_handle(&console));
    // 讨论者是主会话而不是执行者，所以它也可以问用户（spec §7）；这个端口与权限门用的是同一个键
    // 盘。
    let questions: Option<Arc<dyn UserQuestions>> =
        Some(Arc::new(ConsoleQuestions::from_handle(&console)));

    // 讨论者与任何会话一样拿得到四个 MCP 元工具（spec §9）。
    let mcp = mcp_service(&config, &cwd, env, home.as_deref(), questions.clone()).await;

    // 工具表在**渲染器之前**建出来，理由与单 agent 那条路同：详情覆盖层的 Schema 面要读它那份
    // 声明，而它随会话事实在组装时注入一次（`SessionFacts::tool_schemas`）。讨论者拿的就是这一张。
    let tools = tools::with_mcp(
        tools::with_web(
            tools::with_dynamic(&config.tools, questions.is_some()),
            web_service(&config),
        ),
        mcp,
    );
    let facts_tool_schemas = tool_schemas(&tools);

    let use_tui = parsed.tui || (!parsed.plain && std::io::stdout().is_terminal());
    let renderer = if use_tui {
        let caps = match caps_for(&pair[0].model) {
            Ok(caps) => caps,
            Err(error) => {
                eprintln!("heng: {error}");
                return ExitCode::FAILURE;
            }
        };
        let facts = SessionFacts {
            // 讨论 CLI 这条路径上没有可换的模型：`model` 那一格是两个模型的**拼法**，那是名册里
            // 的配置事实，不是一个活会话能中途改的值（`.scratch/model-switching/spec.md` §12）。
            // 点它只给一句说明，`ctrl-t` 与 `/model` 同样回这一句。
            switchable: false,
            session_id: stored.id.as_str().to_owned(),
            session_dir: stored.dir.display().to_string(),
            // 面板只有一行模型和一行上下文，而一次讨论有一对讨论者、没有单一窗口：这一行列出**两个
            // 模型**，窗口是第一位讨论者的 —— 面板也只能是这么个近似（spec §8）。名字是转录的事
            // （`debater_label`）；这一行讲的是模型，所以它显示模型。
            model: render::wording::discussion_pair(&pair[0].model, &pair[1].model),
            context_window: crate::context::usable_input(&caps),
            // `--mode` 属于交互式那条路；讨论读文件。
            mode: config.mode,
            // `session_config` 原样拷贝 `[budget]`，所以文件里的值就是这个会话的值。
            budget_limit: config.budget.limit,
            number_style: config.number_style,
            file_viewer: config.file_viewer,
            diff_viewer: config.diff_viewer.clone(),
            // 每一份工具声明 —— 详情覆盖层的 Schema 面按名字查它（票 24 收口补的那一面）。
            tool_schemas: facts_tool_schemas,
            // 这一对，按名册顺序 —— 与 `pick_pair` 产出的顺序相同，正是它把第一个调色板槽位给了第
            // 一位讨论者（票 07 §1）。
            speaker_order: vec![pair[0].name.clone(), pair[1].name.clone()],
        };
        // 一次讨论是一个问题、一个 harness：它从不是一次重新打开，所以没有历史要重放，TUI 从第一
        // 帧起就在画。
        Renderer::tui(TuiOptions {
            port,
            facts,
            cwd: cwd.clone(),
            reopened: false,
            // `auto` 才读环境（spec 45 / 票 27）；`always` / `never` 是显式覆盖，不碰 env。
            sidebar: if config.sidebar == crate::config::SidebarMode::Auto {
                config.sidebar.initial_wanted(crate::herdr::in_herdr(env))
            } else {
                config.sidebar.initial_wanted(false)
            },
            tile_limit: config.tile_limit,
        })
    } else {
        render::spawn_plain_console(port);
        Renderer::plain(PlainOptions {
            sinks: RenderSinks {
                stdout_result: Box::new(std::io::stdout()),
                stderr_diagnostic: Box::new(std::io::stderr()),
            },
            color: std::io::stderr().is_terminal() && env.get("NO_COLOR").is_none(),
        })
    };
    // 这个前端从不读一行 —— 一个问题进去、一次讨论出来 —— 所以它一辈子都跑在一次 run 里面，并在
    // 循环还不会开口问之前就说了这一点。在这里告诉它而不是让它推断，理由与
    // `ConsoleRequest::RunState` 记下的那条相同。
    console.set_running(true);

    // 与单 agent 那条同一条纪律：渲染器是进程级的（票 13）。
    // 进程级的一张锁表：同一入口里开出的每一个会话都从它取自己 cwd 的那一张 —— 同 cwd
    // 共享一把（写互斥真的成立），异 cwd 各一把（不互相阻塞）（票 13）。取锁表要在
    // `cwd` 被移进脚手架**之前**，所以这里就把两件事合成一句。
    let path_locks = crate::tools::PathLockTable::new().locks(&cwd);
    let channel = crate::render::RenderChannel::start(renderer, stored.id.clone());
    let mut harness = match assemble_discussion(DiscussionParts {
        scaffold: SessionScaffold {
            cwd,
            log_path: stored.log_path.clone(),
            session_id: stored.id.clone(),
            // 工具表在上面、渲染器之前就建好了，就是这一张。
            tools,
            locks: path_locks,
            // 文件里的模式：讨论者与任何会话一样走同一个权限门，而名册共享一个策略（spec §12、
            // §15）。
            policy: Policy::for_mode(config.mode).with_outside_read(config.outside_read),
            asker: Some(asker),
            questions,
            hook: None,
            home,
        },
        debaters,
        synthesizer,
        max_rounds: roster.max_rounds,
        render: channel,
    })
    .await
    {
        Ok(harness) => harness,
        Err(error) => {
            eprintln!("heng: {error}");
            return ExitCode::FAILURE;
        }
    };

    let mut quit = ExitRequest::default();
    let outcome = run_discussion(&mut harness, &mut events, &question, &mut quit).await;
    harness.shutdown().await;

    // 这行在 alt screen 恢复之后打印，因为 TUI 的转录活不过进程：用户在屏幕上漏掉的东西，会话 id
    // 是回到它身边的持久途径。
    match outcome {
        Ok(outcome) => {
            eprintln!(
                "heng: {}",
                render::wording::discussion_ended(outcome.reason, outcome.rounds, &outcome.absent)
            );
            eprintln!(
                "heng: {}",
                render::wording::session_receipt(stored.id.as_str())
            );
            // 整个失败掉的讨论是一次失败；被取消的那个会话正是用户要的，如同交互式会话里被取消的一个回
            // 合（spec §6）。举手退出（`Quit`）走 130，与交互式那条路同一个判定。
            if outcome.reason == StopReason::Error {
                ExitCode::FAILURE
            } else {
                exit_code_after(quit.requested())
            }
        }
        Err(error) => {
            eprintln!("heng: {}", render::wording::error_report(&error));
            ExitCode::FAILURE
        }
    }
}

/// `/discuss [--debaters a,b] [问题]`：这条命令自己的旗标，以及那个问题。
///
/// 旗标只算开头的那些 —— 一旦出现别的东西，这一行剩下的部分（含换行）就是问题，所以一个问题可以
/// 以横线开头，也可以是一整段。
#[derive(Debug)]
pub(crate) struct DiscussLine {
    pub(crate) debaters: Option<[String; 2]>,
    pub(crate) question: String,
}

pub(crate) fn parse_discuss_line(text: &str) -> Result<DiscussLine, String> {
    let mut rest = text.trim_start();
    let mut debaters = None;
    loop {
        let Some((word, tail)) = rest.split_once(char::is_whitespace) else {
            return Ok(DiscussLine {
                debaters,
                question: rest.to_owned(),
            });
        };
        match word {
            "--debaters" | "--debater" => {
                let tail = tail.trim_start();
                match tail.split_once(char::is_whitespace) {
                    Some((value, after)) => {
                        debaters = Some(split_names(value)?);
                        rest = after;
                    }
                    None => {
                        return Ok(DiscussLine {
                            debaters: Some(split_names(tail)?),
                            question: String::new(),
                        });
                    }
                }
            }
            "--" => {
                return Ok(DiscussLine {
                    debaters,
                    question: tail.trim_start().to_owned(),
                });
            }
            other if other.starts_with("--") => {
                return Err(render::wording::unknown_argument(other));
            }
            _ => {
                return Ok(DiscussLine {
                    debaters,
                    question: rest.to_owned(),
                });
            }
        }
    }
}

/// `a,b` 析成两个讨论者名字。
pub(crate) fn split_names(value: &str) -> Result<[String; 2], String> {
    let names: Vec<String> = value
        .split(',')
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .collect();
    <[String; 2]>::try_from(names).map_err(|_| render::wording::needs_two_debaters(value))
}

/// 一次讨论用的那两位讨论者：点名要的，或者从池子里抽的。
///
/// 抽是默认做法，因为池子存在的意义就是换一对；`seed` 是注入的（运行时是时钟，测试里是常量），所
/// 以这个选择可复现。
pub(crate) fn pick_debaters(
    roster: &DiscussionRoster,
    requested: Option<[String; 2]>,
    seed: u64,
) -> Result<[Debater; 2], String> {
    let lookup = |name: &str| {
        roster
            .debater(name)
            .cloned()
            .ok_or_else(|| render::wording::unknown_debater(name, &roster.names()))
    };
    match requested {
        Some([first, second]) => Ok([lookup(&first)?, lookup(&second)?]),
        None => {
            let (first, second) = crate::discussion::pick_pair(roster.debaters.len(), seed)
                .ok_or_else(|| render::wording::discussion_no_roster().to_owned())?;
            Ok([
                roster.debaters[first].clone(),
                roster.debaters[second].clone(),
            ])
        }
    }
}

/// 在这一对辩论**之前**该说什么：池子里有两位同厂商，意味着拿到的是两个样本而不是两个独立判断。
///
/// 这是允许的 —— 一个订阅到期不该让讨论不可用 —— 但要明说，不能拿它冒充本设计适用的情况
/// （spec §15）。
pub(crate) fn advisory_lines(config: &Config, pair: &[Debater; 2]) -> Vec<String> {
    let [first, second] = pair;
    if first.model == second.model {
        return vec![render::wording::discussion_same_model(
            &render::wording::debater_label(&first.name, &first.model),
        )];
    }
    if config.debaters_share_a_vendor(&first.model, &second.model) {
        return vec![render::wording::discussion_one_vendor(
            &render::wording::debater_label(&first.name, &first.model),
            &render::wording::debater_label(&second.name, &second.model),
        )];
    }
    Vec::new()
}

/// 抽一对用的种子：时钟，按会话 id 后缀的方式混合。
///
/// 不是 PRNG —— 它只决定几个讨论者里哪两位参与 —— 但它必须在同一秒开始的两次讨论之间不同，而纳秒
/// 做到了这一点。
pub(crate) fn discussion_seed() -> u64 {
    use std::hash::{BuildHasher, Hasher};

    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    hasher.write_u128(nanos);
    hasher.finish()
}

/// 每位参与者一个 provider 与一组会话值，取自名册。
///
/// 两位讨论者用名册里的模型作答；合成器用路由表的落点作答 —— 什么都没路由时就是第一位讨论者的模
/// 型，也就是 `SessionConfig::model_for(Synthesizer)` 在组装内部解析出的那个。把它的 provider 恰
/// 好建在那个模型上，是让 provider 与发给它的配置保持一致的东西。
///
/// 两个入口都用它：`discuss` 子命令（自带的一次讨论）与 `/discuss`（跑在用户所在会话上的一次讨
/// 论）。
pub(crate) fn discussion_participants(
    config: &Config,
    pair: &[Debater; 2],
) -> Result<(Vec<DebaterParts>, SynthesizerParts), String> {
    let build = |model: &str| -> Result<(config::SessionConfig, Box<dyn crate::provider::Provider>), String> {
        let values = config
            .session_config(model)
            .map_err(|error| error.to_string())?;
        let provider = OpenAiProvider::build(config, model, stderr_warnings())
            .map_err(|error| error.to_string())?;
        Ok((values, Box::new(provider)))
    };
    let [first, second] = pair;
    let (first_config, first_provider) = build(&first.model)?;
    let (second_config, second_provider) = build(&second.model)?;
    let synthesizer_model = config
        .routing
        .synthesizer_model
        .clone()
        .unwrap_or_else(|| first.model.clone());
    let (synthesizer_config, synthesizer_provider) = build(&synthesizer_model)?;
    Ok((
        vec![
            DebaterParts {
                // 讨论者的**名字**才是它在流上的身份 —— 不是模型，池子可能同时把同一个模型交给两位
                // 讨论者（spec §5、§15）。
                speaker: SpeakerId::Debater(first.name.as_str().into()),
                config: first_config,
                provider: first_provider,
                soul: first.soul.clone(),
            },
            DebaterParts {
                speaker: SpeakerId::Debater(second.name.as_str().into()),
                config: second_config,
                provider: second_provider,
                soul: second.soul.clone(),
            },
        ],
        SynthesizerParts {
            config: synthesizer_config,
            provider: synthesizer_provider,
        },
    ))
}

/// `/discuss [问题]`：在**当前会话的流上**跑一次讨论（spec §15）。
///
/// 键盘归循环、日志归会话，所以这里从配置里读出名册，为每位参与者建一个 provider，然后把用户已经
/// 在的那个会话交给协议：讨论者是它的兄弟，于是继承它的上下文、轮次落进它的流。用户需要知道的一切
/// —— 哪些模型、什么问题、怎么收尾的 —— 都走渲染器而不是 stderr，因为屏幕归 TUI。
pub(crate) async fn discuss_in_session(
    harness: &mut Harness,
    inbox: &mut tokio::sync::mpsc::Receiver<CliCommand>,
    config: &Config,
    asked: String,
    quit: &mut ExitRequest,
) -> Result<(), crate::Error> {
    let Some(roster) = config.discussion.clone() else {
        harness.notice(&format!(
            "heng: {}",
            render::wording::discussion_no_roster()
        ));
        return Ok(());
    };
    // 先 `--debaters a,b`，再是问题：旗标属于这条命令，而这一行剩下的部分才是用户要问的东西。
    let line = match parse_discuss_line(&asked) {
        Ok(line) => line,
        Err(message) => {
            harness.notice(&format!("heng: {message}"));
            return Ok(());
        }
    };
    // 一句光秃秃的 `/discuss` 就把这个会话刚被问到的那个东西交给两个模型：「我们刚才在谈什么」正是
    // 值得第二份判断的那个问题。
    let question = if line.question.trim().is_empty() {
        match harness.last_question() {
            Some(last) => last,
            None => {
                harness.notice(&format!(
                    "heng: {}",
                    render::wording::discuss_needs_in_session_question()
                ));
                return Ok(());
            }
        }
    } else {
        line.question
    };
    let pair = match pick_debaters(&roster, line.debaters, discussion_seed()) {
        Ok(pair) => pair,
        Err(message) => {
            harness.notice(&format!("heng: {message}"));
            return Ok(());
        }
    };
    let (debaters, synthesizer) = match discussion_participants(config, &pair) {
        Ok(parts) => parts,
        Err(message) => {
            harness.notice(&format!("heng: {message}"));
            return Ok(());
        }
    };
    for line in advisory_lines(config, &pair) {
        harness.notice(&format!("heng: {line}"));
    }
    harness.notice(&format!(
        "heng: {}",
        render::wording::discussion_starting(
            &render::wording::debater_label(&pair[0].name, &pair[0].model),
            &render::wording::debater_label(&pair[1].name, &pair[1].model),
            &question,
        )
    ));

    let signal = harness.cancel_signal();
    // 模式句柄随行，理由与取消信号相同：那个 run future 借走了 harness，而这个手势无论如何必须够
    // 得到策略。
    let modes = harness.mode_cycle();
    // 同上：`Quit` 只记请求、让这次讨论拿到收尾，130 由 `interactive_loop` 兑现。
    // 这个 future 在它运行的整段时间里借走 harness，所以它自成一个作用域：下面的通告要把 harness
    // 拿回来。
    // 回合 future 借着 harness，所以回执走一个在借用**之前**就克隆下来的渲染句柄
    // （与 `ModeCycle` 同一个理由）。
    let render = harness.render_handle();
    let outcome = {
        let mut run =
            Box::pin(harness.discuss(&question, debaters, synthesizer, roster.max_rounds));
        loop {
            tokio::select! {
                result = &mut run => break result?,
                command = inbox.recv() => match command {
                    // 模式是权限门每次调用都读的一个值，所以这次按键立刻生效、哪怕是在讨论中途：句
                    // 柄之所以存在，是因为 run future 借走了 harness（spec §12）。
                    Some(CliCommand::Gesture { event: FrontEndEvent::CycleMode, .. }) => {
                        modes.cycle();
                    }
                    // 运行中要切换：拒绝，并说清为什么（spec §6）。与模式手势不同形 ——
                    // 换模型要换 provider 与请求参数，而一个讨论正在跑着。
                    Some(CliCommand::Gesture { event: FrontEndEvent::OpenPicker(_), .. }) => {
                        render.notice(&render::wording::heng(render::wording::switch_busy()));
                    }
                    Some(CliCommand::Gesture { event, .. }) => quit.apply(&event, &signal),
                    // 输入结束让讨论像被取消一样落下来，所以流仍然得到它的收尾。
                Some(CliCommand::Close) | None => signal.cancel(),
                // 跑着的时候到达的提交：这个会话的循环一次只办一件，多的忽略。
                Some(_) => {}
                },
            }
        }
    };
    harness.notice(&format!(
        "heng: {}",
        render::wording::discussion_ended(outcome.reason, outcome.rounds, &outcome.absent)
    ));
    Ok(())
}

/// 驱动一次讨论，同时仍然盯着取消手势。
///
/// 形状与 [`run_one_turn`] 相同，理由也相同：讨论占着会话，所以循环没法自己读键盘，改为监听手势。
/// `Quit` 同样只是「取消 + 记账」，进程的收尾与退出码归调用方
/// （`.scratch/exit-gesture/spec.md` §3）。
pub(crate) async fn run_discussion(
    harness: &mut DiscussionHarness,
    events: &mut ConsoleEvents,
    question: &str,
    quit: &mut ExitRequest,
) -> Result<crate::agent::DiscussionOutcome, crate::Error> {
    let signal = harness.cancel_signal();
    let modes = harness.mode_cycle();
    let render = harness.render_handle();
    let mut run = Box::pin(harness.discuss(question));
    loop {
        tokio::select! {
            result = &mut run => return result,
            event = events.recv() => match event {
                // 一个策略盖住三位参与者，所以这与它在别处的是同一个手势（spec §12）。
                Some(FrontEndEvent::CycleMode) => {
                    modes.cycle();
                }
                // 讨论 CLI 这条路径上换不了模型：模型是**名册**里的配置事实，而这个会话
                // 讨论是独立会话（spec §12）。所以这一格给的与忙闲无关，是另一句话。
                Some(FrontEndEvent::OpenPicker(_)) => {
                    render.notice(&render::wording::heng(
                        render::wording::switch_not_switchable(),
                    ));
                }
                Some(event) => quit.apply(&event, &signal),
                // 输入结束让讨论像被取消一样落下来，所以流仍然得到它的收尾。
                None => signal.cancel(),
            },
        }
    }
}
