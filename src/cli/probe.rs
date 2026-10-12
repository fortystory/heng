use super::*;

/// 探针第二个回合的提示词；它让转录继续变长，好让第一个回合的前缀成为缓存必须匹配的那些内容。
pub(crate) const PROBE_FOLLOW_UP: &str = "严格只回复：done";

/// 两次探针回合之间等多久。
///
/// DeepSeek 在磁盘上建它的前缀缓存要花「秒」级的时间；立刻再问一次量到的是冷缓存，会报出假阴性。
pub(crate) const CACHE_WARMUP: std::time::Duration = std::time::Duration::from_secs(10);

/// 探针的第一个回合。填充得远超两家厂商的缓存下限（Kimi 只缓存 256 token 以上的 prompt），这样
/// 第二个请求才有可能显出一次命中。
pub(crate) fn probe_prompt() -> String {
    const FILLER: &str = "敏捷的棕色狐狸跳过了那只懒狗，前缀缓存正在热起来。";
    let mut prompt = String::from("忽略下面的填充：它只是把提示词撑长，好让前缀缓存起作用。\n");
    while prompt.len() < 2_000 {
        prompt.push_str(FILLER);
    }
    prompt.push_str("\n严格只回复：ok");
    prompt
}

#[derive(Debug, Default)]
pub(crate) struct ProbeArgs {
    pub(crate) config: Option<PathBuf>,
    pub(crate) models: Vec<String>,
}

pub(crate) fn parse_probe(args: &[String]) -> Result<ProbeArgs, String> {
    let mut parsed = ProbeArgs::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--config" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| render::wording::needs_path("--config"))?;
                parsed.config = Some(PathBuf::from(value));
            }
            "--model" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| render::wording::needs_model("--model"))?;
                parsed.models.push(value.clone());
            }
            other => return Err(render::wording::unknown_argument(other)),
        }
        index += 1;
    }
    Ok(parsed)
}

/// 加载配置：显式的 `--config` 必须存在；默认路径只在它存在时才被采用。
pub(crate) fn load_config(explicit: Option<PathBuf>, env: &EnvMap) -> Result<Config, String> {
    match explicit {
        Some(path) => config::load(&path, env).map_err(|error| error.to_string()),
        None => {
            let path = config::default_path(env);
            if path.exists() {
                config::load(&path, env).map_err(|error| error.to_string())
            } else {
                config::resolve(None, env).map_err(|error| error.to_string())
            }
        }
    }
}

pub(crate) async fn probe(args: &[String], env: &EnvMap) -> ExitCode {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", render::wording::help_probe());
        return ExitCode::SUCCESS;
    }
    let parsed = match parse_probe(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("heng: {message}");
            println!("{}", render::wording::help_probe());
            return ExitCode::FAILURE;
        }
    };
    let config = match load_config(parsed.config, env) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("heng: {}", render::wording::startup_config(&message));
            return ExitCode::FAILURE;
        }
    };

    // 没登记的模型 id 是启动错误，绝不是悄悄降级：探测任何东西之前先查整张表。
    if let Err(message) = validate_models(&config) {
        eprintln!("heng: {message}");
        return ExitCode::FAILURE;
    }

    let models: Vec<String> = if parsed.models.is_empty() {
        config
            .models_with_keys()
            .into_iter()
            .map(|model| model.id.clone())
            .collect()
    } else {
        parsed.models
    };
    if models.is_empty() {
        eprintln!("heng: {}", render::wording::probe_no_key());
        return ExitCode::FAILURE;
    }

    let mut failed = false;
    // 库不读任何环境，所以 CLI 把它唯一需要的那件事实交给它 —— `rm` 断路器要用的那个。
    let home = env
        .get("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    for model in models {
        match probe_model(&config, &model, home.as_deref(), env).await {
            Ok(()) => {}
            Err(ProbeError::Skipped(message)) => eprintln!("heng: 跳过 {model}：{message}"),
            Err(ProbeError::Failed(message)) => {
                eprintln!("heng: {model}: {message}");
                failed = true;
            }
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// 每一个已配置的模型 id 都必须在任何东西跑起来之前就在能力表里，所以没登记的 id 是启动错误，而不
/// 是一个回合开始之后才发现的意外。
pub(crate) fn validate_models(config: &Config) -> Result<(), String> {
    for model in config.models.values() {
        caps_for(&model.id).map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(crate) enum ProbeError {
    Skipped(String),
    Failed(String),
}

impl ProbeError {
    fn failed(error: impl std::fmt::Display) -> Self {
        ProbeError::Failed(error.to_string())
    }
}

pub(crate) async fn probe_model(
    config: &Config,
    model_id: &str,
    home: Option<&Path>,
    env: &EnvMap,
) -> Result<(), ProbeError> {
    let (_, profile) = config
        .resolve_model(Some(model_id))
        .map_err(ProbeError::failed)?;
    let provider = match OpenAiProvider::build(config, model_id, stderr_warnings()) {
        Ok(provider) => provider,
        Err(BuildError::MissingKey { hint, .. }) => {
            return Err(ProbeError::Skipped(format!(
                "没有 API key（请 export {hint}）"
            )));
        }
        Err(error) => return Err(ProbeError::failed(error)),
    };

    let dir = probe_dir(model_id);
    std::fs::create_dir_all(&dir).map_err(ProbeError::failed)?;
    let log_path = dir.join("log.jsonl");
    let _ = std::fs::remove_file(&log_path);

    // 会话需要的每一个已配置的值 —— 生成参数、token 配额、价目表、路由覆盖 —— 都经同一个函数到达
    // （spec §17），所以这个探针与以后任何前端组装出来的东西都一样。
    let session_config = config
        .session_config(model_id)
        .map_err(ProbeError::failed)?;
    let mcp = mcp_service(config, &dir, env, home, None).await;
    let probe_session = SessionId::new(format!("probe-{model_id}"));
    // 进程级的一张锁表：同一入口里开出的每一个会话都从它取自己 cwd 的那一张 —— 同 cwd
    // 共享一把（写互斥真的成立），异 cwd 各一把（不互相阻塞）（票 13）。取锁表要在
    // `cwd` 被移进脚手架**之前**，所以这里就把两件事合成一句。
    let path_locks = crate::tools::PathLockTable::new().locks(&dir);
    let channel = crate::render::RenderChannel::start(
        Renderer::headless(RenderSinks {
            // 探针把自己的报告打在 stdout 上；渲染器只往 stderr 叙述。
            stdout_result: Box::new(std::io::sink()),
            stderr_diagnostic: Box::new(std::io::stderr()),
        }),
        probe_session.clone(),
    );
    let mut harness = assemble(AssemblyParts {
        scaffold: SessionScaffold {
            cwd: dir,
            log_path: log_path.clone(),
            session_id: probe_session,
            // 探针要跑真实回合，所以给它真实的工具表 —— 除了 `ask_user_question`：无头没有应答者，
            // 而一个只能失败的工具会白白浪费一次模型调用（spec §19）。
            tools: tools::with_mcp(
                tools::with_web(
                    tools::with_dynamic(&config.tools, false),
                    web_service(config),
                ),
                mcp,
            ),
            locks: path_locks,
            // 探针是无头的、没有应答者，所以配置那一档的 `ask`（默认）会拒掉写，而不是挂在一个谁也
            // 看不见的问题上。
            policy: Policy::for_mode(config.mode).with_outside_read(config.outside_read),
            asker: None,
            questions: None,
            // 票 05 落地挂载点；把用户声明的钩子接进 CLI 还没有归属的票，所以探针不带钩子跑。
            hook: None,
            home: home.map(Path::to_path_buf),
        },
        provider: Box::new(provider),
        speaker: SpeakerId::Debater(profile.name.clone().into()),
        config: session_config,
        render: channel,
    })
    .await
    .map_err(ProbeError::failed)?;

    println!(
        "模型 {model_id}（provider {}，{}）",
        profile.name, profile.base_url
    );
    let first = probe_prompt();
    let turns: [&str; 2] = [first.as_str(), PROBE_FOLLOW_UP];
    for (turn, prompt) in turns.iter().enumerate() {
        if turn > 0 {
            // 在再问一次之前，先让厂商把第一个回合的前缀持久化下来。
            tokio::time::sleep(CACHE_WARMUP).await;
        }
        harness.run_turn(prompt).await.map_err(ProbeError::failed)?;
        // 那个回合刚写下的流是这份报告唯一的来源。读不了它时，这次运行不会被悄悄报成免费；它会说
        // 出来。
        let events = match read_events(&log_path) {
            Ok(events) => events,
            Err(error) => {
                println!("  第 {} 回合：读不了会话流：{error}", turn + 1);
                continue;
            }
        };
        match last_usage(&events) {
            Some(usage) => println!(
                "  第 {} 回合：input={} output={} cached={} miss={}",
                turn + 1,
                usage.input_tokens,
                usage.output_tokens,
                usage.cached_tokens,
                usage.miss_tokens,
            ),
            None => println!("  第 {} 回合：没有记下用量", turn + 1),
        }
        // 钱只作显示（spec §17）：模型有价格时探针显示这个会话花了多少，没有时说清缺的是哪张表。
        // `sessions stats`（票 17）才是它长期该待的地方；探针是今天真有人读的那份报告。
        let spent = total_usage(&events);
        match config.pricing.cost(model_id, spent) {
            Some(cost) => println!("  会话：{} token，${cost:.6}", spent.total_tokens()),
            None => println!(
                "  会话：{} token（没有 [pricing.{model_id}] 条目，所以不算钱）",
                spent.total_tokens()
            ),
        }
    }
    harness.shutdown().await;
    Ok(())
}

pub(crate) fn last_usage(events: &[Event]) -> Option<Usage> {
    events.iter().rev().find_map(|event| match &event.payload {
        EventPayload::UsageRecorded { usage } => Some(*usage),
        _ => None,
    })
}

/// 这个会话的联网服务：`[web] enabled` 决定那两个工具在不在表里，后端在这里解析并挂上。
///
/// 组装期建一次 —— 工具表建完不再变化，所以这不是一个运行期开关。**解析不出后端（名字不认识）
/// 或解析不出凭据时，工具仍在表里**：调用给出的是结构化错误，而不是让表随凭据状态抖动
/// （`.scratch/web-search-tool/spec.md` §3）。
pub(crate) fn web_service(config: &Config) -> WebService {
    let settings = config.web.clone();
    if !settings.enabled {
        return WebService::new(settings);
    }
    // 抓取的后端是自己的 HTTP：没有服务器工具可用（DeepSeek 的兼容表里没有
    // `web_fetch_tool_result`），所以那一层的 SSRF 防护就是全部的防线。
    let service = WebService::new(settings.clone()).with_fetch(Arc::new(
        HttpFetch::new(
            settings.fetch_max_chars,
            std::time::Duration::from_millis(settings.fetch_timeout_ms),
        )
        .with_trust_proxy_dns(settings.trust_proxy_dns),
    ));
    match settings.search_provider.as_str() {
        DEEPSEEK_SEARCH_PROVIDER => {
            // 零新密钥：搜索复用会话这一家已经配好的那一把。
            let key = config
                .provider(DEEPSEEK_SEARCH_PROVIDER)
                .and_then(|provider| provider.api_key.clone());
            service.with_search(Arc::new(DeepSeekSearch::new(
                &settings.search_base_url,
                key,
            )))
        }
        // 不认识的名字：不挂搜索后端，调用给出 `WEB_PROVIDER_UNAVAILABLE`，消息里点名配的是
        // 哪一家。
        _ => service,
    }
}

/// 工具表里每个工具的**声明 schema**，按工具名 —— 详情覆盖层的 Schema 面要的那份账
/// （[`SessionFacts::tool_schemas`]）。
///
/// 它直接来自 [`Registry::specs`]：那正是 `agent` 每次请求发给 provider 的同一份声明，所以渲染
/// 层画出来的与模型手里的不会漂开，也不经第二次加工。表在组装期建好之后不再变，于是这一份抄本
/// 只在组装时取一次。
pub(crate) fn tool_schemas(registry: &Registry) -> BTreeMap<String, serde_json::Value> {
    registry
        .specs()
        .into_iter()
        .map(|spec| (spec.name, spec.parameters))
        .collect()
}

/// 这个会话的 MCP 服务（`.scratch/mcp-support/spec.md` §1、§3、§5）。
///
/// `[mcp] enabled`（缺省关）决定四个元工具在不在表里。项目级的 `.mcp.json` 已经由
/// [`Config::load_project_mcp`] 在 cwd 定下来之后逐台并进 `config.mcp`，所以这里读到的就是
/// 最终那一份。
///
/// 打开时在组装期把每台 server **并发**连起来，失败的跳过：它的元工具调用给出的是一条结构化
/// 错误，而其余几台照常工作（spec §3）。
pub(crate) async fn mcp_service(
    config: &Config,
    cwd: &Path,
    env: &EnvMap,
    home: Option<&Path>,
    questions: Option<Arc<dyn UserQuestions>>,
) -> McpService {
    let sandbox = mcp_sandbox(config, cwd);
    // `env` 是整份环境快照；连接层只从里面挑白名单那三个键（`PATH` / `HOME` / `LANG`）——
    // 「只给 server 该拿的」这件事在连接层强制，前端不负责挑。
    let options = mcp::ConnectOptions::new(cwd, &sandbox, env)
        .with_home(home)
        // server 的输入请求走**同一个**问询端口（票 15）：两条路不会漂成两套实现。
        .with_questions(questions)
        .with_stderr(Arc::new(|line: &str| {
            eprintln!("heng: {}", render::wording::mcp_server_stderr(line));
        }));
    mcp::connect_all(&config.mcp, &options).await
}

/// 建 MCP 连接时要用的沙箱。
///
/// 与 `Harness::session` 用的是同一份 [`config::SandboxSettings`]，只是这里要**自己探测一次**：
/// MCP 的连接在组装之前就建好了，那时 harness 还不存在。`mode = "off"` 时不探测也没关系。
pub(crate) fn mcp_sandbox(config: &Config, cwd: &Path) -> Sandbox {
    let mut settings = config.sandbox.clone();
    // 走组装期那同一个入口（`resolve_availability` 自己看 `needs_probe()`），不在这一处另写
    // 一遍「该不该探」的判断。
    settings.availability = tools::sandbox::resolve_availability(&settings, cwd);
    Sandbox::new(&settings)
}

pub(crate) fn probe_dir(model_id: &str) -> PathBuf {
    let slug: String = model_id
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect();
    std::env::temp_dir().join("heng-probe").join(slug)
}

#[derive(Debug)]
pub(crate) struct PruneArgs {
    /// 这个桶里要留多少个最近的会话。
    pub(crate) keep: usize,
    pub(crate) dry_run: bool,
    /// 要 prune 哪个工作区的桶；默认是当前目录。
    pub(crate) cwd: Option<PathBuf>,
}

pub(crate) fn parse_prune(args: &[String]) -> Result<PruneArgs, String> {
    let mut parsed = PruneArgs {
        // 留下 `--continue` 会接着跑的那个会话，删掉其余的。
        keep: 1,
        dry_run: false,
        cwd: None,
    };
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--keep" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| render::wording::needs_value("--keep"))?;
                parsed.keep = value
                    .parse()
                    .map_err(|_| render::wording::needs_number("--keep", value))?;
            }
            "--dry-run" => parsed.dry_run = true,
            "--cwd" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| render::wording::needs_path("--cwd"))?;
                parsed.cwd = Some(PathBuf::from(value));
            }
            other => return Err(render::wording::unknown_argument(other)),
        }
        index += 1;
    }
    Ok(parsed)
}

/// `prune` 手动删掉会话目录（spec §11：什么都不自动 prune）。
///
/// 它只作用在一个桶上 —— 这个工作区的桶 —— 因为那是 store 所绑定的单位，而它留下最新的 `--keep`
/// 个会话（默认一个会话，即 `--continue` 会接着跑的那个会话）。stdout 承载结果，诊断走 stderr。
pub(crate) fn prune(args: &[String], env: &EnvMap) -> ExitCode {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", render::wording::help_prune());
        return ExitCode::SUCCESS;
    }
    let parsed = match parse_prune(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("heng: {message}");
            println!("{}", render::wording::help_prune());
            return ExitCode::FAILURE;
        }
    };
    let cwd = match parsed.cwd {
        Some(path) => path,
        None => match std::env::current_dir() {
            Ok(dir) => dir,
            Err(error) => {
                eprintln!("heng: {}", render::wording::startup_cwd(&error.to_string()));
                return ExitCode::FAILURE;
            }
        },
    };
    let Some(root) = config::sessions_dir(env) else {
        eprintln!("heng: {}", render::wording::startup_no_session_store());
        return ExitCode::FAILURE;
    };
    let store = SessionStore::new(root);

    if parsed.dry_run {
        return match store.list(&cwd) {
            Ok(sessions) => {
                for session in sessions.into_iter().skip(parsed.keep) {
                    println!(
                        "{}",
                        render::wording::prune_would_remove(
                            session.id.as_str(),
                            &session.dir.display().to_string()
                        )
                    );
                }
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!(
                    "heng: {}",
                    render::wording::startup_store_read(&error.to_string())
                );
                ExitCode::FAILURE
            }
        };
    }

    match store.prune(&cwd, parsed.keep) {
        Ok(removed) => {
            for session in removed {
                println!(
                    "{}",
                    render::wording::prune_removed(
                        session.id.as_str(),
                        &session.dir.display().to_string()
                    )
                );
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "heng: {}",
                render::wording::startup_store_prune(&error.to_string())
            );
            ExitCode::FAILURE
        }
    }
}

// ---------------------------------------------------------------------------
// `sessions`：可观测性 CLI（spec §18）
// ---------------------------------------------------------------------------
