use super::discuss::*;
use super::goals::*;
use super::loading::*;
use super::probe::*;
use super::sessions::*;
use super::*;

/// 接着跑哪一个会话（`-c` / `--continue` / `--session`）。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Resume {
    /// 本工作区里最近写下的那个会话。
    Latest,
    /// 指名的那个 id，或者它会话目录的路径。
    Session(String),
}

/// 一次已解析的交互式调用。渲染器在这里选定并注入组装，所以只会有一个模式在跑。
#[derive(Debug, Default)]
pub(crate) struct InteractiveArgs {
    /// 强制用 plain 渲染器。
    pub(crate) plain: bool,
    /// 强制用 TUI 渲染器。
    pub(crate) tui: bool,
    /// 接着跑哪一个会话（spec §11）：不写就是新开一个会话。
    pub(crate) resume: Option<Resume>,
    pub(crate) config: Option<PathBuf>,
    pub(crate) model: Option<String>,
    /// `--mode readonly|ask|workspace|auto`：这一趟跑的权限模式，覆盖 `[permissions] mode`（spec §12）。不
    /// 写表示「文件怎么写就怎么来」。
    pub(crate) mode: Option<Mode>,
    pub(crate) cwd: Option<PathBuf>,
}

pub(crate) fn parse_interactive(args: &[String]) -> Result<InteractiveArgs, String> {
    let mut parsed = InteractiveArgs::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--plain" => parsed.plain = true,
            "--tui" => parsed.tui = true,
            // `-c` / `--continue` 后面跟一个不以 `-` 开头的词，就是**指名**续哪个会话
            // （`heng -c 20261001T155845Z-7a69cbff`）；没有就是本工作区最新那个会话。
            // 判的是「下一个词是不是旗标」而不是「是不是 id」：这样 `heng -c --plain`
            // 仍然是「续最新 + plain」，不会去找一个会话叫 `--plain` 的会话。
            "--continue" | "-c" => {
                parsed.resume = Some(match args.get(index + 1) {
                    Some(id) if !id.starts_with('-') => {
                        index += 1;
                        Resume::Session(id.clone())
                    }
                    _ => Resume::Latest,
                });
            }
            // 同一个意思的显式拼写，只是它**必须**给 id（而且和 `-c` 一样，旗标不算 id）。
            "--session" => {
                index += 1;
                let id = args
                    .get(index)
                    .filter(|value| !value.starts_with('-'))
                    .ok_or_else(|| render::wording::needs_value("--session"))?;
                parsed.resume = Some(Resume::Session(id.clone()));
            }
            flag @ ("--config" | "--model" | "--mode" | "--cwd") => {
                let flag = flag.to_owned();
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| render::wording::needs_value(&flag))?;
                match flag.as_str() {
                    "--config" => parsed.config = Some(PathBuf::from(value)),
                    "--model" => parsed.model = Some(value.clone()),
                    // 在这里解析而不是在组装里：权限门不认识的一个模式是打错字，而打错字值得在
                    // provider 被构建、会话被创建之前就拒掉。
                    "--mode" => {
                        parsed.mode = Some(
                            Mode::parse(value)
                                .ok_or_else(|| render::wording::unknown_mode(value))?,
                        )
                    }
                    "--cwd" => parsed.cwd = Some(PathBuf::from(value)),
                    _ => unreachable!(),
                }
            }
            other => return Err(render::wording::unknown_argument(other)),
        }
        index += 1;
    }
    if parsed.plain && parsed.tui {
        return Err(render::wording::renderers_mutually_exclusive().to_owned());
    }
    Ok(parsed)
}

/// 一趟运行从哪一档开始：`--mode` 旗标压过 `[permissions] mode`（spec §12；
/// `.scratch/todo-and-modes/spec.md` §1）。
///
/// 做成具名函数而不是内联表达式，因为「两者谁赢」**就是**入口点的决定，而这里是测试唯一能把它按住
/// 的地方 —— 剩下的路径需要终端。
pub(crate) fn effective_mode(parsed: &InteractiveArgs, config: &Config) -> Mode {
    parsed.mode.unwrap_or(config.mode)
}

/// 交互式会话：一个工作区、一个渲染器、一个键盘。
///
/// 渲染器在组装之前选定并注入，而同一个控制台端口既服务循环的提示、也服务权限门的询问 —— 这两样东
/// 西来自同一个键盘（spec §19）。
pub(crate) async fn interactive(args: &[String], env: &EnvMap) -> ExitCode {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", render::wording::help_interactive());
        return ExitCode::SUCCESS;
    }
    let parsed = match parse_interactive(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("heng: {message}");
            println!("{}", render::wording::help_interactive());
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
    let model = parsed
        .model
        .clone()
        .unwrap_or_else(|| config.default_model.clone());
    let profile = match config.resolve_model(Some(&model)) {
        Ok((_, profile)) => profile.clone(),
        Err(error) => {
            eprintln!("heng: {error}");
            return ExitCode::FAILURE;
        }
    };
    let Some(root) = config::sessions_dir(env) else {
        eprintln!("heng: {}", render::wording::startup_no_session_store());
        return ExitCode::FAILURE;
    };
    let store = SessionStore::new(root);
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
    // 打开哪个会话，以及这一趟的工作目录 —— 按 id 续上别的工作区的会话时，cwd 跟着那个会话走。
    let chosen = match choose_session(&store, &cwd, parsed.resume.as_ref()) {
        Ok(chosen) => chosen,
        Err(message) => {
            eprintln!("heng: {}", message);
            return ExitCode::FAILURE;
        }
    };
    let (stored, cwd) = (chosen.stored, chosen.cwd);
    let elsewhere = chosen.elsewhere;
    // 项目级的 MCP 配置跟着这一趟的工作目录走：仓库根的 `.mcp.json` 逐台盖用户级。
    // `[mcp] enabled = false`（缺省）时这一步连文件都不看。
    if let Err(error) = config.load_project_mcp(&cwd) {
        eprintln!(
            "heng: {}",
            render::wording::startup_config(&error.to_string())
        );
        return ExitCode::FAILURE;
    }

    let provider = match OpenAiProvider::build(&config, &model, stderr_warnings()) {
        Ok(provider) => provider,
        Err(error) => {
            eprintln!("heng: {error}");
            return ExitCode::FAILURE;
        }
    };
    let session_config = match config.session_config(&model) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("heng: {error}");
            return ExitCode::FAILURE;
        }
    };
    let home = env
        .get("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    // 目标清单住在数据根下、会话目录的旁边（`.scratch/goal-loop/spec.md` §1）。库不读环境，所以
    // 在这里定下来，随循环一起进去。
    let goals_dir = config::goals_dir(env);

    // 这一趟从哪一档开始：旗标压过文件（spec §12）。在这里一次性定下来，因为有三样东西要同一个答
    // 案 —— 权限门跑在哪一档、横幅、以及状态行。
    let mode = effective_mode(&parsed, &config);

    // 键盘的两端：循环持有的句柄与手势接收端，以及选定的渲染器（或 plain 逐行读取器）为它服务的那
    // 个端口。
    let (console, port, mut events) = render::console();
    let asker = Arc::new(ConsoleAsker::from_handle(&console));
    // 模型的问题走同一个键盘、走它自己的端口（spec §7）。交互式组装总是有一个 —— TUI 或 plain
    // —— 所以工具表提供 `ask_user_question`，而这张表是按端口在不在建的，不是按第二个容易漂移的
    // 标志建的。
    let questions: Option<Arc<dyn UserQuestions>> =
        Some(Arc::new(ConsoleQuestions::from_handle(&console)));

    // MCP 这一层也在组装期定下：`[mcp] enabled` 与仓库根的 `.mcp.json` 都在这里读一次。
    // 包成 `Arc`：工具表拿一份，`/` 菜单那半边（模板）也要拿一份 —— 同一次连接、同一个值。
    let mcp = Arc::new(mcp_service(&config, &cwd, env, home.as_deref(), questions.clone()).await);
    // 模板清单是**界面层**的东西：它进 `/` 菜单，不进工具表、也不进前缀缓存。server 不可用时
    // 它的条目根本不出现（spec §8）。
    let prompts = mcp_prompt_entries(&mcp).await;

    // 工具表在**渲染器之前**建出来：详情覆盖层的 Schema 面要读它那份声明，而它随会话事实在组装时
    // 注入一次（`SessionFacts::tool_schemas`）。表仍然只建一份 —— 往下照旧交给脚手架，没有第二张。
    let tools = tools::with_mcp(
        tools::with_web(
            tools::with_dynamic(&config.tools, questions.is_some()),
            web_service(&config),
        ),
        Arc::clone(&mcp),
    );
    let facts_tool_schemas = tool_schemas(&tools);

    let use_tui = parsed.tui || (!parsed.plain && std::io::stdout().is_terminal());
    let renderer = if use_tui {
        // 表头与面板显示这些；它们都不走事件流。模式就是其中之一：它是前端要显示、手势要挪动的一个
        // 会话值，而不再是流承载的东西。窗口是模型的输入预算。上面的 provider 已经解析过同一张表，
        // 所以下面这个失败是双保险：它让没登记的模型在这里也是启动错误，而不是在构建那些事实的地方
        // panic。
        let caps = match caps_for(&model) {
            Ok(caps) => caps,
            Err(error) => {
                eprintln!("heng: {error}");
                return ExitCode::FAILURE;
            }
        };
        let facts = SessionFacts {
            switchable: true,
            session_id: stored.id.as_str().to_owned(),
            session_dir: stored.dir.display().to_string(),
            model: model.clone(),
            context_window: crate::context::usable_input(&caps),
            mode,
            budget_limit: session_config.budget.limit,
            number_style: config.number_style,
            file_viewer: config.file_viewer,
            diff_viewer: config.diff_viewer.clone(),
            // 工具表里每一份声明 —— 详情覆盖层的 Schema 面读的就是它（票 24 收口补的那一面）。
            // 表在上面的渲染器之前建好，这里只是把它的声明抄成一份按名字查的账。
            tool_schemas: facts_tool_schemas,
            // 单 agent 会话以它的档案发言，所以那就是转录的名字配色需要安排的全部名册
            // （票 07 §1）。
            speaker_order: vec![profile.name.clone()],
        };
        Renderer::tui(TuiOptions {
            port,
            facts,
            // 标题的路径段用的就是这一档工作目录 —— `--cwd` 已经解析过了。
            cwd: cwd.clone(),
            // 重新打开的会话会在横幅之前重放它的历史，所以 TUI 必须知道，在那次重放到达之前什么都
            // 不要画（`.scratch/tui-history-replay/spec.md` §1、§3）。
            reopened: parsed.resume.is_some(),
            // 左栏的启动初值（票 26、27）：`auto` 只在 TUI 组装这一刻读一次环境，
            // plain 与 headless 不读。
            // `auto` 才读环境（spec 45 / 票 27）；`always` / `never` 是显式覆盖，不碰 env。
            sidebar: if config.sidebar == crate::config::SidebarMode::Auto {
                config.sidebar.initial_wanted(crate::herdr::in_herdr(env))
            } else {
                config.sidebar.initial_wanted(false)
            },
            tile_limit: config.tile_limit,
        })
    } else {
        // plain 前端读 stdin；它是行缓冲的，所以没有 raw 模式、也没有按键事件。
        render::spawn_plain_console(port);
        Renderer::plain(PlainOptions {
            sinks: RenderSinks {
                stdout_result: Box::new(std::io::stdout()),
                stderr_diagnostic: Box::new(std::io::stderr()),
            },
            color: std::io::stderr().is_terminal() && env.get("NO_COLOR").is_none(),
        })
    };
    // 渲染器整个进程只起一次：它是**进程级**的值，会话从它取一个带自己身份的写端
    // （`.scratch/session-tab/spec.md` §1、票 13）。
    // 进程级的一张锁表：同一入口里开出的每一个会话都从它取自己 cwd 的那一张 —— 同 cwd
    // 共享一把（写互斥真的成立），异 cwd 各一把（不互相阻塞）（票 13）。取锁表要在
    // `cwd` 被移进脚手架**之前**，所以这里就把两件事合成一句。
    let path_locks = crate::tools::PathLockTable::new().locks(&cwd);
    let channel = crate::render::RenderChannel::start(renderer, stored.id.clone());
    // 宿主表在组装**之前**就立起来（票 15 说它是这一层的持有者），因为按会话包的应答者与
    // 渲染侧的开关都要先拿到前台指针那一份订阅（票 22）。
    let mut host = SessionHost::<CliCommand>::new(stored.id.clone());
    let foreground = host.subscribe_foreground();
    // 第一个会话的应答者：真正的作答仍然是 `ConsoleAsker`，外面包一层「不是前台就挂住」。
    let session_asker = Arc::new(crate::session::SessionAsker::new(
        Arc::clone(&asker) as Arc<dyn Asker>,
        stored.id.clone(),
        foreground.clone(),
    ));
    let config = std::sync::Arc::new(config);
    let outside_read = config.outside_read;
    // 装载另一个会话要的那批材料（票 20）：同一批配置、同一个渲染端、同一张锁表。
    let kit = AssemblyKit {
        config: std::sync::Arc::clone(&config),
        model: model.clone(),
        speaker: profile.name.clone(),
        store: store.clone(),
        locks: path_locks.clone(),
        channel: channel.clone(),
        console_asker: asker.clone(),
        foreground,
        questions: questions.clone(),
        mcp: std::sync::Arc::clone(&mcp),
        home: home.clone(),
        mode,
        goals: GoalSetup {
            dir: goals_dir.clone(),
            store: store.clone(),
            cwd: cwd.clone(),
            settings: config.goals,
        },
    };
    let loaded = match load(AssemblyParts {
        scaffold: SessionScaffold {
            cwd: cwd.clone(),
            log_path: stored.log_path.clone(),
            session_id: stored.id.clone(),
            // 工具表在上面、在渲染器之前就建好了，就是这一张（内建的那些加上每一个动态声明的
            // 工具，spec §14）。
            tools,
            locks: path_locks.clone(),
            // 用户选的那一档：`[permissions] mode`，或者压在它上面的 `--mode`（spec §12）。无头调
            // 用方没有应答者，于是降级。
            policy: Policy::for_mode(mode).with_outside_read(outside_read),
            asker: Some(session_asker.clone()),
            questions: questions.clone(),
            hook: None,
            home: home.clone(),
        },
        provider: Box::new(provider),
        speaker: SpeakerId::Debater(profile.name.clone().into()),
        config: session_config,
        render: channel.clone(),
    })
    .await
    {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!("heng: {error}");
            return ExitCode::FAILURE;
        }
    };
    let Loaded {
        mut harness,
        read_only,
        filled,
    } = loaded;

    // 重新打开的会话组装时带上的历史，在横幅之前推给前端，好让 TUI 先把它铺好、横幅落在接缝**之
    // 后**，而不是落在接缝中间（`.scratch/tui-history-replay/spec.md` §1）。payload 就是组装好的
    // 那份快照，所以它带着 `--continue` 的恢复为悬空工具调用写下的合成结果。新会话没有历史，所以
    // 什么都不发。
    if parsed.resume.is_some() {
        console.replay(harness.events());
    }

    // 会话页要的那份列表（`.scratch/session-tab/spec.md` §3）：数据在这一侧扫出来 —— 渲染层
    // 不读环境，也不该为了一次列表去起 `git` 进程。
    if let Some(root) = config::sessions_dir(env) {
        let store = SessionStore::new(root);
        let mut titles = crate::session::TitleCache::new();
        let listing = crate::session::listing::sessions(
            &store,
            &mut titles,
            harness.session_id(),
            &std::collections::HashMap::new(),
        );
        console.sessions(listing);
    }

    // MCP 这一层加载成什么样，作为一条上下文注入进流（票 19）：与技能清单同形 —— 模型因此知道
    // 手里有哪些外部 server，而转录里那行**点得开**、详情就是同一段数据。
    if let Some(catalog) = mcp.catalog_text()
        && let Err(error) = harness.inject_context(ContextSource::McpCatalog, &catalog)
    {
        eprintln!("heng: {}", render::wording::error_report(&error));
    }

    // 用户故事 A.12：在第一个问题之前说清这是哪个模型、哪一档、哪个会话。它走渲染器而不是 stderr：
    // TUI 在 harness 组装时就已经启动，第二个往终端写的人会落在它的活动区域里、盖在状态行上。
    // 拼给模型的那段私有身份，给读的人留一条记录。它**不进事件流**（身份每次请求现拼、
    // 从不落盘，spec §15），所以轨迹页看到的是「按当前代码拼的一份」—— 那句话由来源名自己
    // 说出来。摆在横幅之前：那是模型看到的**第一样东西**。
    // 旧流缺字段：那几样是按**进程配置默认**补上的，如实说一句（票 18 §2）。
    if !filled.is_empty() {
        harness.notice(&render::wording::heng(&render::wording::facts_filled(
            &filled,
        )));
    }
    harness.identity(&crate::agent::agent_identity());
    harness.notice(&render::wording::banner(
        harness.session_id().as_str(),
        &model,
        harness.mode(),
        &stored.dir.display().to_string(),
        parsed.resume.is_some(),
    ));
    // 按 id 续上了一个别的工作区的会话：工作目录跟着它走了，所以先说一句 —— 否则下一步
    // `ls` 出来的东西与横幅上的目录对不上，人会以为是自己敲错了。
    if elsewhere {
        harness.notice(&render::wording::session_followed(
            &cwd.display().to_string(),
        ));
    }

    // `/` 菜单里的名字。真正对提交作出反应的是循环，所以「有哪些名字」也由循环说了算：它解析的那
    // 些内建命令，然后是这个会话发现到的技能 —— 这正是它在这里、在组装之后、第一个提示之前发出，
    // 而不是随表头那些事实一起注入的原因。
    console.catalog(
        harness.session_id().clone(),
        slash_catalog(
            &render::wording::BUILT_IN_COMMANDS,
            &harness.skill_catalog(),
            &prompts,
        ),
    );

    // 这个会话交给宿主表：拥有它的那个 task 跑完整段交互循环，主循环从表里驱动它
    // （`.scratch/session-tab/spec.md` §1）。渲染端先克隆一份 —— 会话收工之后由主循环等它
    // 把缓冲写完（会话那一份写端随 task 一起没了）。
    let render_channel = harness.render_channel();
    let session_id = harness.session_id().clone();
    let settings = config.goals;
    // 只读预览由 80ms 的宿主事实快照带给渲染器（`HostedSession::read_only`）：
    // 页内改名那种「渲染器自己发起」的写动作要挡住（票 18）。
    // 前台会话的只读快照（票 15 的 `FrontFacts`）：主循环解析每一次提交要的那几样。会话的
    // `Harness` 一交给 task 就问不到它了，所以在这里取一份。
    let front = harness.front_facts();
    let signal = harness.cancel_signal();
    // 提示行走渲染通道（它才是「往屏幕上说一句话」的那一端）；主循环自己那一份取启动这个会话
    // 的身份。
    let render = render_channel.handle(session_id.clone());
    let prompts_shared = std::sync::Arc::new(prompts);
    let (requests, mut open_requests) = tokio::sync::mpsc::unbounded_channel::<CliRequest>();
    let actor = CliActor {
        // 输入归主循环，渲染端的句柄两边各一份（它克隆很便宜）。
        console: console.clone(),
        config: std::sync::Arc::clone(&config),
        goals: GoalSetup {
            dir: goals_dir,
            store: store.clone(),
            cwd: cwd.clone(),
            settings,
        },
        mcp,
        prompts: std::sync::Arc::clone(&prompts_shared),
        read_only,
        loop_running: false,
        quit: ExitRequest::default(),
        requests: requests.clone(),
    };
    if let Err(error) = host.adopt_with(harness, cwd.clone(), read_only, Box::new(actor)) {
        eprintln!("heng: {error}");
        return ExitCode::FAILURE;
    }
    // 入表之后把这个会话的三态发送端交给应答者（票 22）—— 它得能置 `AwaitingHuman`。
    if let Some(entry) = host.get(&stored.id) {
        session_asker.attach(entry.state_tx.clone());
    }
    // 恢复（§10）：只对**重新打开**的会话做，而且只对认领过目标的那些。
    if parsed.resume.is_some()
        && front.goal.is_some()
        && let Some(entry) = host.get(&session_id)
    {
        let _ = entry.cmd.send(CliCommand::ResumeGoal).await;
    }

    // **输入归主循环**（`.scratch/session-tab/spec.md` §1、决策票 04 §3）：前端事件接收端是
    // 单一消费者，只有这一处读得动它。主循环读一次提交、解析成 `Submission`，再交给宿主表里
    // **前台**那个会话执行 —— 切换前台于是只是「换一个命令的收件人」。
    let mut quit = ExitRequest::default();
    // 宿主表的事实投影（票 03）：80ms 采一次样，`HostTracker` 一处推三态边沿、未读与两个
    // 后台计数，再作为一份 `HostFacts` 快照推给前端 —— 不再为每种事实各发一条命令。
    let mut host_tracker = crate::session::HostTracker::new();
    // herdr 的自接入上报（`src/herdr.rs`）：只在 `HERDR_ENV=1` 时存在。外面是 `None`，
    // 一次调用都不发。
    let herdr = crate::herdr::Reporter::from_env(env);
    // 上一次报给 herdr 的「前台会话 + 它的状态」。只报前台 —— herdr 一个 pane 只有一个 agent。
    let mut herdr_last: Option<(SessionId, crate::session::host::SessionState)> = None;
    // 那一行提示**只发一次、跨多轮 `select` 挂着**。
    //
    // 每轮重新 `console.prompt()` 会发出一个新的请求、把上一份应答端丢掉，而那 80ms 的
    // 窗口里的输入会被一份**已经没人等**的 reply 接走：`plain` 前端每个请求都真去读一行，
    // 于是 `/quit` 稳定无效；TUI 把最新一份 reply 记着，于是它偶发无效
    // （2026-10-12 用 pty 观测到，`scripts/tui-startup-check.py` 的 `/quit` 那一轮）。
    // 拿到一行之后才重新武装 —— 那时它确实完成了，留着它再 poll 是未定义行为。
    type LineFuture<'a> =
        std::pin::Pin<Box<dyn std::future::Future<Output = Option<String>> + Send + 'a>>;
    let mut prompt: Option<LineFuture<'_>> = None;
    let code = loop {
        // 「回到等一行」每轮都到这里（等输入时被 80ms 采样唤醒），但 `RunState` 是边沿
        // 通知：`ConsoleHandle::set_running` 只在值真的变了时才发，所以重复的这条不会把
        // 一次运行切成每 80ms 一次（`run-state-edge` 票 01）。
        console.set_running(false);
        if prompt.is_none() {
            prompt = Some(Box::pin(console.prompt()));
        }
        tokio::select! {
            line = prompt.as_mut().expect("上一句刚补上") => {
                prompt = None;
                let Some(line) = line else {
                    break ExitCode::SUCCESS;
                };
                let submission = submission(
                    &line,
                    |name| front.skills.iter().any(|skill| skill == name),
                    &prompts_shared,
                );
                // **空宿主**：一个会话都没有的时候没有可提交的对象 —— 回一句，别让它静默消失
                // （票 21）。退出与空行照旧放行。
                if host.is_empty()
                    && !matches!(submission, Submission::Quit | Submission::Ignore)
                {
                    render.notice(&render::wording::heng(
                        &render::wording::no_sessions_left(),
                    ));
                    continue;
                }
                // **只读预览**：凡写流的一律禁 —— 提交、所有 `/` 命令、页内改名都不行
                // （票 18）。退出不是写流，照旧放行。
                if read_only && !matches!(submission, Submission::Quit | Submission::Ignore) {
                    render.notice(&render::wording::heng(
                        &render::wording::read_only_session(),
                    ));
                    continue;
                }
                match submission {
                    Submission::Ignore => {}
                    // 退出：先把前台那个会话收工，再兑现退出码。
                    Submission::Quit => break ExitCode::SUCCESS,
                    other => {
                        console.set_running(true);
                        let target = host.foreground();
                        if let Some(entry) = host.get(&target) {
                            let _ = entry
                                .cmd
                                .send(CliCommand::Submit {
                                    submission: Box::new(other),
                                    raw: line,
                                })
                                .await;
                        }
                    }
                }
            }
            // 宿主表的事实投影（票 03）：80ms 采一次样。它是**给人看**的读数，不是同步手段。
            // 三态边沿、未读寿命、后台两个计数与装载了哪几个都由 `HostTracker` 一处推。
            _ = tokio::time::sleep(std::time::Duration::from_millis(80)) => {
                let sample = crate::session::HostSample::from_host(&host);
                let facts = host_tracker.update(&sample);
                console.facts(facts);
                // herdr 的自接入：**只报前台会话**的状态，变了才发（一个 pane 只有一个 agent）。
                if let Some(reporter) = &herdr {
                    let state = sample
                        .sessions
                        .iter()
                        .find(|session| session.id == sample.foreground)
                        .map(|session| session.state)
                        .unwrap_or_default();
                    if herdr_last.as_ref() != Some(&(sample.foreground.clone(), state)) {
                        reporter.report(
                            crate::herdr::AgentState::from_session(state),
                            sample.foreground.as_str(),
                        );
                        herdr_last = Some((sample.foreground.clone(), state));
                    }
                }
            }
            // 会话页上点了别的哪个会话：切过去（票 20）。宿主表在主循环手里。
            request = open_requests.recv() => match request {
                Some(CliRequest::Open(id)) => {
                    loading::open_session(&mut host, &kit, &console, &requests, &id).await;
                }
                Some(CliRequest::New) => {
                    let cwd = host
                        .get(&host.foreground())
                        .map(|entry| entry.cwd.clone())
                        .unwrap_or_else(|| cwd.clone());
                    loading::open_new(&mut host, &kit, &console, &requests, &cwd, &front, None).await;
                }
                Some(CliRequest::Rollover {
                    carry_tokens,
                    goal,
                    summary,
                }) => {
                    // 翻页（票 21）：同 cwd 新建一个会话，并把额度、目标与摘要交接过去。
                    let cwd = host
                        .get(&host.foreground())
                        .map(|entry| entry.cwd.clone())
                        .unwrap_or_else(|| cwd.clone());
                    loading::open_new(
                        &mut host,
                        &kit,
                        &console,
                        &requests,
                        &cwd,
                        &front,
                        Some((carry_tokens, goal, summary)),
                    )
                    .await;
                }
                Some(CliRequest::Close(id)) => {
                    close_session(&mut host, &kit, &console, &id).await;
                }
                None => {}
            },
            event = events.recv() => match event {
                Some(FrontEndEvent::Quit) => {
                    // **退出闸门**（票 23）：还有没收尾的会话就拒绝，并把可点入口指向第一个会话 ——
                    // 不自动取消、不代用户做决定。
                    match crate::session::exit_gate(&host) {
                        crate::session::ExitGate::Clear => {
                            quit.apply(&FrontEndEvent::Quit, &signal);
                            break quit.code();
                        }
                        crate::session::ExitGate::Running { count, first } => {
                            render.notice(&render::wording::heng(
                                &render::wording::exit_blocked(count),
                            ));
                            // 提示行那句话的可点入口指它，而**那把退出的举手随之作废**
                            // （票 28）：被拒之后下一次按下仍然只是第一下。
                            console.exit_blocked(count, first);
                        }
                    }
                }
                Some(FrontEndEvent::AttentionSeen) => {
                    // 用户有过一次输入：未读的「跑完」到此为止（票 25）。
                    host_tracker.attention_seen();
                }
                Some(FrontEndEvent::Cancel) => {
                    // `Ctrl-C` **只取消前台当前的动作，永不退出**（票 23）。它不再记进退出账本。
                    forward_gesture(&host, FrontEndEvent::Cancel);
                    // 前台没有可取消的东西时说一句；后台还有会话时顺带说那句。
                    let idle_foreground = host
                        .get(&host.foreground())
                        .is_none_or(|entry| *entry.state.borrow() == crate::session::host::SessionState::Idle);
                    if idle_foreground {
                        let background = host.len().saturating_sub(1);
                        render.notice(&render::wording::heng(
                            &render::wording::cancel_nothing(background),
                        ));
                    }
                }
                Some(event) => {
                    // 会话级的那三条由这个循环自己转成宿主表请求（票 20、票 21）—— 宿主表就在
                    // 它手里，转发给别人办只会多一圈（见 [`front_to_request`]）。其余手势归前台
                    // 那个会话：取消、模式循环、选择器、改名都是会话内部的动作。
                    match front_to_request(&event) {
                        Some(request) => {
                            let _ = requests.send(request);
                        }
                        None => {
                            quit.apply(&event, &signal);
                            forward_gesture(&host, event);
                        }
                    }
                }
                None => break ExitCode::SUCCESS,
            },
        }
    };
    // **先放掉主循环自己那份渲染写端**。`RenderChannel::shutdown` 的契约是「等每一个写端
    // 消失」，而循环手里那一个（`render`，给闸门拒绝与取消回执用的）在退出流程里还活着 ——
    // 渲染任务于是永远等不到「没有写端了」，而循环在 `await` 它：两头顶着，进程退不出去。
    // 退出卡住之后主循环不再回到 `select`，屏幕上的一切手势（按键、点击会话页）都静默失效 ——
    // 2026-10-12 维护者报告的那两条症状（`Ctrl-D` 不能退出、点击切会话没反应）是同一个死锁。
    drop(render);
    // 退出流程（票 23）：**广播 `Close` 给每一个会话**，等它们全部收工，再等渲染器把缓冲写完。
    // 退出码照旧只有 0 与 130 两种（`quit.code()`）。
    let ids: Vec<SessionId> = host
        .entries()
        .iter()
        .map(|entry| entry.id.clone())
        .collect();
    for id in &ids {
        let _ = host.close_with(id, CliCommand::Close).await;
    }
    // 退出时把 pane 还回 herdr（`src/herdr.rs`）：不清的话 herdr 要等 pane 回到 shell
    // 提示符才察觉，慢一两秒。同步执行 —— 异步线程会被进程退出带走。
    if let Some(reporter) = &herdr {
        reporter.release();
    }
    render_channel.shutdown().await;
    // 终端交还之后那一行（`.scratch/exit-gesture/spec.md` §5）：stdout 只承载最终产物，
    // 所以回执走 stderr；TUI 与 `--plain` 共用这一处收尾。启动不打它（横幅已经有 id），
    // `probe` / `sessions` 一族也不打。
    finish_session(code, session_id.as_str(), &mut std::io::stderr())
}

/// 那个问题：用它被给进来时的那些词，或者 stdin。
///
/// 终端上给一个提示 —— 还没有任何东西进过 raw 模式，所以 cooked 读仍然可用 —— 而管道读到结尾，这
/// 正是 `echo 问题 | heng discuss` 需要的。
pub(crate) fn question(words: &[String]) -> Option<String> {
    let typed = words.join(" ");
    if !typed.trim().is_empty() {
        return Some(typed);
    }
    if std::io::stdin().is_terminal() {
        eprint!("{}", render::wording::question_prompt());
        let _ = std::io::stderr().flush();
        let mut line = String::new();
        return match std::io::stdin().read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line.trim().to_owned()).filter(|question| !question.is_empty()),
        };
    }
    let mut text = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut text).ok()?;
    let text = text.trim().to_owned();
    (!text.is_empty()).then_some(text)
}

/// 一次 `/undo` 对前端说的那句话。
///
/// 三种结果**都**要开口：成功那次尤其 —— 它刚把一个文件改回原样，而屏幕上没有别的东西替它
/// 说话，少这一句就与「没东西可撤」长得一模一样（`.scratch/fs-agent-v1/issues/35-undo-receipt.md`）。
pub(crate) fn undo_receipt(outcome: Result<Option<UndoOutcome>, crate::Error>) -> String {
    match outcome {
        Ok(Some(undone)) => render::wording::heng(&render::wording::undone(&undone.path)),
        Ok(None) => render::wording::heng(render::wording::nothing_to_undo()),
        Err(error) => render::wording::heng(&render::wording::error_report(&error)),
    }
}

/// 读一行、跑它、重复 —— 直到用户离开或输入结束。
/// 组装一个新模型：provider、会话配置与它挂在哪个 profile 下。
///
/// 会话中途换模型与换档的**四个入口**（`/model`、`/effort`、点状态行那两格、`ctrl-t`）都从这里
/// 出（spec §7），所以「换完之后下一次请求长什么样」只有一个答案。
///
/// `carried_tokens` 由调用方给（从当前会话读出来）：累计额度是**会话**事实 —— 这个会话之前已经
/// 花掉的 token，换模型不该抹掉它（`.scratch/goal-loop/spec.md` §8 的「翻页不重置额度」是同一条
/// 纪律）。其余从配置来的一切（预算、打码器、逐 agent 的旋钮、沙箱）照
/// [`Config::session_config`] 给的走 —— 换模型后预算按新模型的配置走，这是对的。
///
/// `[routing]` 的两个覆盖由 `session_config` 重新填，而不是从上一个会话的副本抄过来：它们是**配置
/// 事实**，换模型不该跟着上一个会话的模型走（spec「明确不做」第三条）。
pub(crate) fn retarget_to(
    config: &Config,
    model: &str,
    effort: Option<ReasoningEffort>,
    carried_tokens: u64,
) -> Result<Retargeted, String> {
    // 先问能力表：没登记的 model id 那条错误把已知 id 列全了，是这里最有用的一句。
    let caps = caps_for(model).map_err(|error| error.to_string())?;
    let provider = OpenAiProvider::build(config, model, stderr_warnings())
        .map_err(|error| error.to_string())?;
    let profile = config
        .resolve_model(Some(model))
        .map_err(|error| error.to_string())?
        .1
        .name
        .clone();
    let mut session_config = config
        .session_config(model)
        .map_err(|error| error.to_string())?;
    // 档位查的是刚拿到的那张能力表 —— 新模型不认的这一档在这里就落回「默认」，而不是留到回合
    // 中途由适配器告警丢掉。
    if let Some(wanted) = effort {
        session_config.params.reasoning_effort =
            caps.reasoning_efforts.contains(&wanted).then_some(wanted);
    }
    session_config.carried_tokens = carried_tokens;
    Ok(Retargeted {
        provider: Arc::new(provider),
        config: session_config,
        profile,
    })
}

/// [`retarget_to`] 的产物。`profile` 是**发言者名字的来源**（spec §4）。
pub(crate) struct Retargeted {
    pub(crate) provider: Arc<dyn crate::provider::Provider>,
    pub(crate) config: crate::config::SessionConfig,
    pub(crate) profile: String,
}

/// 模型候选（spec §8）：`config.toml` 里**显式配置过**的那些 provider 名下的模型（有密钥）。
///
/// 两条判据都要：
///
/// * **配置里写过的** —— [`Config::configured_providers`]。五个内建 profile 无论有没有被配置都在
///   （`config.toml` 只做覆盖，注释掉 `[providers.kimi-code]` 只是不给覆盖），所以「我配了哪几家」
///   只能另记一份。2026-10-08 维护者定的：注释掉一段就当它不存在。
/// * **有密钥** —— 配置了但没给 key 的那条切过去起不来，那属于配置诊断而不属于这个浮层。
///
/// **当前模型兜底**：一份什么模型段都没写的配置里，上面的两条会列出空清单 —— 而清单里看不到自己
/// 正在用的那个模型是最刺眼的一种不诚实，所以它无条件在列。
///
/// 顺序是 `BTreeMap` 的键序，不是插入序，所以两次列出来的读法一致。
pub(crate) fn model_picker_options(config: &Config, current: &str) -> Vec<PickerOption> {
    config
        .models
        .iter()
        .filter(|(id, model)| {
            *id == current
                || (config.configured_providers.contains(&model.provider)
                    && config
                        .providers
                        .get(&model.provider)
                        .is_some_and(|profile| profile.api_key.is_some()))
        })
        .map(|(id, model)| PickerOption {
            label: id.clone(),
            detail: render::wording::model_detail_profile(&model.provider),
            current: id == current,
            enabled: true,
        })
        .collect()
}

/// 档位候选（spec §8）：这一档（`current: true`）与「默认」（spec §5）。
///
/// **模型自己的档位表**决定有什么可切，所以两家厂商的五档与三档各自不同。表是空的
/// （`MiniMax-M3`、`kimi-for-coding-highspeed`）时给**空清单**：那一格显示 `固定`，前端点它给
/// 一句说明 —— 一个只有「默认」可选的清单会读成「有东西可切」，而那里其实什么也没有。
pub(crate) fn effort_picker_options(
    model: &str,
    current: Option<ReasoningEffort>,
) -> Vec<PickerOption> {
    let Ok(caps) = caps_for(model) else {
        return Vec::new();
    };
    if caps.reasoning_efforts.is_empty() {
        return Vec::new();
    }
    let mut options = vec![PickerOption {
        label: render::wording::effort_default().to_owned(),
        detail: render::wording::effort_default_detail().to_owned(),
        current: current.is_none(),
        enabled: true,
    }];
    // 能力表里那张表已经是弱到强排好的，所以画出来的顺序就是强度顺序。
    options.extend(caps.reasoning_efforts.iter().map(|effort| PickerOption {
        label: effort.as_str().to_owned(),
        detail: String::new(),
        current: current == Some(*effort),
        enabled: true,
    }));
    options
}

/// 当前模型认不认这一档。`Err` 那句把它的档位列全 —— 换模型才可能得到它。
pub(crate) fn effort_supported(model: &str, effort: ReasoningEffort) -> Result<(), String> {
    let caps = caps_for(model).map_err(|error| error.to_string())?;
    if caps.reasoning_efforts.contains(&effort) {
        return Ok(());
    }
    Err(render::wording::effort_unknown_for(
        model,
        &caps
            .reasoning_efforts
            .iter()
            .map(|known| known.as_str())
            .collect::<Vec<_>>(),
    ))
}

/// 档位在回执与状态行上的读法。`None` 是「默认」那一档，它**不是**缺省值（spec §5）。
pub(crate) fn effort_label(effort: Option<ReasoningEffort>) -> String {
    effort
        .map(|effort| effort.as_str().to_owned())
        .unwrap_or_else(|| render::wording::effort_default().to_owned())
}

/// 一次切换的正文：换 provider 与配置、推一条新事实回去（spec §3）。
pub(crate) fn apply_switch(
    harness: &mut Harness,
    console: &ConsoleHandle,
    config: &Config,
    model: &str,
    effort: Option<ReasoningEffort>,
) -> Result<crate::Switched, String> {
    let retargeted = retarget_to(config, model, effort, harness.carried_tokens())?;
    let switched =
        harness.switch_model(retargeted.provider, retargeted.config, &retargeted.profile);
    console.session_update(
        switched.model.clone(),
        switched.effort,
        switched.context_window,
        switched.speakers.clone(),
    );
    Ok(switched)
}

/// 点状态行或按 `ctrl-t` 之后的那条路（spec §7）：算候选 → 发清单 → 等答案 → 切。
///
/// `running` 是忙闲判据（§6），由调用点给 —— 它就是「这个手势到达时有没有东西在跑」这
/// 一件事，运行中给一句回执、空闲才真的开清单。四个入口共用这一个动作，所以「换完之后
/// 下一次请求长什么样」只有一个答案。
///
/// 人取消了（`Esc` 或框外点击）就什么都不发生 —— 一次取消不是一次切换，不给回执。
pub(crate) async fn open_picker(
    harness: &mut Harness,
    console: &ConsoleHandle,
    config: &Config,
    kind: PickerKind,
    running: bool,
) {
    if running {
        harness.notice(&render::wording::heng(render::wording::switch_busy()));
        return;
    }
    let (title, options) = match kind {
        PickerKind::Model => (
            render::wording::picker_title_model(),
            model_picker_options(config, harness.model()),
        ),
        PickerKind::Effort => (
            render::wording::picker_title_effort(),
            effort_picker_options(harness.model(), harness.effort()),
        ),
    };
    let Some(index) = console.picker(title, options).await else {
        return;
    };
    // 清单**现算一遍**（不是复用发出去的那份）：等待期间人可能已经用 `/model` 换过模型，
    // 于是档位那张表已经不同了 —— 拿旧表去切会切到一个新模型不认的档位。判据同样现读
    // `harness`：那才是此刻这个会话真正的模型与档位。
    let fresh = match kind {
        PickerKind::Model => model_picker_options(config, harness.model()),
        PickerKind::Effort => effort_picker_options(harness.model(), harness.effort()),
    };
    let picked = fresh.get(index);
    let Some(choice) = picked else {
        return;
    };
    // 禁用行点了没反应（前端那一层就已经不响应）；这里再判一次是为了不让一条别人的清单
    // 走成切换。
    if !choice.enabled {
        return;
    }
    switch_to(harness, console, config, choice.label.clone(), kind);
}

/// 切换失败时把那句话给出去 —— 「组不出 provider」「这一档模型不认」都在这儿变成一句提示行。
pub(crate) fn report(harness: &mut Harness, outcome: Result<(), String>) {
    if let Err(message) = outcome {
        harness.notice(&render::wording::heng(&message));
    }
}

/// 切换本身，外加回执。两个命令与两个手势都到这里。
pub(crate) fn switch_to(
    harness: &mut Harness,
    console: &ConsoleHandle,
    config: &Config,
    chosen: String,
    kind: PickerKind,
) {
    // 换模型要把**当前档位**带着走（换档不需要 —— 它自己就是那一档），所以判据现读。
    let receipt = match kind {
        PickerKind::Model => {
            let before = harness.effort();
            switch_to_model(harness, console, config, &chosen, before)
        }
        PickerKind::Effort => switch_to_effort(harness, console, config, &chosen),
    };
    if let Err(message) = receipt {
        harness.notice(&render::wording::heng(&message));
    }
}

/// 换模型。档位照旧带着走 —— 人刚选的那一档不该因为换了模型而丢；新模型不认它时它落回
/// 「默认」，而那件事要说出来（`Switched::effort` 就是它）。
pub(crate) fn switch_to_model(
    harness: &mut Harness,
    console: &ConsoleHandle,
    config: &Config,
    model: &str,
    before: Option<ReasoningEffort>,
) -> Result<(), String> {
    let switched = apply_switch(harness, console, config, model, before)?;
    let mut receipt = render::wording::switched_model(&switched.model);
    if switched.effort != before {
        receipt.push_str(&format!(
            "，{}",
            render::wording::switched_effort(&effort_label(switched.effort))
        ));
    }
    harness.notice(&receipt);
    Ok(())
}

/// 换档。**只**改 `SessionConfig::params` —— provider 不读它（`build_body` 只从
/// `ChatRequest.params` 读），所以这是一个纯配置动作，不需要重建 provider；把它当成与换
/// 模型同一件事，是这里最容易有的误解。
pub(crate) fn switch_to_effort(
    harness: &mut Harness,
    console: &ConsoleHandle,
    config: &Config,
    label: &str,
) -> Result<(), String> {
    let effort = if label == render::wording::effort_default() {
        None
    } else {
        let Some(effort) = ReasoningEffort::from_token(label) else {
            return Err(format!(
                "`{label}` 不是一档思考强度；{}",
                render::wording::effort_usage()
            ));
        };
        Some(effort)
    };
    let model = harness.model().to_owned();
    let switched = apply_switch(harness, console, config, &model, effort)?;
    harness.notice(&render::wording::switched_effort(&effort_label(
        switched.effort,
    )));
    Ok(())
}

/// 一个会话要**宿主表**替它办的事（票 20、票 21）。
///
/// 宿主表在主循环手里，而输入也在主循环手里（决策票 04 §3）—— 所以点会话页、新建与关闭都由
/// 那个循环自己转成一条 [`CliRequest`]，由它自己的请求分支接住（见 [`front_to_request`]）。
/// 两边都想拿那张表的那条老路已经拆掉了。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CliRequest {
    /// 切到这个会话（不在表里就先装载）。
    Open(SessionId),
    /// 同 cwd 再加载一个空会话，并切过去（票 21）。`/clear` 走的是这一条。
    New,
    /// **阈值翻页**（票 21 的最后一条）：新建一个会话，并把这一趟的额度、目标与压缩摘要交接过去。
    ///
    /// 它与 `New` 的差别只有「交接什么」：翻页是同一个目标在新会话里接着跑，`/clear` 是清场。
    Rollover {
        carry_tokens: u64,
        goal: String,
        summary: Option<String>,
    },
    /// 把这个会话从宿主表里卸下来（盘上留着）。
    Close(SessionId),
}

/// 关掉一个会话（票 21 的规则、票 06）。
///
/// 三条：正在跑回合拒绝、空闲会话单击即关不二次确认、只读预览也能关。
/// **前台也关得掉**（票 06）：渲染层先切到相邻的那一个、再关它（两条请求，顺序固定）。
/// 它不属于装载（票 34）：关是把一行从宿主表里卸下来，装是把它放进去。
pub(crate) async fn close_session(
    host: &mut SessionHost<CliCommand>,
    kit: &AssemblyKit,
    console: &ConsoleHandle,
    id: &SessionId,
) {
    if let Err(refusal) = crate::session::close_check(host, id) {
        match &refusal {
            // 正在跑的那一个：走**提示行**，并给一段可点的「切到「<相邻会话>」」—— 切过去好
            // 在那儿按 ctrl-c 取消（票 06：前台不再被拒，这条提示行是给「在跑」用的）。
            crate::session::CloseRefusal::Running(target) => {
                console.close_blocked(target.clone());
            }
            // 不在表里：进转录说一句 —— 动作层面的说明，没有可点的下一步。
            _ => kit.notice(&host.foreground(), &refusal.to_string()),
        }
        return;
    }
    if let Err(error) = host.close_with(id, CliCommand::Close).await {
        kit.notice(&host.foreground(), &error.to_string());
        return;
    }
    let empty = host.is_empty();
    // 渲染器从下一次宿主事实快照里看到「它不在装载列表里」：切片状态、读数与空宿主态都由
    // 那一份 `HostFacts` 推导（票 03）。关到最后一个会话**不打 notice**（票 32）：页顶
    // `已装载 0 个`、页区 `没有会话`、输入区禁用已经在说同一件事。
    let _ = empty;
}

/// 一个会话自己的交互循环。
///
/// 这段代码以前是 `interactive_loop(&mut harness, …)`，由 `interactive` 直接驱动；现在它是
/// [`CliActor`] 的一次 `Run`，而 `Harness` 归宿主表的那个 task
/// （`.scratch/session-tab/spec.md` §1）。**行为一个字不变。**
///
/// 它为什么整段在 task 里、而不是「主循环解析提交、task 只跑回合」：前端事件接收端是**单一
/// 消费者**，而这一段的每一处都在与它打交道 —— 提示行、运行中的手势、目标循环。
pub(crate) struct CliActor {
    pub(crate) console: render::ConsoleHandle,
    pub(crate) config: std::sync::Arc<Config>,
    pub(crate) goals: GoalSetup,
    pub(crate) mcp: std::sync::Arc<McpService>,
    pub(crate) prompts: std::sync::Arc<Vec<McpPromptEntry>>,
    /// 只读预览（票 18）：半读的流装起来只能看 —— 凡写流的一律禁。
    pub(crate) read_only: bool,
    /// 要宿主表办的事（票 20 的换会话）。
    pub(crate) requests: tokio::sync::mpsc::UnboundedSender<CliRequest>,
    /// 「已经有一个 loop 在跑」那条边界读的状态（原 `interactive_loop` 的局部量）。
    pub(crate) loop_running: bool,
    /// 退出账本（同上）。
    pub(crate) quit: ExitRequest,
}

/// 主循环能对一个会话做的事（`.scratch/session-tab/spec.md` §1、决策票 04 §3）。
///
/// **输入归主循环、执行归 task**：前端事件接收端是单一消费者，只有一处读得动它，所以主循环
/// 读一次提交、解析成 `Submission`，再把它交给宿主表里**前台**那个会话执行。切换前台于是只是
/// 「换一个命令的收件人」。
pub(crate) enum CliCommand {
    /// 一次前台提交。`raw` 是那一行**原文** —— 「这一条提交会不会在流上留痕」要读它
    /// （`.scratch/command-echo/spec.md`）。
    Submit {
        submission: Box<Submission>,
        raw: String,
    },
    /// 运行中的一个手势。`running` 是主循环那一侧读到的忙闲（渲染器不知道什么在跑）。
    Gesture { event: FrontEndEvent, running: bool },
    /// 接着推进上次没收尾的目标（`--continue` 那一路，§10）。
    ResumeGoal,
    /// 翻页交接（票 21）：接住上一趟的额度、目标与压缩摘要，然后接着跑。
    Rollover {
        carry_tokens: u64,
        goal: String,
        summary: Option<String>,
    },
    /// 结束这个会话。
    Close,
}

/// 把一条前端手势转给**当前前台**那个会话（票 04 §3：输入只有一处消费者，手势从这里转发）。
pub(crate) fn forward_gesture(host: &SessionHost<CliCommand>, event: FrontEndEvent) {
    let running = host.running() > 0;
    if let Some(entry) = host.get(&host.foreground()) {
        let _ = entry.cmd.try_send(CliCommand::Gesture { event, running });
    }
}

/// 一条前端手势在主循环里变成哪一条宿主表请求（`.scratch/session-tab/spec.md` §1、§3）。
///
/// 会话级的那三条 —— 切一个、新建、关一个会话 —— 要动的是**宿主表**，而宿主表就在这个循环里，
/// 所以它们**就地**转成一条 [`CliRequest`]，由上面那个请求分支接住。
///
/// 它们曾经绕一圈：主循环把它们转发给前台那个会话，那个会话的 `CliActor::gesture` 再转回来
/// （票 20 的实现记录）。输入归主循环之后那一圈不再必要，而它有两个代价：前台那个会话正在跑
/// 回合时手势落进回合的 `select` 里被丢掉，于是「切换不因为目标在跑回合而阻挡」那条验收
/// 并不成立；更要紧的是 `OpenSession` 一旦在主循环这边被一个空分支吃掉，那条圈就再也没有别的
/// 路 —— 点会话页上的一行于是什么都不发生。
pub(crate) fn front_to_request(event: &FrontEndEvent) -> Option<CliRequest> {
    match event {
        FrontEndEvent::OpenSession { id } => Some(CliRequest::Open(id.clone())),
        FrontEndEvent::NewSession => Some(CliRequest::New),
        FrontEndEvent::CloseSession { id } => Some(CliRequest::Close(id.clone())),
        _ => None,
    }
}

#[async_trait::async_trait]
impl crate::session::SessionActor<CliCommand> for CliActor {
    async fn act(
        &mut self,
        harness: &mut Harness,
        command: CliCommand,
        inbox: &mut tokio::sync::mpsc::Receiver<CliCommand>,
    ) -> bool {
        match command {
            CliCommand::Submit { submission, raw } => {
                self.execute(harness, *submission, &raw, inbox).await;
                true
            }
            CliCommand::Gesture { event, running } => {
                self.gesture(harness, event, running).await;
                true
            }
            CliCommand::ResumeGoal => {
                self.resume_goal(harness, inbox).await;
                true
            }
            CliCommand::Rollover {
                carry_tokens,
                goal,
                summary,
            } => {
                // 额度不随翻页重置（§8）：把到现在为止属于这个目标的用量填给新会话。
                harness.carry_usage(carry_tokens);
                if let Some(summary) = &summary
                    && let Err(error) =
                        harness.inject_context(crate::events::ContextSource::Compaction, summary)
                {
                    harness.notice(&format!("heng: {}", render::wording::error_report(&error)));
                }
                match harness.select_goal(&goal) {
                    Err(error) => {
                        harness.notice(&format!("heng: {}", render::wording::error_report(&error)))
                    }
                    // 认领好了就接着跑那一趟目标 —— 清单与提醒由循环自己摆。
                    Ok(()) => self.resume_goal(harness, inbox).await,
                }
                true
            }
            CliCommand::Close => false,
        }
    }
}

impl CliActor {
    /// 运行中的一个手势（原 `interactive_loop` 的 `events.recv()` 那几支）。
    async fn gesture(&mut self, harness: &mut Harness, event: FrontEndEvent, running: bool) {
        match event {
            // 一次运行中途有人举手退出时，那个回合（或那次讨论、那趟目标循环）已经拿到它的
            // 收尾；这里只把「取消」送到。
            FrontEndEvent::Cancel | FrontEndEvent::Quit => harness.cancel_signal().cancel(),
            FrontEndEvent::CycleMode => {
                if self.read_only {
                    harness.notice(&render::wording::heng(&render::wording::read_only_session()));
                } else {
                    harness.mode_cycle().cycle();
                }
            }
            FrontEndEvent::OpenPicker(kind) => {
                open_picker(harness, &self.console, &self.config, kind, running).await;
            }
            FrontEndEvent::SessionTitled { id, title } => {
                if id == *harness.session_id()
                    && let Err(error) = harness.set_title(&title)
                {
                    harness.notice(&format!("heng: {}", render::wording::error_report(&error)));
                }
            }
            // 换会话、新建与关闭**不归这里**：那三条要动宿主表，而宿主表在主循环手里，
            // 所以主循环收到它们时就地转成了请求（`front_to_request`）—— 曾经由这里转回去，
            // 而那圈路让「前台正在跑回合时切不过去」。
            FrontEndEvent::NewSession | FrontEndEvent::OpenSession { .. } => {}
            FrontEndEvent::CloseSession { .. } => {}
            // 未读的清除也归主循环（它持那本账）。
            FrontEndEvent::AttentionSeen => {}
        }
    }

    /// 恢复（§10）。只对**重新打开**的会话做，而且只对认领过目标的那些。
    async fn resume_goal(
        &mut self,
        harness: &mut Harness,
        inbox: &mut tokio::sync::mpsc::Receiver<CliCommand>,
    ) {
        let Some(name) = harness.current_goal() else {
            return;
        };
        match harness.resume_goal() {
            // 正常收尾：回来是空闲等人 —— 人主动停是人的意思，该尊重它。
            None => harness.notice(&render::wording::heng(
                &render::wording::resumed_closed_goal(&name),
            )),
            // 异常中断：接着跑，不必人点头。
            Some(name) => {
                harness.notice(&render::wording::heng(&render::wording::resumed_goal(
                    &name,
                )));
                self.console.set_running(true);
                if let Err(message) = run_goal_loop(
                    harness,
                    &self.console,
                    inbox,
                    &self.requests,
                    &self.goals,
                    &name,
                    &mut self.loop_running,
                    &mut self.quit,
                )
                .await
                {
                    harness.notice(&render::wording::heng(&message));
                }
            }
        }
    }

    /// 执行一次提交 —— 原 `interactive_loop` 的 `match submission` 那一段。
    async fn execute(
        &mut self,
        harness: &mut Harness,
        submission: Submission,
        raw: &str,
        inbox: &mut tokio::sync::mpsc::Receiver<CliCommand>,
    ) {
        // 命令在转录里留一条**原文**（`.scratch/command-echo/spec.md`）：判据是「这一条提交
        // 会不会变成一句 user 消息」。记在执行之前，所以 `/clear` 自己的那一条留在**这个会话**
        // 会话里。
        if records_itself(&submission)
            && let Err(error) = harness.record_command_run(raw.trim_end())
        {
            harness.notice(&format!("heng: {}", render::wording::error_report(&error)));
        }
        match submission {
            // 退出由主循环处理：它不发下来。
            Submission::Quit => {}
            // 一个空行：没有可答的，于是再问一次。提示处返回 `None` 是唯一结束输入的东西，而那种情
            // 况就在上面处理。
            Submission::Ignore => {}
            Submission::Undo => {
                let receipt = undo_receipt(harness.undo_last_edit().await);
                harness.notice(&receipt);
            }
            // 一次模板调用：收参数（命令行上的位置参数 + 缺的用问卷问）→ `prompts/get` → 那段
            // 文本**就是这一轮的输入**（server 渲染出来的那条消息）。
            Submission::McpPrompt {
                server,
                prompt,
                inline,
            } => {
                match run_mcp_prompt(
                    &self.console,
                    &self.mcp,
                    &self.prompts,
                    &server,
                    &prompt,
                    &inline,
                )
                .await
                {
                    Ok(Some(text)) => {
                        if let Err(error) =
                            run_one_turn(harness, inbox, TurnStart::Prompt(&text), &mut self.quit)
                                .await
                        {
                            harness.notice(&format!(
                                "heng: {}",
                                render::wording::error_report(&error)
                            ));
                        }
                    }
                    // 人把这次询问丢掉了：什么都不发生，回到提示行。
                    Ok(None) => {}
                    Err(message) => harness.notice(&format!("heng: {}", message)),
                }
            }
            Submission::Unknown(line) => {
                let names = harness.skill_names();
                harness.notice(&format!(
                    "heng: {}",
                    render::wording::unknown_command(&line, &names)
                ));
            }
            // `/<skill> [task]` 是用户侧的技能调用（spec §9）：`disable-model-invocation: true`
            // 的技能为用户留下的那条路。正文进上下文尾部。光秃秃的 `/<skill>` 就地跑 —— 正文**就
            // 是**指令 —— 而打了任务时，**整条原文**（技能名在内）作为一条普通 user 消息跟在它
            // 后面（`.scratch/ui-trim/spec.md` §2）。
            Submission::Skill { name, task, bare } => {
                if bare {
                    harness.notice(&format!("heng: {}", render::wording::skill_started(&name)));
                    if let Err(error) =
                        run_one_turn(harness, inbox, TurnStart::Skill(&name), &mut self.quit).await
                    {
                        harness.notice(&format!("heng: {}", render::wording::error_report(&error)));
                    }
                } else if let Err(error) = harness.load_skill(&name) {
                    harness.notice(&format!("heng: {}", render::wording::error_report(&error)));
                } else {
                    harness.notice(&format!("heng: {}", render::wording::skill_loaded(&name)));
                    if let Err(error) =
                        run_one_turn(harness, inbox, TurnStart::Prompt(&task), &mut self.quit).await
                    {
                        harness.notice(&format!("heng: {}", render::wording::error_report(&error)));
                    }
                }
            }
            // `/discuss [问题]`：在**当前会话的流上**的一次讨论（spec §15）。讨论者是这个会话的兄
            // 弟，所以继承它的上下文，并把轮次追加到它的日志上；之后用户回到提示符前。
            Submission::Discuss(question) => {
                if let Err(error) =
                    discuss_in_session(harness, inbox, &self.config, question, &mut self.quit).await
                {
                    harness.notice(&format!("heng: {}", render::wording::error_report(&error)));
                }
            }
            // `/goal-new <名字> <来源>…`：从一批票生成一份目标清单（§2）。手势，不进流。
            Submission::Goal(args) => {
                let message = run_goal_command(harness, self.goals.dir.as_deref(), &args);
                harness.notice(&render::wording::heng(&message));
            }
            // `/clear`：结束当前会话、开一个新的（§12）。它复用翻页那条机制 —— 两段入口、
            // 一段机制 —— 只是不带压缩、不带摘要注入：人是主动清场，没有「要带过去的历史」
            // 这回事。
            Submission::Clear => {
                // `/clear` 与阈值 rollover 与页签 `新建` 是**同一个动作**（票 21）：同 cwd 再
                // 加载一个空会话、切过去，旧的留在宿主表里。**闸门取消** —— 旧会话不再被换
                // 掉，拦它没有理由。
                let _ = self.requests.send(CliRequest::New);
            }
            Submission::Loop(args) => {
                let message = match parse_loop_line(&args) {
                    Err(message) => message,
                    Ok(name) => {
                        let result = run_goal_loop(
                            harness,
                            &self.console,
                            inbox,
                            &self.requests,
                            &self.goals,
                            &name,
                            &mut self.loop_running,
                            &mut self.quit,
                        )
                        .await;
                        result.err().unwrap_or_default()
                    }
                };
                if !message.is_empty() {
                    harness.notice(&render::wording::heng(&message));
                }
            }
            // `/model <id>`：换模型。**不带参数不给开弹窗而是说一句用法** —— 选择器是 TUI
            // 独占的交互，而这一路必须能一步到位（plain 前端就靠它换模型，spec §7）。
            Submission::Model(args) => {
                let model = args.trim();
                if model.is_empty() {
                    harness.notice(&render::wording::heng(render::wording::model_usage()));
                } else if let Err(message) = switch_to_model(
                    harness,
                    &self.console,
                    &self.config,
                    model,
                    harness.effort(),
                ) {
                    harness.notice(&render::wording::heng(&message));
                }
            }
            // `/effort <档>`：换档。这一条到达时一定**空闲**（提交只在回合之间被读），所以
            // 不必判忙闲 —— 忙闲是那三个运行中到达的入口的事。
            Submission::Effort(args) => {
                let arg = args.trim();
                if arg.is_empty() {
                    harness.notice(&render::wording::heng(&render::wording::effort_usage()));
                } else if let Some(effort) = ReasoningEffort::from_token(arg) {
                    // 当前模型认不认这一档 —— 换模型才可能得到它，而「它有哪几档」本身就是答案的
                    // 一半，所以这句说清，而不是默默落回「默认」。
                    match effort_supported(harness.model(), effort) {
                        Err(message) => harness.notice(&render::wording::heng(&message)),
                        Ok(()) => {
                            let label = effort.as_str();
                            let outcome =
                                switch_to_effort(harness, &self.console, &self.config, label);
                            report(harness, outcome);
                        }
                    }
                } else {
                    // `默认` 与「不是一档」都由 [`switch_to_effort`] 自己判、自己说 —— 选择器那条
                    // 路与它共用同一段判断，所以命令这一层不重写一遍。
                    let outcome = switch_to_effort(harness, &self.console, &self.config, arg);
                    report(harness, outcome);
                }
            }
            // 其余的都是 prompt，含换行：转录把它显示成用户写下的那一条消息（spec §12）。
            Submission::Prompt(text) => {
                if let Err(error) =
                    run_one_turn(harness, inbox, TurnStart::Prompt(&text), &mut self.quit).await
                {
                    harness.notice(&format!("heng: {}", render::wording::error_report(&error)));
                }
            }
        }
    }
}

/// 这一条提交会不会在流上**一个字都不留**，因而需要另记一条命令记录
/// （`.scratch/command-echo/spec.md`）。
///
/// 判据只有一条：提交之后有没有一句是用户说的。`Prompt` 与**带任务**的 `/<skill>` 都会把
/// 原文送进模型上下文，转录里已经有了（那是气泡，模型也读得到），再记一遍只会重复；其余那几
/// 条 —— 内建命令、MCP 模板、**裸**技能、以及一个错字 —— 都由循环自己执行，所以流上找不到
/// 「我刚才敲了什么」。
///
/// 空行与退出不记：前者什么都不是，后者在人离开的那一刻写下一个动作没有意义。
pub(crate) fn records_itself(submission: &Submission) -> bool {
    match submission {
        Submission::Ignore | Submission::Quit | Submission::Prompt(_) => false,
        // 技能是唯一一个「名字本身就是这句话的一部分」的记号，所以光秃秃的一条另记；
        // 打了任务的那条**整条原文**已经是 user 消息（`.scratch/ui-trim/spec.md` §2）。
        Submission::Skill { bare, .. } => *bare,
        _ => true,
    }
}

/// 一个记号两侧的文字拼成任务：记号被抹掉，**前文（含它前面的每一行）**与后文接起来。
///
/// `请 /loop 我的目标` → `请 我的目标`；`第一行\n/loop 目标` → `第一行\n目标`（记号与后文
/// 原来在同一行时用一个空格接上，跨行时保留那个换行）。只丢结尾的空白行，所以一段全是空白
/// 的续行不算任务 —— 与旧 `task_of(inline, rest)` 对「记号之后」那半边的规则逐字相同
/// （`.scratch/input-tokens/spec.md` §5）。
pub(crate) fn task_around(text: &str, token: &Token) -> String {
    let (start, end) = token_bytes(text, token);
    let before = &text[..start];
    let after = &text[end..];
    let head = before.trim_end();
    let tail = after.trim_start().trim_end_matches('\n');
    let mut task = head.to_owned();
    if !tail.trim().is_empty() {
        if !task.is_empty() {
            task.push(if before.ends_with('\n') { '\n' } else { ' ' });
        }
        task.push_str(tail);
    }
    task
}

/// 一个记号在 `text` 里的字节范围 —— 记号的字符下标换算一次就够，两处切片都用它。
pub(crate) fn token_bytes(text: &str, token: &Token) -> (usize, usize) {
    (byte_offset(text, token.start), byte_offset(text, token.end))
}

/// 字符下标 → 字节偏移。记号的下标是字符意义上的（`token` 模块），而切片要字节。
pub(crate) fn byte_offset(text: &str, at: usize) -> usize {
    text.char_indices()
        .nth(at)
        .map(|(index, _)| index)
        .unwrap_or(text.len())
}

/// 一次提交在要什么（spec §12）。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Submission {
    /// 没有要发的东西：一个空行，或者去掉空白后是空的一行。循环不再起一个回合，直接再问一次 —— 在
    /// 空输入上按回车是空行，绝不是输入结束（spec §6）。
    Ignore,
    Quit,
    Undo,
    /// 第一行以 `/` 开头、却什么都没点名，而且后面只有空行：一个错字，也是唯一会被告诉用户的那种
    /// 情况。
    Unknown(String),
    /// `/<skill>` 以及跟在它后面的任务。
    ///
    /// `task` 是**整条原文**，技能名留在里面：`/research 帮我查 X` 发出去的就是这一条，于是
    /// 对话里读得出「这句话是怎么发出去的」（`.scratch/ui-trim/spec.md`）。它曾经是记号
    /// **两侧**的字拼起来的（`task_around`），技能名被丢掉。
    ///
    /// `bare` 说的是「整条草稿只有那一个记号」（`/research`，尾巴上的空白不算）：那时正文
    /// **就是**指令，不再多发一条 user 消息。技能是唯一一个「名字本身就是这句话的一部分」
    /// 的记号，所以内建命令与 MCP 模板的参数仍取记号旁边的字。
    Skill {
        name: String,
        task: String,
        bare: bool,
    },
    /// `/<server>:<模板>`：一次 MCP 提示词模板调用（票 17）。发起者是**人**：它走 `/` 菜单，
    /// 不进工具表。`inline` 是命令行上给的位置参数，缺的那些由循环用问卷补齐。
    McpPrompt {
        server: String,
        prompt: String,
        inline: String,
    },
    /// `/discuss [问题]`：在**当前会话的流上**跑一次讨论（spec §15）。用户没打问题时问题为空 ——
    /// 那时循环把这个会话最后一个问题交给那两位讨论者。
    Discuss(String),
    /// `/goal-new <名字> <来源> [--force]`：从一批票生成一份目标清单
    /// （`.scratch/goal-loop/spec.md` §2）。它是一个**手势**，不是工具调用：由循环直接执行，
    /// 不走工具表、不走权限门、也不进事件流 —— 清单文件存在即是「目标已创建」的证据。
    Goal(String),
    /// `/loop <名字>`：选定目标并连续工作（§4）。参数是目标的名字，一个不断开的词。
    Loop(String),
    /// `/model <model id>`：换这个会话用的模型（spec §7）。缺参数是**用法错误**而不是
    /// 「打开选择器」—— 选择器是 TUI 独占的交互，命令这一路必须能一步到位（plain 前端就靠
    /// 它换模型）。
    Model(String),
    /// `/effort <档位>`：换思考强度（spec §7）。参数可以是五个档位之一或 `默认`（= 不发这个
    /// 参数）。缺参数同样是用法错误。
    Effort(String),
    /// `/clear`：结束当前会话、开一个新的（`.scratch/goal-loop/spec.md` §12）。**不是**「清空
    /// 上下文继续用」—— 旧会话留在磁盘上，进程不重启，渲染器与终端留着。
    Clear,
    /// 整条提交，含换行，作为一条 prompt。
    Prompt(String),
}

/// 读一次提交。
///
/// 一条命令是**整个草稿里**任意行、任意位置上**最靠左**的那个 `/name`，其中 `name` 在命令表里
/// （内建命令、技能、MCP 模板的并集）。记号前面的字（含它前面的每一行）与它后面的字**一起**成为
/// 任务，所以 `请 /loop 我的目标` 读作 `Loop("请 我的目标")` —— 内建命令过去那条「要么整条、要么
/// 什么都不算」的规矩要防的是丢用户写下的字，而这条规则同样一个字都不丢
/// （`.scratch/input-tokens/spec.md` §5）。
///
/// **这一档是有牙的**（访谈里明码标价选下的）：`@` 被误认只是个引用，而 `/` 被误认**会真的
/// 执行** —— `用 /loop 做目标循环` 就是一条命令。别自作主张加一条「整行起头」的守卫。
///
/// 无参命令（`/quit`/`/exit`/`/undo`/`/clear`）仍要求**整条只有它**，于是「请把 `/clear` 加到文档
/// 里」是一条普通消息、不会被劫持成一次清空。草稿里一个命令记号都没有时整条仍当普通消息；唯一的
/// 例外是一个以 `/` 开头、占满整行却不认识的名字 —— 那是错字，会告诉用户。
///
/// 去掉空白后为空的提交是 [`Submission::Ignore`]：没有要发的东西，所以循环再问一次，而不是起一个回
/// 合。
///
/// 开头的斜杠一律剥掉（`//undo` 读作 `undo`）。任务结尾的空白行丢掉；其余按原样保留。
pub(crate) fn submission(
    text: &str,
    has_skill: impl Fn(&str) -> bool,
    prompts: &[McpPromptEntry],
) -> Submission {
    if text.trim().is_empty() {
        return Submission::Ignore;
    }
    let is_command = |name: &str| {
        render::wording::BUILT_IN_COMMANDS
            .iter()
            .any(|command| command.name == name)
            || has_skill(name)
            || prompts.iter().any(|entry| entry.name == name)
    };
    let token = token::tokens(text)
        .into_iter()
        .filter(|token| token.prefix == '/')
        .find(|token| is_command(command_name(text, token)));

    let Some(token) = token else {
        // 一个命令记号都没有：整条还是普通消息。唯一的例外是占满整行的一个不认识的名字 ——
        // 那是错字，一直会被告诉用户。
        if text.trim_start().starts_with('/') && !text.trim().contains('\n') {
            return Submission::Unknown(text.trim().to_owned());
        }
        return Submission::Prompt(text.to_owned());
    };

    let name = command_name(text, &token);
    // 整条草稿只有那**一个**记号（尾巴上的空白不算）。两类判断都从它出发：内建命令与 `bare`
    // 的技能调用都是「光秃秃的一个名字」。
    let only_token = text.trim().chars().count() == token.end - token.start;
    // 内建命令没有任务来装记号旁边的字，所以它们还多要一条：记号写成 `/<名字>`（没有多出来的
    // 斜杠 —— `//undo` 不算）。技能不受多出来那个斜杠的限制，名字本来就是剥掉斜杠之后才比的。
    let whole = only_token && token.query == name;
    if whole {
        match name {
            "quit" | "exit" => return Submission::Quit,
            "undo" => return Submission::Undo,
            "clear" => return Submission::Clear,
            _ => {}
        }
    }
    let task = task_around(text, &token);
    match name {
        // `/discuss [问题]`：这一条的问题就是记号旁边那一整段。光秃秃的 `/discuss` 把问题留给
        // 循环。
        "discuss" => Submission::Discuss(task),
        // `/goal-new <名字> <来源>`：记号旁边那一整段都是参数。`/goal` 这一族用**连字符**写成
        // 一条命令（将来的 `/goal-list` 等照走），于是 `/goal-` 一个前缀就能在 `/` 菜单里把它们
        // 全列出来 —— 空格形状的子命令补不出来。
        "goal-new" => Submission::Goal(task),
        // `/loop <名字>`：一个参数，形状与 `/goal-new` 相同。
        "loop" => Submission::Loop(task),
        // `/model <id>` 与 `/effort <档>`：换模型与换档。缺参数在这里**不**拦 —— 循环给一句
        // 用法，与 `parse_goal_new_line` 缺参数是同一个形状（`Err(())` 然后一句话）。
        "model" => Submission::Model(task),
        "effort" => Submission::Effort(task),
        _ => {
            // `/<server>:<模板>`：模板条目是运行时才知道的名字，所以这里问的是那份菜单
            // （票 17）。冒号是它与人打出来的技能名的分界。
            if let Some((server, prompt)) = name.split_once(':')
                && prompts.iter().any(|entry| entry.name == name)
            {
                return Submission::McpPrompt {
                    server: server.to_owned(),
                    prompt: prompt.to_owned(),
                    inline: task,
                };
            }
            if has_skill(name) {
                Submission::Skill {
                    name: name.to_owned(),
                    task: text.trim_end().to_owned(),
                    bare: only_token,
                }
            } else {
                // 一个内建命令带着记号之外的字：整条仍是一条普通消息，用户写下的东西一个字
                // 都不会被丢掉。
                Submission::Prompt(text.to_owned())
            }
        }
    }
}

/// 一个 `/` 记号的命令名：记号里那一段剥掉开头的斜杠（`//undo` 读作 `undo`）。
pub(crate) fn command_name<'a>(text: &'a str, token: &Token) -> &'a str {
    let (start, end) = token_bytes(text, token);
    text[start..end].trim_start_matches('/')
}

/// 还缺哪些**必填**参数。
pub(crate) fn missing_required_arguments<'a>(
    prompt: &'a mcp::PromptSummary,
    arguments: &serde_json::Map<String, serde_json::Value>,
) -> Vec<&'a str> {
    prompt
        .arguments
        .iter()
        .filter(|argument| argument.required && !arguments.contains_key(&argument.name))
        .map(|argument| argument.name.as_str())
        .collect()
}

/// 把一次 `/` 菜单里的模板调用跑完（票 17）。
///
/// 三步：命令行上的位置参数按声明顺序填进去 → 缺的用**同一个**问询端口逐项问 → `prompts/get`。
/// 返回的文本会成为这一轮的一条 `user` 消息；`Ok(None)` 表示人把这次询问丢掉了。
pub(crate) async fn run_mcp_prompt(
    console: &ConsoleHandle,
    mcp: &McpService,
    prompts: &[McpPromptEntry],
    server: &str,
    prompt: &str,
    inline: &str,
) -> Result<Option<String>, String> {
    let Some(entry) = prompts
        .iter()
        .find(|entry| entry.server == server && entry.prompt.name == prompt)
    else {
        // 正常情况下到不了这里：`submission` 只在菜单清单里有的名字上产生这个变体。留着是
        // 因为它是一句人话，而不是一次 panic。
        return Err(render::wording::mcp_prompt_unknown(server, prompt));
    };

    let mut arguments = prompt_inline_arguments(&entry.name, &entry.prompt, inline)?;

    // 缺的参数逐项问：走的是 `ask_user_question` 那条通道，不是第二套界面。
    let missing: Vec<mcp::PromptArgument> = entry
        .prompt
        .arguments
        .iter()
        .filter(|argument| !arguments.contains_key(&argument.name))
        .cloned()
        .collect();
    if !missing.is_empty() {
        let questions: Vec<UserQuestion> = missing
            .iter()
            .map(|argument| UserQuestion {
                id: argument.name.clone(),
                question: argument
                    .description
                    .clone()
                    .unwrap_or_else(|| argument.name.clone()),
                header: Some(entry.name.clone()),
                options: Vec::new(),
                multi_select: false,
            })
            .collect();
        let port = ConsoleQuestions::from_handle(console);
        let answers = match port.ask(&questions).await {
            Ok(answers) => answers,
            // 人把这次询问丢掉了（输入结束，或者这次运行被取消）：不发这次调用。
            Err(_) => return Ok(None),
        };
        for question in &questions {
            if let Some(answer) = answers
                .answers
                .iter()
                .find(|answer| answer.id == question.id)
                && let Some(value) = answer
                    .custom
                    .clone()
                    .or_else(|| answer.selected.first().cloned())
            {
                arguments.insert(question.id.clone(), serde_json::Value::String(value));
            }
        }
    }

    let missing_required = missing_required_arguments(&entry.prompt, &arguments);
    if !missing_required.is_empty() {
        return Err(render::wording::mcp_prompt_missing_arguments(
            &entry.name,
            &missing_required,
        ));
    }

    mcp.get_prompt(server, prompt, serde_json::Value::Object(arguments))
        .await
        .map(Some)
        .map_err(|error| error.to_string())
}

/// `/` 菜单的条目（`.scratch/goal-loop/spec.md` §13）：**内建命令在前、技能在后**。
///
/// 命令是程序自带的、技能是用户装的，所以固定项优先更可预测：打一个 `/c` 时 `/clear` 排在
/// 用户那个恰好也叫 `c…` 的技能前面。命令按声明的固定顺序（不按字母），技能按名字。
///
/// 每一条带着它的**类别**（`.scratch/tui-feedback/spec.md` §11）：三样东西共用一个菜单，
/// 而它们该看得出是三类 —— 名字借类别上色，次序与分派一个字不改。
///
/// 抽出这个纯函数是为了让这条次序有地方断言 —— 它原来内联在组装点里，只能靠真终端看。
pub(crate) fn slash_catalog(
    commands: &[render::wording::Command],
    skills: &[(&str, &str)],
    prompts: &[McpPromptEntry],
) -> Vec<render::CatalogEntry> {
    let mut skills: Vec<&(&str, &str)> = skills.iter().collect();
    skills.sort_by(|left, right| left.0.cmp(right.0));
    commands
        .iter()
        .map(|command| render::CatalogEntry::command(command.name, command.description))
        .chain(
            skills
                .into_iter()
                .map(|(name, description)| render::CatalogEntry::skill(*name, *description)),
        )
        // 菜单的**动态那一半**：MCP server 的提示词模板（票 17）。它不进工具表、也不进前缀
        // 缓存 —— 这里是它唯一的落点。
        .chain(
            prompts
                .iter()
                .map(|entry| render::CatalogEntry::template(&entry.name, entry.description())),
        )
        .collect()
}

/// 命令行上的位置参数按模板的声明顺序填进参数表；多出来的那几个是错误。
pub(crate) fn prompt_inline_arguments(
    display: &str,
    prompt: &mcp::PromptSummary,
    inline: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let mut arguments = serde_json::Map::new();
    let mut words = inline.split_whitespace();
    for argument in &prompt.arguments {
        let Some(word) = words.next() else { break };
        arguments.insert(
            argument.name.clone(),
            serde_json::Value::String(word.to_owned()),
        );
    }
    if words.next().is_some() {
        return Err(render::wording::mcp_prompt_too_many_arguments(display));
    }
    Ok(arguments)
}

/// `/` 菜单的动态那一半：连接与这一轮的模板条目。
///
/// 打包成一个值，只因为 `interactive_loop` 的参数已经够多了 —— 这两样总是成对出现。
/// `/` 菜单里的一条 MCP 模板：`/<server>:<模板名>`。
pub(crate) struct McpPromptEntry {
    pub(crate) server: String,
    pub(crate) prompt: mcp::PromptSummary,
    /// 菜单上那个**不带斜杠**的名字，与 `submission` 解析时看到的一样。
    pub(crate) name: String,
}

impl McpPromptEntry {
    pub(crate) fn new(server: String, prompt: mcp::PromptSummary) -> Self {
        let name = format!("{server}:{}", prompt.name);
        Self {
            server,
            prompt,
            name,
        }
    }

    /// 菜单第二列：模板自己的说明；它没写说明时，把参数名列一遍。
    fn description(&self) -> String {
        if let Some(description) = self.prompt.description.as_deref() {
            let description = description.trim();
            if !description.is_empty() {
                return description.to_owned();
            }
        }
        let names: Vec<&str> = self
            .prompt
            .arguments
            .iter()
            .map(|argument| argument.name.as_str())
            .collect();
        render::wording::mcp_prompt_description(&self.server, &names)
    }
}

/// 组装后拉一次模板清单 —— `/` 菜单的动态那一半。
///
/// **不在前缀缓存里**：它进的是菜单，不是工具表。失败的 server 直接跳过（它的模板条目根本
/// 不出现），而不是出现之后点开报错（spec §8）。
pub(crate) async fn mcp_prompt_entries(service: &McpService) -> Vec<McpPromptEntry> {
    service
        .prompt_entries()
        .await
        .into_iter()
        .map(|(server, prompt)| McpPromptEntry::new(server, prompt))
        .collect()
}

/// `/loop <名字>` 的一次运行（`.scratch/goal-loop/spec.md` §4）。
///
/// 四条启动边界在**写任何事件之前**判；通过之后先落一条 `GoalSelected`，把清单注入上下文，
/// 然后一个回合接一个回合地跑，每个**回合边界**上依次判：完成（§1、§11）→ 阈值（§6、§7）→
/// 无进展（§9），而额度那道闸门在每回合开头自己判（§8）。走到循环末尾只剩一个原因：手势。
///
/// `running` 是这个会话「有没有一个 loop 在跑」的状态，由调用方持有：它跨过整次运行，也正是
/// 第三条启动边界读的那个值。
///
/// `Err` 是一句说给人听的拒绝或失败。
///
/// 八个参数一起描述一次目标循环的入口（会话、控制台、命令、请求、目标目录、名字、运行位、
/// 退出举手）—— 它们各自来自不同的持有者，拆成一个结构只会把同一批搬家。clippy 的
/// 「参数太多」在这里是有意为之。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_goal_loop(
    harness: &mut Harness,
    console: &ConsoleHandle,
    inbox: &mut tokio::sync::mpsc::Receiver<CliCommand>,
    requests: &tokio::sync::mpsc::UnboundedSender<CliRequest>,
    goals: &GoalSetup,
    name: &str,
    running: &mut bool,
    quit: &mut ExitRequest,
) -> Result<(), String> {
    let Some(dir) = goals.dir.as_deref() else {
        return Err(render::wording::no_goal_dir().to_owned());
    };
    // 读不出来分两种：清单不在（提示先 `/goal-new`），或者它是人手改坏的文件（照说坏在哪）。
    let loaded = crate::goals::load(dir, name);
    let broken = match &loaded {
        Ok(_) | Err(crate::goals::GoalError::Read { .. }) => None,
        Err(error) => Some(error.to_string()),
    };
    let manifest = loaded.as_ref().ok();
    let progress = manifest.map(|manifest| goal_progress(goals, name, manifest));
    let complete = progress
        .as_ref()
        .is_some_and(crate::goals::Progress::is_complete);
    let unattended = harness.mode().allows_unattended();
    if let Err(refusal) = crate::goals::check_start(manifest, complete, *running, unattended) {
        return Err(match refusal {
            crate::goals::StartRefusal::AlreadyRunning => {
                render::wording::loop_already_running(name)
            }
            crate::goals::StartRefusal::Unattended => render::wording::loop_needs_unattended_mode(
                render::wording::mode_label(harness.mode()),
            ),
            crate::goals::StartRefusal::Unknown => {
                broken.unwrap_or_else(|| render::wording::loop_unknown_goal(name))
            }
            crate::goals::StartRefusal::NoWork => render::wording::loop_no_work(name),
        });
    }
    // 三条都过了：从那句提示开始，输入区就禁言了 —— 它由这只闩管着，离开作用域就解除，于是
    // 每一条返回路径都恢复得回来（§5）。
    let _muted = Muted::new(console);
    harness.notice(render::wording::input_muted());
    let manifest = manifest.expect("check_start 放行就意味着清单在");
    let progress = progress.expect("清单在就有进度");

    // 预算的口径（§8）：这个目标**别处**已经花掉的，先填进这个会话 —— 当前会话自己那份不算
    // 在里面，它就在自己这条流上。
    let current = harness.session_id().as_str().to_owned();
    harness.carry_usage(goal_usage(goals, name, &current));
    // 归属先落流：进度重算、预算与恢复都从它派生（§4、§8、§10）。
    if let Err(error) = harness.select_goal(name) {
        return Err(render::wording::error_report(&error));
    }
    // 清单是封闭的，所以模型得在开跑时就知道上面有哪些条目 —— 它不能往里加东西，只能按 id
    // 标完成（§1、§3）。
    if let Err(error) =
        harness.inject_context(crate::events::ContextSource::Goal, &manifest.render())
    {
        return Err(render::wording::error_report(&error));
    }
    harness.notice(&render::wording::goal_loop_started(
        name,
        progress.completed(),
        progress.total(),
    ));
    // 越界的 id 被忽略，但绝不静默（§3）。
    let mut reported_unknown: Vec<String> = Vec::new();
    report_unknown_goal_ids(harness, name, &progress.unknown, &mut reported_unknown);

    // 无进展的计数（§9）：连续几次翻页零条目完成。它从这一刻的进度起算。
    let mut no_progress = crate::goals::NoProgress::new(progress.completed());
    // provider 失败的重试预算（§9）。
    let mut retry = crate::goals::Retry::new(goals.settings.provider_retries);
    // 「这一档已经提醒过」这个跨回合的标记（§6）：跨过阈值只注入一次，翻页时复位。
    let mut reminded = false;
    *running = true;
    loop {
        if harness.cancel_signal().is_cancelled() {
            break;
        }
        let outcome = match run_one_turn(harness, inbox, TurnStart::Injected, quit).await {
            Ok(outcome) => outcome,
            // 连驱动都失败（会话 i/o，不是 provider）：这不是重试能修的那一类，所以照说、
            // 收工。流上没有收尾事件，于是 `--continue` 会把它读成一次异常中断 —— 对一次
            // i/o 失败来说正是该有的读法。
            Err(error) => {
                *running = false;
                return Err(render::wording::error_report(&error));
            }
        };
        // provider 调用失败：重试若干次，耗尽后停下并写一条收尾事件（§9）。
        if outcome.reason == StopReason::Error {
            if retry.another_attempt() {
                harness.notice(&render::wording::provider_retry(retry.failures()));
                tokio::time::sleep(RETRY_DELAY).await;
                continue;
            }
            *running = false;
            return stop_goal_loop(
                harness,
                name,
                crate::events::GoalStopReason::ProviderFailed,
                retry.failures(),
                manifest,
                goals,
            );
        }
        // 撞顶（§8）：沿用既有语义 —— **降级收尾，而不是中断**，并落一条收尾事件，于是恢复
        // （§10）分得出这一次与一次崩溃。
        if outcome.reason == StopReason::BudgetExhausted {
            *running = false;
            return stop_goal_loop(
                harness,
                name,
                crate::events::GoalStopReason::BudgetExhausted,
                0,
                manifest,
                goals,
            );
        }
        // 中途成功一次：计数归零。
        retry.succeeded();
        if harness.cancel_signal().is_cancelled() {
            break;
        }

        // 回合边界上重算一次进度（§1）：判据是机械的，循环自己判，不需要人点头。一个回合是一个
        // 不可分的步子，所以「每次 `todo` 落地之后」这一刻就是它结束的时候。
        //
        // 信任假设：`completed` 是**模型自己填的**，所以判据机械**不等于**结果可靠。
        let streams = goal_streams(goals, name);
        let calls: Vec<crate::goals::TodoCall> = streams
            .iter()
            .flat_map(|events| crate::goals::todo_calls(events))
            .collect();
        let progress = crate::goals::progress(&manifest.entries, &calls);
        // 运行中新冒出来的越界 id 也要有人告诉模型（§3）。
        report_unknown_goal_ids(harness, name, &progress.unknown, &mut reported_unknown);
        if progress.is_complete() {
            // 收尾汇总由一次模型调用写（§11），而那一次调用照记用量 —— 它落进目标预算。调用
            // 没成不改变「做完了」这个事实，缺的只是那段叙述，所以退回一份机械的说明。
            let notes: Vec<String> = streams
                .iter()
                .flat_map(|events| crate::goals::notes_of(events))
                .collect();
            if let Err(error) = harness
                .finish_goal(name, manifest, &progress, streams.len(), &notes)
                .await
            {
                *running = false;
                return Err(render::wording::error_report(&error));
            }
            harness.notice(&render::wording::goal_completed_notice(
                name,
                progress.completed(),
                progress.total(),
                streams.len(),
            ));
            *running = false;
            return Ok(());
        }

        // 阈值（§6、§7）：判据取**投影前**的估算，在回合边界比对 —— 不能在一个工具跑到一半
        // 的时候判。
        let percent = harness.context_percent();
        match crate::goals::threshold_step(
            percent,
            goals.settings.remind_at,
            goals.settings.compact_at,
            reminded,
        ) {
            crate::goals::ThresholdStep::None => {}
            crate::goals::ThresholdStep::Remind => {
                // 跨过阈值时注入**一次**，不是每轮：每轮注入会每轮打掉前缀缓存，而本仓库有
                // 「前缀只增不改」的不变量。
                if let Err(error) = harness.inject_context(
                    crate::events::ContextSource::Reminder,
                    render::wording::goal_reminder(),
                ) {
                    *running = false;
                    return Err(render::wording::error_report(&error));
                }
                reminded = true;
                harness.notice(&render::wording::goal_reminded(percent));
            }
            crate::goals::ThresholdStep::Compact => {
                // 压缩与翻页是一个动作（§7）。压缩照做 —— 它是**这一趟**的收尾；而**翻页**从
                // 「换掉这个会话」变成「新建一个会话、旧的留在宿主表里」（票 21）。
                let from = harness.session_id().as_str().to_owned();
                let summary = match harness.compact().await {
                    Ok(summary) => summary,
                    Err(error) => {
                        *running = false;
                        return Err(render::wording::error_report(&error));
                    }
                };
                // 无进展（§9）先在**这一趟**结算：连续 N 次翻页零条目完成就停下报告，不再开新
                // 会话。跨会话的那本账**不交接** —— 新会话从零开始数（票面记着这一条）。
                let after = goal_progress(goals, name, manifest);
                let streak = no_progress.after_rollover(after.completed());
                if no_progress.reached(goals.settings.no_progress_rollovers) {
                    *running = false;
                    return stop_goal_loop(
                        harness,
                        name,
                        crate::events::GoalStopReason::NoProgress,
                        streak,
                        manifest,
                        goals,
                    );
                }
                // 交接给新会话的是这三样：额度（§8：不随翻页重置）、目标名、压缩摘要。新会话由
                // 主循环装起来 —— 那一边才有它的 `Harness`，所以这三样随一条命令过去。
                let carry_tokens = goal_usage(goals, name, &from);
                harness.notice(&render::wording::heng(
                    &render::wording::goal_rolled_over_new(&from),
                ));
                let _ = requests.send(CliRequest::Rollover {
                    carry_tokens,
                    goal: name.to_owned(),
                    summary,
                });
                // 这一趟到这里收工：接着跑的是**新的那个会话**。
                *running = false;
                return Ok(());
            }
        }
    }
    // 走到这里只有一个原因：**手势**。取消可能是 `Esc` 确认框里选了「停下」，也可能是
    // `Ctrl-C` 或输入结束 —— 三种都是人的意思，所以落一条「人主动停」的收尾事件（§5、§10）。
    // 它让 `--continue` 分得出「人停的」与「崩掉的」：前者回来别自己又跑起来。
    *running = false;
    stop_goal_loop(
        harness,
        name,
        crate::events::GoalStopReason::UserStopped,
        0,
        manifest,
        goals,
    )
}

/// 循环即将驱动的那个回合由什么起头。
pub(crate) enum TurnStart<'a> {
    /// 用户打的 prompt：它成为一条 `user` 消息，然后回合跑起来。
    Prompt(&'a str),
    /// 光秃秃的 `/<skill>`：[`Harness::run_skill`] 加载正文，它自己就投影成一条 `user` 消息，所以
    /// 为它凭空造一条 prompt 是不对的。转录绝不能显示用户没打过的字。
    Skill(&'a str),
    /// 目标循环的一个回合：起头的是刚刚注入的那条上下文（清单、提醒或摘要）。
    Injected,
}

/// 一条退出路径该用哪个码（`.scratch/exit-gesture/spec.md` §3）。
///
/// 人主动退（空闲双击、`/quit`、输入结束）= 0；忙碌中被打断而退 = 130（与 `SIGINT` 的
/// 128 + 2 惯例一致）。空闲那条路**不许**「顺便」变成 130，所以两档只在这里定义一次。
pub(crate) fn exit_code_after(quit: bool) -> ExitCode {
    if quit {
        ExitCode::from(130)
    } else {
        ExitCode::SUCCESS
    }
}

/// 交互式会话的退出账本：人在一次运行中途有没有要求退出
/// （`.scratch/exit-gesture/spec.md` §3、§5）。
///
/// 忙碌里的第二下 `Ctrl-C` 不是「立刻把进程按下去」：它**记下请求**，并让当前回合像被取消
/// 一样拿到它的收尾事件 —— 等回合落地之后由循环用 [`exit_code_after`] 兑现那个码。这样
/// `TerminalModes` 的 `Drop`、`ratatui::restore()` 与退出回执都会跑；旧写法
/// ``std::process::exit`` 会跳过一切析构，把终端留在 raw + 备用屏幕里。
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ExitRequest {
    pub(crate) quit: bool,
}

impl ExitRequest {
    /// 把一个前端手势并进这份账。
    pub(crate) fn apply(&mut self, event: &FrontEndEvent, signal: &CancelSignal) {
        match event {
            FrontEndEvent::Quit => {
                self.quit = true;
                signal.cancel();
            }
            FrontEndEvent::Cancel => signal.cancel(),
            // 模式循环在调用点自己处理。
            FrontEndEvent::CycleMode => {}
            // 改名也是：它既不退出，也不取消。
            FrontEndEvent::SessionTitled { .. } => {}
            // 换会话更不是：它由调用点处理（那边才有宿主表）。
            FrontEndEvent::OpenSession { .. } => {}
            FrontEndEvent::NewSession | FrontEndEvent::CloseSession { .. } => {}
            FrontEndEvent::AttentionSeen => {}
            // 切换由调用点自己处理（忙闲判据在那一侧）。
            FrontEndEvent::OpenPicker(_) => {}
        }
    }

    /// 有没有人要求退出。
    pub(crate) fn requested(&self) -> bool {
        self.quit
    }

    /// 这一趟该用的退出码。
    pub(crate) fn code(&self) -> ExitCode {
        exit_code_after(self.quit)
    }
}

/// 跑一个回合，同时仍然盯着取消手势。
///
/// 回合占着会话，所以循环没法自己读键盘；它改为 select 控制台那些未被请求的事件。取消手势让回合
/// 拿到它的收尾事件；`Quit`（忙碌里举手之后的那第二下）同样先取消，只把「要退出」记进
/// [`ExitRequest`] —— 进程不在这一层结束（spec §6、`.scratch/exit-gesture/spec.md` §3）。
pub(crate) async fn run_one_turn(
    harness: &mut Harness,
    inbox: &mut tokio::sync::mpsc::Receiver<CliCommand>,
    start: TurnStart<'_>,
    quit: &mut ExitRequest,
) -> Result<crate::agent::TurnOutcome, crate::Error> {
    let signal = harness.cancel_signal();
    let modes = harness.mode_cycle();
    // 同上：回执走借用之前克隆下来的渲染句柄。
    let render = harness.render_handle();
    let mut turn = Box::pin(async move {
        match start {
            TurnStart::Prompt(input) => harness.run_turn(input).await,
            TurnStart::Skill(name) => harness.run_skill(name).await,
            TurnStart::Injected => harness.run_injected_turn().await,
        }
    });
    loop {
        tokio::select! {
            result = &mut turn => return result,
            command = inbox.recv() => match command {
                // 权限门每次调用都读策略，所以这次按键挪动的是「下一次调用」的立场 —— 这就是「回合
                // 中途换档」的意思。
                Some(CliCommand::Gesture { event: FrontEndEvent::CycleMode, .. }) => {
                    modes.cycle();
                }
                // 运行中要切换：拒绝，并说清为什么（spec §6）。
                Some(CliCommand::Gesture { event: FrontEndEvent::OpenPicker(_), .. }) => {
                    render.notice(&render::wording::heng(render::wording::switch_busy()));
                }
                Some(CliCommand::Gesture { event, .. }) => quit.apply(&event, &signal),
                // 输入结束让回合像被取消一样落下来，所以流仍然得到它的收尾。
                Some(CliCommand::Close) | None => signal.cancel(),
                // 跑着的时候到达的提交：这个会话的循环一次只办一件，多的忽略。
                Some(_) => {}
            },
        }
    }
}
