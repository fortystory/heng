use super::interactive::{
    CliCommand, CliRequest, ExitRequest, McpPromptEntry, Submission, exit_code_after,
    front_to_request, records_itself, retarget_to, submission, undo_receipt,
};
use super::{ConsoleHandle, Mode, finish_session};
use crate::agent::{CancelSignal, UndoOutcome};
use crate::config::{Config, ReasoningEffort};
use crate::events::{SessionId, SpeakerId, ToolCallId};
use crate::render::{ConsolePort, ConsoleRequest, FrontEndEvent, PickerKind, console};
use crate::{Harness, Policy};
use std::path::PathBuf;
use std::process::ExitCode;

/// 一张三家厂商都给了 key 的配置，于是 `retarget_to` 能真的组装出 provider。
fn switchable_config() -> crate::config::Config {
    crate::config::resolve(
        None,
        &[
            ("MOONSHOT_API_KEY", "sk-kimi"),
            ("DEEPSEEK_API_KEY", "sk-deepseek"),
            ("MINIMAX_API_KEY", "sk-minimax"),
            ("MINIMAX_CN_API_KEY", "sk-minimax-cn"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect(),
    )
    .expect("一张能跑的配置")
}

/// 换模型是**重建**：provider 与配置都重新组装，而累计额度（会话事实）照旧带着走。
#[test]
fn retargeting_rebuilds_the_provider_and_carries_the_goal_usage_over() {
    let config = switchable_config();
    let retargeted = retarget_to(&config, "MiniMax-M3.1-Flash-Preview", None, 12_345)
        .expect("MiniMax M3.1 有 key");

    assert_eq!(retargeted.profile, "minimax");
    assert_eq!(retargeted.config.model, "MiniMax-M3.1-Flash-Preview");
    assert_eq!(retargeted.config.carried_tokens, 12_345);
    // 请求参数是从这个模型的配置条目读的，不是从上一个会话抄的。
    assert_eq!(
        retargeted.config.executor_model,
        config.routing.executor_model
    );
    // 换一个厂商就换 profile，于是发言者的名字也跟着换（spec §4）。
    assert_eq!(
        retargeted.config.model, "MiniMax-M3.1-Flash-Preview",
        "模型 id 就是线上发的那一个"
    );
}

/// 当前档位新模型不认时退回「默认」—— 而不是让适配器在回合中途把它丢掉。
#[test]
fn an_effort_the_new_model_cannot_have_falls_back_to_the_default() {
    let config = switchable_config();
    // M3.1 认 xhigh。
    let kept = retarget_to(
        &config,
        "MiniMax-M3.1-Flash-Preview",
        Some(ReasoningEffort::Xhigh),
        0,
    )
    .unwrap();
    assert_eq!(
        kept.config.params.reasoning_effort,
        Some(ReasoningEffort::Xhigh)
    );

    // Kimi 只有 low/high/max，于是 xhigh 落回「默认」（不发这个参数）。
    let dropped = retarget_to(&config, "kimi-k3", Some(ReasoningEffort::Xhigh), 0).unwrap();
    assert_eq!(dropped.config.params.reasoning_effort, None);
    assert_eq!(dropped.profile, "kimi");

    // 换档只是配置动作：`high` 照旧留着。
    let high = retarget_to(&config, "kimi-k3", Some(ReasoningEffort::High), 0).unwrap();
    assert_eq!(
        high.config.params.reasoning_effort,
        Some(ReasoningEffort::High)
    );
}

/// 没登记的模型与缺 key 的模型都是错误，且错的那句把修法说全。
#[test]
fn retargeting_to_a_model_that_cannot_be_assembled_says_why() {
    let config = switchable_config();
    let unknown = retarget_to(&config, "gpt-4o", None, 0).err().unwrap();
    assert!(unknown.contains("gpt-4o"), "{unknown}");
    assert!(unknown.contains("kimi-k3"), "已知 id 要列全：{unknown}");

    // 一个指向没给 key 的 profile 的模型：那条错误必须点名那个环境变量（修法能直接抄）。
    let config = crate::config::resolve(
        Some(
            "[providers.kimi-code-cn]\nbase_url = \"https://api.kimi.com/coding/v1\"\n\
             api_key_env = \"KIMI_CODE_CN_API_KEY\"\n\n\
             [models.\"kimi-k3\"]\nprovider = \"kimi-code-cn\"\n",
        ),
        &Default::default(),
    )
    .unwrap();
    let message = retarget_to(&config, "kimi-k3", None, 0)
        .err()
        .expect("缺 key 就组不出 provider");
    assert!(message.contains("KIMI_CODE_CN_API_KEY"), "{message}");
}

/// `/model` 与 `/effort` 进 `Submission`，且**命令记号在任何位置都算命令**。
///
/// 后一半是 `input-tokens` 票 04 那条纪律：一条普通消息里夹着命令记号，仍然被解析成
/// 命令（而不是把整条当成 prompt）。
#[test]
fn the_switch_commands_are_parsed_and_a_token_anywhere_still_counts() {
    assert!(matches!(read("/model kimi-k3"), Submission::Model(id) if id == "kimi-k3"));
    assert!(matches!(read("/effort high"), Submission::Effort(effort) if effort == "high"));
    assert!(matches!(read("/effort 默认"), Submission::Effort(effort) if effort == "默认"));
    // 缺参数**不是**一条错误命令：它到循环那里才判（与 `parse_goal_new_line` 同形）。
    assert!(matches!(read("/model"), Submission::Model(id) if id.is_empty()));
    // 带点号的 id 不按点号切（`MiniMax-M3.1-Flash-Preview`）。
    assert!(matches!(
        read("/model MiniMax-M3.1-Flash-Preview"),
        Submission::Model(id) if id == "MiniMax-M3.1-Flash-Preview"
    ));
    // 命令记号在**任何位置**都算命令；参数照旧是「记号旁边那一整段」，与 `/goal-new`
    // 同一个形状（`input-tokens` 票 04 的纪律）。
    assert!(matches!(
        read("先看看这个 /model kimi-k3"),
        Submission::Model(id) if id == "先看看这个 kimi-k3"
    ));
    // 内建命令带着记号之外的字仍然**不是**内建命令 —— 整条退回普通消息。
    assert!(matches!(read("/model 这个"), Submission::Model(_)));
}

/// 模型候选：**`config.toml` 里显式配置过的那几家**名下、有密钥的模型，按键序。
///
/// 两条判据都要，而**当前模型无条件在列**（一份什么都不写的配置里清单会空，而看不到自己正在用
/// 的那个模型是最刺眼的一种不诚实）。
#[test]
fn the_model_candidates_are_the_configured_providers_models_and_the_current_one() {
    let config = crate::config::resolve(
        Some(
            // 只给两条 profile：一条有 key（minimax），一条没给 key（deepseek）。
            "[providers.minimax]\napi_key = \"sk-minimax\"\n\n[providers.deepseek]\n",
        ),
        &Default::default(),
    )
    .unwrap();
    // 当前模型挂在 `kimi`（内建、config.toml 里没写）上 —— 它兜底进清单。
    let options = super::interactive::model_picker_options(&config, "kimi-k3");
    let labels: Vec<&str> = options.iter().map(|option| option.label.as_str()).collect();
    assert_eq!(
        labels,
        vec![
            // 配置里有、且有密钥的那家（两个 id）。
            "MiniMax-M3",
            "MiniMax-M3.1-Flash-Preview",
            // 当前模型：内建 profile，也无条件在列（键序落在最后）。
            "kimi-k3",
            // 配置里有、但没给密钥的那家（`deepseek-flash` 与 `deepseek-v4-pro`）
            // 一个都不在 —— 切过去起不来。
        ],
        "配置里写过的 + 当前模型"
    );
    // 缺密钥的那家的模型一个都不在。
    assert!(
        !labels.contains(&"deepseek-v4-pro") && !labels.contains(&"deepseek-flash"),
        "缺 key 的不列：{labels:?}"
    );
    // 内建但没被配置的 profile（`kimi-code`）名下的模型也不在 —— 那正是注释掉
    // `[providers.kimi-code]` 的效果。
    assert!(
        !labels
            .iter()
            .any(|label| label.starts_with("k3") || label.contains("coding")),
        "没配置的那家不列：{labels:?}"
    );
    // 当前那个被标出来了，detail 说它走哪个 profile。
    let current = options
        .iter()
        .find(|option| option.current)
        .expect("当前那个被标出来了");
    assert_eq!(current.label, "kimi-k3");
    assert!(current.detail.contains("kimi"), "{:?}", current.detail);
}

/// 档位候选：**第一档是「默认」**，其余按强度从弱到强；表为空时给空清单（前端显示 `固定`）。
#[test]
fn the_effort_candidates_start_with_the_default_and_follow_the_strength_order() {
    let five = super::interactive::effort_picker_options(
        "MiniMax-M3.1-Flash-Preview",
        Some(ReasoningEffort::High),
    );
    assert_eq!(
        five.iter()
            .map(|option| option.label.as_str())
            .collect::<Vec<_>>(),
        vec!["默认", "low", "medium", "high", "xhigh", "max"]
    );
    let current = five.iter().find(|option| option.current).unwrap();
    assert_eq!(current.label, "high");

    // Kimi 只有三档。
    assert_eq!(
        super::interactive::effort_picker_options("kimi-k3", None)
            .iter()
            .map(|option| option.label.as_str())
            .collect::<Vec<_>>(),
        vec!["默认", "low", "high", "max"]
    );
    assert!(
        super::interactive::effort_picker_options("kimi-k3", None)[0].current,
        "None 就是「默认」那一档"
    );

    // 没有旋钮的模型：空清单。那一格显示 `固定`，而不是给一个只有「默认」可选的假清单。
    assert!(super::interactive::effort_picker_options("MiniMax-M3", None).is_empty());
    assert!(
        super::interactive::effort_picker_options("kimi-for-coding-highspeed", None).is_empty()
    );
}

/// 一个共享的 `Vec<u8>`，拿来当 headless 渲染器的写出口。
#[derive(Clone, Default)]
struct Sink(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("sink 已中毒").extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Sink {
    fn text(&self) -> String {
        String::from_utf8(self.0.lock().expect("sink 已中毒").clone()).expect("sink 里是 UTF-8")
    }
}

/// 一个交互式的循环与它那条键盘：忙闲判定、切换、回执都在这一对上面验。
async fn live_session(
    config: &Config,
) -> (Harness, ConsoleHandle, ConsolePort, Sink, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("临时目录");
    let cwd = dir.path().join("workspace");
    std::fs::create_dir_all(&cwd).expect("工作区");
    let stderr = Sink::default();
    let (console, port, _events) = console();
    let harness = crate::assemble(crate::AssemblyParts {
        provider: Box::new(
            crate::provider::openai::OpenAiProvider::build(
                config,
                "kimi-k3",
                crate::provider::openai::silent_warnings(),
            )
            .expect("kimi-k3 有 key"),
        ),
        speaker: SpeakerId::Debater("kimi".into()),
        config: config.session_config("kimi-k3").expect("kimi-k3 在配置里"),
        render: crate::render::RenderChannel::start(
            crate::render::Renderer::headless(crate::render::RenderSinks {
                stdout_result: Box::new(std::io::sink()),
                stderr_diagnostic: Box::new(stderr.clone()),
            }),
            crate::events::SessionId::new("s-1"),
        ),
        scaffold: crate::SessionScaffold {
            cwd,
            log_path: dir.path().join("log.jsonl"),
            session_id: SessionId::new("s-1"),
            tools: crate::tools::builtin(false),
            locks: crate::tools::PathLocks::new(),
            policy: Policy::for_mode(Mode::Ask),
            // 没有交互式应答者，于是权限门降级为「不动手」：这些测试不写文件。
            asker: None,
            questions: None,
            hook: None,
            home: None,
        },
    })
    .await
    .expect("组装得起来");
    (harness, console, port, stderr, dir)
}

/// 运行中切换被**拒绝**，而拒绝要说清为什么（spec §6）。
#[tokio::test]
async fn a_switch_while_something_runs_is_refused_with_a_reason() {
    let config = switchable_config();
    let (mut harness, console, mut port, stderr, _dir) = live_session(&config).await;

    super::interactive::open_picker(&mut harness, &console, &config, PickerKind::Model, true).await;
    // 没有清单发出去，而会话一点没变 —— 被拒绝的切换不重建 provider，也不推新事实。
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), port.recv())
            .await
            .is_err(),
        "运行中不发清单"
    );
    assert_eq!(harness.model(), "kimi-k3");
    // 渲染器是另一个 task：把它收掉，每一条渲染事件才都落到那两个写出口上。
    harness.shutdown().await;
    assert!(
        stderr
            .text()
            .contains(super::render::wording::switch_busy()),
        "拒绝给了那一句：{:?}",
        stderr.text()
    );
}

/// 空闲时那一份清单真的发到端口上了，而且人取消了之后什么都不会变。
#[tokio::test]
async fn an_idle_switch_asks_the_front_end_and_a_cancellation_changes_nothing() {
    let config = switchable_config();
    let (mut harness, console, mut port, stderr, _dir) = live_session(&config).await;

    let front_end = tokio::spawn(async move {
        match port.recv().await {
            Some(ConsoleRequest::Picker(request)) => {
                let title = request.title.clone();
                let count = request.options.len();
                let _ = request.reply.send(None);
                (title, count)
            }
            other => panic!("期望一份清单，实际 {other:?}"),
        }
    });
    super::interactive::open_picker(&mut harness, &console, &config, PickerKind::Model, false)
        .await;
    let (title, count) = front_end.await.expect("前端回来了");

    assert_eq!(title, super::render::wording::picker_title_model());
    assert_eq!(
        count,
        // `switchable_config()` 只给环境变量、config.toml 是空的 —— 于是候选只有**当前模型**
        // 兜底那一个（「配置里写过的」那一条在它下面没有可选项）。
        1,
        "没有显式配置时清单里只有当前模型"
    );
    assert_eq!(harness.model(), "kimi-k3", "取消不是一次切换");
    harness.shutdown().await;
    assert!(
        !stderr.text().contains("前缀缓存"),
        "取消不给回执：{:?}",
        stderr.text()
    );
}

/// 这些测试里一个会话知道的技能。
fn has_skill(name: &str) -> bool {
    matches!(name, "ask-matt" | "review")
}

/// 这一轮 `/` 菜单里的 MCP 模板条目。
fn prompts() -> Vec<McpPromptEntry> {
    vec![McpPromptEntry::new(
        "db".to_owned(),
        crate::mcp::PromptSummary {
            name: "user_report".to_owned(),
            description: Some("按 id 出一份报告".to_owned()),
            arguments: vec![],
        },
    )]
}

fn read(text: &str) -> Submission {
    let prompts = prompts();
    submission(text, has_skill, &prompts)
}

/// 一次 `/undo` 的三种结果各说一句话，成功那次也**必须**说 —— 回滚在屏幕上本来是无声的
/// （`.scratch/fs-agent-v1/issues/35-undo-receipt.md`）。
#[test]
fn an_undo_always_answers_with_one_line() {
    let undone = undo_receipt(Ok(Some(UndoOutcome {
        tool_call_id: ToolCallId::new("call-1"),
        path: PathBuf::from("src/a.rs"),
    })));
    assert_eq!(undone, "heng: 已撤销对 src/a.rs 的这次编辑");
    assert_eq!(undo_receipt(Ok(None)), "heng: 没有可撤销的修改");
    assert_eq!(
        undo_receipt(Err(crate::Error::Undo("快照对不上".to_owned()))),
        "heng: 撤销失败：快照对不上"
    );
}

/// 哪些提交要另记一条命令记录（`.scratch/command-echo/spec.md`）。
///
/// 判据只有一条：这条提交会不会变成一句 user 消息。会的那几条原文已经在转录里了
/// （用户气泡，模型也读得到），不会的那些 —— 内建命令、MCP 模板、裸技能、错字 ——
/// 一个字都不留，而回执只有成功才有。
#[test]
fn only_the_submissions_that_leave_no_words_of_their_own_get_a_command_record() {
    for line in [
        "/clear",
        "/undo",
        "/model kimi-k3",
        "/effort high",
        "/loop 一个目标",
        "/goal-new x .scratch/a",
        "/discuss 一个问题",
        "/ask-matt",
        "/db:user_report",
        "/没这个命令",
        // 记号前面有字也一样记：`/loop` 的提交语义是**执行**（ADR 0012），所以
        // `请 /loop 一个目标` 真的会启动循环，而它启动的那句话一个字都不留。
        "请 /loop 一个目标",
    ] {
        assert!(records_itself(&read(line)), "「{line}」该留一条原文");
    }
    for line in [
        "问一句普通的话",
        "/ask-matt 帮我改一下这两处",
        "",
        "   ",
        "/quit",
    ] {
        assert!(!records_itself(&read(line)), "「{line}」不该重复记一条");
    }
}

#[test]
fn required_parameters_must_be_filled() {
    let prompt = crate::mcp::PromptSummary {
        name: "user_report".to_owned(),
        description: None,
        arguments: vec![
            crate::mcp::PromptArgument {
                name: "id".to_owned(),
                description: None,
                required: true,
            },
            crate::mcp::PromptArgument {
                name: "format".to_owned(),
                description: None,
                required: false,
            },
        ],
    };
    let empty = serde_json::Map::new();
    assert_eq!(
        super::interactive::missing_required_arguments(&prompt, &empty),
        vec!["id"]
    );

    let mut filled = serde_json::Map::new();
    filled.insert("id".to_owned(), serde_json::Value::String("42".to_owned()));
    assert!(
        super::interactive::missing_required_arguments(&prompt, &filled).is_empty(),
        "必填填上了就不该再报"
    );
}

#[test]
fn the_menu_shows_the_builtin_commands_skills_and_mcp_prompts() {
    let prompts = vec![McpPromptEntry::new(
        "db".to_owned(),
        crate::mcp::PromptSummary {
            name: "user_report".to_owned(),
            description: Some("按 id 出一份报告".to_owned()),
            arguments: vec![],
        },
    )];
    let entries = slash_catalog(
        &crate::render::wording::BUILT_IN_COMMANDS,
        &[("review", "审一遍")],
        &prompts,
    );
    let names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
    assert!(names.contains(&"undo"), "常量条目照旧：{names:?}");
    assert!(names.contains(&"review"), "技能照旧：{names:?}");
    assert!(
        names.contains(&"db:user_report"),
        "运行时那一半也在菜单里：{names:?}"
    );
    let entry = entries
        .iter()
        .find(|entry| entry.name == "db:user_report")
        .unwrap();
    assert_eq!(entry.description, "按 id 出一份报告");
}

#[test]
fn inline_arguments_fill_the_declared_parameters_in_order() {
    let prompt = crate::mcp::PromptSummary {
        name: "user_report".to_owned(),
        description: None,
        arguments: vec![
            crate::mcp::PromptArgument {
                name: "id".to_owned(),
                description: None,
                required: true,
            },
            crate::mcp::PromptArgument {
                name: "format".to_owned(),
                description: None,
                required: false,
            },
        ],
    };
    let filled =
        super::interactive::prompt_inline_arguments("db:user_report", &prompt, "42 json").unwrap();
    assert_eq!(filled["id"], "42");
    assert_eq!(filled["format"], "json");

    let too_many =
        super::interactive::prompt_inline_arguments("db:user_report", &prompt, "42 json extra");
    assert!(too_many.is_err(), "多出来的参数该被拒");
}

#[test]
fn a_prompt_entry_carries_its_inline_arguments() {
    let parsed = read("/db:user_report 42");
    match parsed {
        Submission::McpPrompt {
            server,
            prompt,
            inline,
        } => {
            assert_eq!(server, "db");
            assert_eq!(prompt, "user_report");
            assert_eq!(inline, "42");
        }
        other => panic!("该解析成模板调用，实际是 {other:?}"),
    }
}

#[test]
fn a_colon_name_that_is_not_in_the_menu_is_an_unknown_command() {
    // 菜单里没有这个名字时，它与任何打错的 `/` 名字同类 —— 不会被当成模板调用。
    assert!(matches!(read("/db:nope 42"), Submission::Unknown(_)));
    assert!(matches!(
        read("/db:user_report 42"),
        Submission::McpPrompt { .. }
    ));
}

#[test]
fn the_exit_code_is_zero_unless_someone_asked_to_quit() {
    // `.scratch/exit-gesture/spec.md` §3：人主动退 = 0；忙碌里被打断而退 = 130
    // （与 `SIGINT` 的 128 + 2 惯例一致）。空闲那条路不许「顺便」变成 130。
    assert_eq!(exit_code_after(false), ExitCode::SUCCESS);
    assert_eq!(exit_code_after(true), ExitCode::from(130));
}

#[test]
fn the_receipt_names_the_session_and_never_changes_the_exit_code() {
    // `.scratch/exit-gesture/spec.md` §5：交互式会话（TUI 与 `--plain`）退出时在终端
    // 交还之后打一行能直接粘的复盘命令。writer 由调用方传（生产那边是 stderr），所以
    // 「打在哪」由调用点保证，这里钉的是内容与「绝不改退出码」。
    for code in [ExitCode::SUCCESS, ExitCode::from(130)] {
        let mut out: Vec<u8> = Vec::new();
        let returned = finish_session(code, "01J8ZQ4K7M", &mut out);
        let text = String::from_utf8(out).expect("回执是 utf-8");
        assert_eq!(text, "heng: 会话 01J8ZQ4K7M；接着跑：heng -c 01J8ZQ4K7M\n");
        assert_eq!(returned, code, "一行回执不该改退出码");
    }
}

#[test]
fn a_quit_gesture_is_recorded_and_cancels_instead_of_killing_the_process() {
    // 忙碌里的第二下（举手之后的 `Quit`）不是「立刻把进程按下去」：它记下退出请求，
    // 并让当前回合像被取消一样拿到它的收尾事件。旧写法 ``std::process::exit``
    // 会跳过一切析构，把终端留在 raw + 备用屏幕里（§3）。
    let signal = CancelSignal::new();
    let mut quit = ExitRequest::default();
    assert!(!quit.requested());
    assert_eq!(quit.code(), ExitCode::SUCCESS);

    quit.apply(&FrontEndEvent::Quit, &signal);
    assert!(quit.requested(), "退出请求被记下来了");
    assert!(signal.is_cancelled(), "而回合拿到了它的收尾");
    assert_eq!(quit.code(), ExitCode::from(130));

    // 普通的取消手势只取消：取消不等于退出。
    let signal = CancelSignal::new();
    let mut quit = ExitRequest::default();
    quit.apply(&FrontEndEvent::Cancel, &signal);
    assert!(signal.is_cancelled());
    assert!(!quit.requested(), "Esc / 第一下 Ctrl-C 不是退出");
    assert_eq!(quit.code(), ExitCode::SUCCESS);
}

#[test]
fn the_session_gestures_become_host_table_requests_where_the_host_table_is() {
    // 点会话页上别的那一行、新建、关一个会话 —— 三条都落到宿主表，而宿主表在主循环手里，所以主
    // 循环就地转成一条请求。曾经在主循环这边有一个空分支把 `OpenSession` 吃掉，于是点击
    // 既不装载也不切换：屏幕上什么都没有发生（2026-10-12 维护者报告）。
    let id = SessionId::new("01J8ZQ4K7M".to_owned());
    assert_eq!(
        front_to_request(&FrontEndEvent::OpenSession { id: id.clone() }),
        Some(CliRequest::Open(id.clone())),
        "点会话页上的一行 = 切到那个会话"
    );
    assert_eq!(
        front_to_request(&FrontEndEvent::NewSession),
        Some(CliRequest::New)
    );
    assert_eq!(
        front_to_request(&FrontEndEvent::CloseSession { id: id.clone() }),
        Some(CliRequest::Close(id.clone()))
    );
    // 其余手势不是会话级的：它们归前台那个会话办（取消、模式、选择器、改名）。
    for event in [
        FrontEndEvent::Cancel,
        FrontEndEvent::Quit,
        FrontEndEvent::AttentionSeen,
    ] {
        assert_eq!(front_to_request(&event), None, "{event:?} 不是会话级手势");
    }
}

#[test]
fn a_blank_submission_is_ignored_rather_than_sent_or_read_as_the_end_of_input() {
    // 在空输入上按回车产生什么。以前是循环自己决定这件事的，所以 `cargo test` 里没有任何东西把
    // 它钉住 —— 而 TUI 曾经把空草稿当作输入结束的哨兵发出去，结果退掉了整个会话。
    assert!(matches!(read(""), Submission::Ignore));
    assert!(matches!(read("   "), Submission::Ignore));
    assert!(matches!(read("\n\n\t\n"), Submission::Ignore));
    // 它也不是一条 prompt：空消息绝不能到达模型。
    assert!(!matches!(read("  \n  "), Submission::Prompt(_)));
}

#[test]
fn a_single_line_still_reads_exactly_as_it_did() {
    assert_eq!(read("/quit"), Submission::Quit);
    assert_eq!(read("/exit"), Submission::Quit);
    assert_eq!(read("/undo"), Submission::Undo);
    assert_eq!(read("  /quit  "), Submission::Quit, "与从前一样去掉空白");
    assert_eq!(read("/nope"), Submission::Unknown("/nope".to_owned()));
    assert_eq!(
        read("/ask-matt 帮我看一下"),
        Submission::Skill {
            name: "ask-matt".to_owned(),
            task: "/ask-matt 帮我看一下".to_owned(),
            bare: false,
        },
        "技能名留在发给模型的文本里（ui-trim §2）"
    );
    assert_eq!(
        read("/ask-matt"),
        Submission::Skill {
            name: "ask-matt".to_owned(),
            task: "/ask-matt".to_owned(),
            bare: true,
        },
        "光秃秃的技能名只加载正文"
    );
    assert_eq!(read("hello"), Submission::Prompt("hello".to_owned()));
}

#[test]
fn a_multi_line_brief_follows_the_skill_named_on_the_first_line() {
    assert_eq!(
        read("/ask-matt\n第一行\n第二行"),
        Submission::Skill {
            name: "ask-matt".to_owned(),
            task: "/ask-matt\n第一行\n第二行".to_owned(),
            bare: false,
        }
    );
    // 任务可以从技能那一行开始，再往下面续。
    assert_eq!(
        read("/ask-matt 第一行\n第二行"),
        Submission::Skill {
            name: "ask-matt".to_owned(),
            task: "/ask-matt 第一行\n第二行".to_owned(),
            bare: false,
        }
    );
}

#[test]
fn a_skill_keeps_the_words_around_its_name() {
    // 技能名**前面**的字也不许丢，技能名本身也不许丢：整条原文就是发出去的那条消息
    // （`.scratch/ui-trim/spec.md` §2）。内建命令那套「参数取记号旁边的字」不适用于它 ——
    // 技能的名字本身就是这句话的一部分。
    assert_eq!(
        read("请用 /ask-matt 帮我看一下"),
        Submission::Skill {
            name: "ask-matt".to_owned(),
            task: "请用 /ask-matt 帮我看一下".to_owned(),
            bare: false,
        }
    );
}

#[test]
fn a_pasted_paragraph_that_opens_with_a_slash_is_a_prompt() {
    // 后面带行的东西都按它是一整段来读，所以粘贴一个路径或一段代码不会换来一句它从未想说的
    // 「未知命令」。
    assert_eq!(
        read("/usr/bin/env cargo test\n第二行"),
        Submission::Prompt("/usr/bin/env cargo test\n第二行".to_owned())
    );
    // 只占一行时它仍然是个错字，而且是会告诉用户的那一种。
    assert_eq!(
        read("/usr/bin/env"),
        Submission::Unknown("/usr/bin/env".to_owned())
    );
    // 它后面的空行仍然算「后面什么都没有」。
    assert_eq!(read("/nope\n   "), Submission::Unknown("/nope".to_owned()));
}

#[test]
fn a_built_in_takes_no_task_so_a_line_after_it_is_not_dropped() {
    // `/undo` 后面跟着别的东西时就不是这条命令了：后面的行绝不能消失，所以整条提交按「以 `/` 开
    // 头的行」那一套规则读 —— `/undo` 没点名任何技能，所以它是一条 prompt。
    assert_eq!(
        read("/undo\n把 X 改成 Y"),
        Submission::Prompt("/undo\n把 X 改成 Y".to_owned())
    );
    assert_eq!(read("/undo"), Submission::Undo);
    assert_eq!(read("/undo  "), Submission::Undo, "结尾的空白没关系");
}

#[test]
fn a_doubled_slash_is_not_a_built_in_command() {
    // 内建命令要求记号的原文就是 `/<名字>`，所以 `//undo` 不再是 `Undo` —— 旧路径把它当
    // 错字，而 `Undo` 有回滚副作用，不该由多出来的一个斜杠换来。技能那一侧不受影响：
    // 名字本来就是剥掉斜杠之后才比的。
    assert_eq!(read("//undo"), Submission::Prompt("//undo".to_owned()));
    assert_eq!(read("//clear"), Submission::Prompt("//clear".to_owned()));
    assert_eq!(
        read("//review"),
        Submission::Skill {
            name: "review".to_owned(),
            task: "//review".to_owned(),
            bare: true,
        },
        "多出来那个斜杠不把技能挡在裸调用之外"
    );
}

#[test]
fn a_command_can_sit_anywhere_and_earlier_words_join_the_task() {
    // `.scratch/input-tokens/spec.md` §5：整个草稿里任意行、任意位置的 `/name` 都算命令，
    // 取**最靠左**的那一个，而它前面的字（含前面的每一行）并入任务 —— 这一条正是为了不
    // 重蹈「丢用户写下的字」。
    assert_eq!(
        read("请 /loop 我的目标"),
        Submission::Loop("请 我的目标".to_owned())
    );
    assert_eq!(
        read("第一行\n/loop 目标"),
        Submission::Loop("第一行\n目标".to_owned()),
        "位置不看行"
    );
    // 这一档是**有牙**的（访谈里明码标价选下的）：`@` 被误认没有后果，`/` 被误认会真的
    // 执行。如实钉住这个代价，别把它当缺陷改掉。
    assert_eq!(
        read("用 /loop 做目标循环"),
        Submission::Loop("用 做目标循环".to_owned())
    );
    // 记号在末尾时，前文就是全部任务。
    assert_eq!(read("请 /loop"), Submission::Loop("请".to_owned()));
    // 一个句子里只有**命令**记号算数，别的记号只是普通文字。
    assert_eq!(
        read("看 /tmp/x，然后 /loop 目标"),
        Submission::Loop("看 /tmp/x，然后 目标".to_owned())
    );
}

#[test]
fn a_no_argument_command_still_needs_the_whole_draft() {
    // 无参命令要么是整条、要么什么都不算 —— 一句话里提一句 `/clear` 不会被劫持成一次清空
    // （spec §5，「明确不做」那一条）。
    assert_eq!(
        read("请把 /clear 加到文档里"),
        Submission::Prompt("请把 /clear 加到文档里".to_owned())
    );
    assert_eq!(
        read("请看 /undo"),
        Submission::Prompt("请看 /undo".to_owned())
    );
    // 单独一行仍旧是它自己（不变）。
    assert_eq!(read("/clear"), Submission::Clear);
    assert_eq!(read("/undo"), Submission::Undo);
}

#[test]
fn a_draft_without_a_command_token_is_still_a_message() {
    // 草稿里找不到命令记号时整条仍当普通消息（不变）。
    assert_eq!(
        read("看 /tmp/x 这个文件"),
        Submission::Prompt("看 /tmp/x 这个文件".to_owned())
    );
    assert_eq!(
        read("第一行\n/loopish 目标"),
        Submission::Prompt("第一行\n/loopish 目标".to_owned())
    );
}

#[test]
fn a_retired_command_is_now_an_unknown_one() {
    // 在它们所控制的那个模式退场之前，`plan` 与 `endplan` 都是内建命令
    // （`.scratch/todo-and-modes`）。现在没有东西去解析它们了，答案是未知命令那段文本 —— 那段
    // 文本会点名真正存在的内建命令。
    assert_eq!(read("/plan"), Submission::Unknown("/plan".to_owned()));
    assert_eq!(read("/endplan"), Submission::Unknown("/endplan".to_owned()));
}

#[test]
fn the_mode_flag_parses_the_four_modes_and_refuses_the_rest() {
    use super::interactive::parse_interactive;

    let args = |words: &[&str]| {
        parse_interactive(
            &words
                .iter()
                .map(|word| (*word).to_owned())
                .collect::<Vec<_>>(),
        )
    };
    for (written, expected) in [
        ("readonly", Mode::Readonly),
        ("ask", Mode::Ask),
        ("workspace", Mode::Workspace),
        ("auto", Mode::Auto),
    ] {
        assert_eq!(args(&["--mode", written]).unwrap().mode, Some(expected));
    }
    assert_eq!(args(&[]).unwrap().mode, None, "不写表示由文件决定");
    let error = args(&["--mode", "plan"]).unwrap_err();
    for word in ["plan", "readonly", "ask", "workspace", "auto"] {
        assert!(error.contains(word), "`{word}` 没有出现在：{error}");
    }
}

#[test]
fn the_continue_flag_parses_both_spellings_and_defaults_to_off() {
    // `.scratch/exit-gesture/spec.md` 的「补充说明」点名要补的两条缺口之一：`exit(130)` 那条
    // 路径与 `--continue` 的旗标解析（这里的 `mod tests` 原先只测过 `--mode`）。
    use super::interactive::{Resume, parse_interactive};

    let args = |words: &[&str]| {
        parse_interactive(
            &words
                .iter()
                .map(|word| (*word).to_owned())
                .collect::<Vec<_>>(),
        )
    };
    assert_eq!(args(&["--continue"]).unwrap().resume, Some(Resume::Latest));
    assert_eq!(
        args(&["-c"]).unwrap().resume,
        Some(Resume::Latest),
        "短拼写是同一个旗标"
    );
    assert_eq!(args(&[]).unwrap().resume, None, "不写就是不接着跑");
    // 它与别的旗标可以混着写，顺序无所谓。
    let mixed = args(&["--plain", "-c", "--model", "kimi-k3"]).unwrap();
    assert_eq!(mixed.resume, Some(Resume::Latest));
    assert!(mixed.plain);
    assert_eq!(mixed.model.as_deref(), Some("kimi-k3"));
}

#[test]
fn a_session_is_named_by_id_or_by_its_directory() {
    // `heng -c 20261001T155845Z-7a69cbff`：`-c` / `--continue` 后面跟一个不以 `-` 开头
    // 的词就是**指名**续哪个会话；`--session <id>` 是同一个意思的显式拼写。
    use super::interactive::{Resume, parse_interactive};

    let args = |words: &[&str]| {
        parse_interactive(
            &words
                .iter()
                .map(|word| (*word).to_owned())
                .collect::<Vec<_>>(),
        )
    };
    let named = Some(Resume::Session("20261001T155845Z-7a69cbff".to_owned()));
    assert_eq!(
        args(&["-c", "20261001T155845Z-7a69cbff"]).unwrap().resume,
        named
    );
    assert_eq!(
        args(&["--continue", "20261001T155845Z-7a69cbff"])
            .unwrap()
            .resume,
        named,
        "长拼写也吃 id"
    );
    assert_eq!(
        args(&["--session", "20261001T155845Z-7a69cbff"])
            .unwrap()
            .resume,
        named
    );
    // 会话目录也算 id（`sessions show` 一直这么收）。
    assert_eq!(
        args(&[
            "--session",
            "/tmp/sessions/bucket/20261001T155845Z-7a69cbff"
        ])
        .unwrap()
        .resume,
        Some(Resume::Session(
            "/tmp/sessions/bucket/20261001T155845Z-7a69cbff".to_owned()
        ))
    );

    // `-c` 后面那个词要是旗标，就不当 id —— 否则 `heng -c --plain` 会去续一个会话叫
    // 「--plain」的会话。
    let mixed = args(&["-c", "--plain"]).unwrap();
    assert_eq!(mixed.resume, Some(Resume::Latest));
    assert!(mixed.plain);
    // 顺序反过来也一样。
    let mixed = args(&["--plain", "-c"]).unwrap();
    assert_eq!(mixed.resume, Some(Resume::Latest));
    assert!(mixed.plain);

    // `--session` 没有可选值：缺了就是打错了，而旗标也不算 id。
    let error = args(&["--session"]).unwrap_err();
    assert!(error.contains("--session"), "{error}");
    let error = args(&["--session", "--plain"]).unwrap_err();
    assert!(error.contains("--session"), "`--plain` 不是 id：{error}");
}

#[test]
fn opening_a_session_is_newest_named_or_fresh() {
    // `choose_session` 是 `-c` 三种含义的**唯一**决定处：新开、续本工作区最新那个会话、按 id 续
    // 指名的那个会话（先本桶、再全 store，命中别处时工作目录跟着那个会话走）。
    use super::interactive::Resume;
    use super::sessions::choose_session;
    use crate::events::{Event, EventPayload, SCHEMA_VERSION, SpeakerId};
    use crate::session::SessionStore;

    let dir = tempfile::tempdir().unwrap();
    let here = dir.path().join("here");
    let there = dir.path().join("there");
    std::fs::create_dir_all(&here).unwrap();
    std::fs::create_dir_all(&there).unwrap();
    let store = SessionStore::new(dir.path().join("store"));
    // 一个会话开在哪里由它流里那条 `SessionStarted` 说 —— 目录名是编码过的桶名，读不回来。
    let seed = |session: &crate::session::store::StoredSession, cwd: &std::path::Path| {
        let event = Event::new(
            1,
            SpeakerId::System,
            EventPayload::SessionStarted {
                session_id: session.id.clone(),
                cwd: cwd.display().to_string(),
                schema_version: SCHEMA_VERSION,
                model: None,
                provider: None,
                effort: None,
                permission_mode: None,
                sandbox_mode: None,
            },
        );
        std::fs::write(
            &session.log_path,
            format!("{}\n", serde_json::to_string(&event).unwrap()),
        )
        .unwrap();
    };

    // 不带 `resume`：新开一个会话，工作目录就是请求的那个。
    let fresh = choose_session(&store, &here, None).unwrap();
    assert_eq!(fresh.cwd, here);
    assert!(!fresh.elsewhere);
    seed(&fresh.stored, &here);

    // 续最新：这个桶里最近写下的那个会话。
    let latest = choose_session(&store, &here, Some(&Resume::Latest)).unwrap();
    assert_eq!(latest.stored.id, fresh.stored.id);
    assert_eq!(latest.cwd, here);
    assert!(!latest.elsewhere);

    // 桶里一个会话都没有：报错，而不是悄悄新开一个会话 —— `-c` 是「接着跑」，接不上就要说出来。
    let error = choose_session(&store, &there, Some(&Resume::Latest)).unwrap_err();
    assert!(error.contains("没有可继续的会话"), "{error}");

    // 按 id：本桶命中，工作目录不变。
    let named = Resume::Session(fresh.stored.id.as_str().to_owned());
    let found = choose_session(&store, &here, Some(&named)).unwrap();
    assert_eq!(found.stored.id, fresh.stored.id);
    assert_eq!(found.cwd, here);
    assert!(!found.elsewhere, "本桶命中不算「跟着走了」");

    // 按 id：那个会话在**别的工作区**里 —— 找得到（id 全局唯一），而工作目录跟着它走：
    // 续写要发生在它自己的目录里，否则工具的相对路径、前缀缓存与横幅全不对。
    let other = store.create(&there).unwrap();
    seed(&other, &there);
    let elsewhere = Resume::Session(other.id.as_str().to_owned());
    let found = choose_session(&store, &here, Some(&elsewhere)).unwrap();
    assert_eq!(found.stored.id, other.id);
    assert_eq!(found.cwd, there, "续上别处的会话，工作目录跟着那个会话走");
    assert!(found.elsewhere, "这一条要让前端说一句");

    // 会话目录的路径也算 id（`sessions show` 一直这么收）。
    let by_dir = Resume::Session(other.dir.display().to_string());
    let found = choose_session(&store, &here, Some(&by_dir)).unwrap();
    assert_eq!(found.stored.id, other.id);
    assert_eq!(found.cwd, there);
    assert!(found.elsewhere);

    // 找不到：点名那个 id，并且说清搜过哪儿。
    let missing = Resume::Session("20261001T000000Z-deadbeef".to_owned());
    let error = choose_session(&store, &here, Some(&missing)).unwrap_err();
    assert!(error.contains("20261001T000000Z-deadbeef"), "{error}");
    assert!(error.contains("任何其他桶"), "说清搜过哪儿：{error}");
}

#[test]
fn the_mode_flag_overrides_the_configuration_and_its_absence_does_not() {
    use super::interactive::{InteractiveArgs, effective_mode};

    // 那一条入口点规则的两个方向：旗标设了就根本不看文件，没设时文件正是这个会话开始于的那一档
    // （`.scratch/todo-and-modes/spec.md` §1）。
    let configured = |written: Option<&str>| {
        crate::config::resolve(written, &crate::config::EnvMap::new()).unwrap()
    };
    let flag = |mode| InteractiveArgs {
        mode,
        ..Default::default()
    };

    let default = configured(None);
    assert_eq!(effective_mode(&flag(None), &default), Mode::Ask);
    assert_eq!(
        effective_mode(&flag(Some(Mode::Readonly)), &default),
        Mode::Readonly
    );

    let auto = configured(Some("[permissions]\nmode = \"auto\"\n"));
    assert_eq!(effective_mode(&flag(None), &auto), Mode::Auto);
    assert_eq!(
        effective_mode(&flag(Some(Mode::Readonly)), &auto),
        Mode::Readonly
    );
}

#[test]
fn a_skill_followed_only_by_blanks_still_only_loads() {
    assert_eq!(
        read("/review\n   \n"),
        Submission::Skill {
            name: "review".to_owned(),
            task: "/review".to_owned(),
            bare: true,
        }
    );
}

#[test]
fn anything_else_is_the_whole_message() {
    let text = "第一行\n第二行\n第三行";
    assert_eq!(read(text), Submission::Prompt(text.to_owned()));
    // 按原样保留：这条消息就是用户打下的东西，不是修剪过的版本。
    assert_eq!(
        read("  第一行\n第二行  "),
        Submission::Prompt("  第一行\n第二行  ".to_owned())
    );
}

// --- 会话内的讨论（spec §15） --------------------------------------------

#[test]
fn a_session_discussion_takes_a_question_or_leaves_it_to_the_loop() {
    // 问题就是这一行剩下的部分加上跟在它后面的一切，与技能的任务完全一样：一个讨论的问题可以是
    // 一整段。
    assert_eq!(
        read("/discuss 要不要换掉权限模型"),
        Submission::Discuss("要不要换掉权限模型".to_owned())
    );
    assert_eq!(
        read("/discuss 第一行\n第二行\n\n"),
        Submission::Discuss("第一行\n第二行".to_owned())
    );
    // 光秃秃的：循环把这个会话最后一个问题交给那两位讨论者。
    assert_eq!(read("/discuss"), Submission::Discuss(String::new()));
    assert_eq!(read("  /discuss   "), Submission::Discuss(String::new()));
    // 只有那个名字才是命令：一个叫 `discussion` 的技能就只是个技能。
    assert_eq!(
        read("/discussion x"),
        Submission::Unknown("/discussion x".to_owned())
    );
}

// --- 讨论抽取的池子（spec §15） ------------------------------------------

use super::discuss::{parse_discuss_line, pick_debaters, split_names};
use crate::config::{Debater, DiscussionRoster};

fn pool() -> DiscussionRoster {
    DiscussionRoster {
        debaters: vec![
            Debater {
                name: "保守".to_owned(),
                model: "kimi-k3".to_owned(),
                soul: Some("保守".to_owned()),
            },
            Debater {
                name: "激进".to_owned(),
                model: "deepseek-v4-pro".to_owned(),
                soul: None,
            },
            Debater {
                name: "审查".to_owned(),
                model: "deepseek-flash".to_owned(),
                soul: None,
            },
        ],
        max_rounds: None,
    }
}

fn names(pair: &[Debater; 2]) -> [String; 2] {
    [pair[0].name.clone(), pair[1].name.clone()]
}

#[test]
fn a_discussion_draws_a_pair_or_takes_the_one_it_is_given() {
    // 点名：恰好是这两位，按要的顺序。
    let picked = pick_debaters(&pool(), Some(["审查".to_owned(), "保守".to_owned()]), 0).unwrap();
    assert_eq!(names(&picked), ["审查", "保守"]);
    assert_eq!(picked[0].model, "deepseek-flash");

    // 抽的：池子里两位不同的成员，由种子决定是哪两位。
    let first = pick_debaters(&pool(), None, 0).unwrap();
    assert_eq!(names(&first), ["保守", "激进"]);
    let second = pick_debaters(&pool(), None, 2).unwrap();
    assert_eq!(names(&second), ["保守", "审查"], "种子挪动了这一对");

    // 池子里没有的名字，会说清池子里有什么。
    let error =
        pick_debaters(&pool(), Some(["保守".to_owned(), "没有".to_owned()]), 0).unwrap_err();
    assert!(error.contains("没有"), "{error}");
    assert!(error.contains("`审查`"), "{error}");
}

#[test]
fn a_discussion_command_reads_its_flags_before_the_question() {
    let line = parse_discuss_line("--debaters 保守,激进 换个角度再看").unwrap();
    assert_eq!(line.debaters, Some(["保守".to_owned(), "激进".to_owned()]));
    assert_eq!(line.question, "换个角度再看");

    // 没有要问的问题：循环补上这个会话最后一个。
    let bare = parse_discuss_line("--debaters 保守,激进").unwrap();
    assert_eq!(bare.question, "");
    assert_eq!(bare.debaters.unwrap(), ["保守", "激进"]);

    // 一个不带旗标的问题，含换行。
    let plain = parse_discuss_line("第一行\n第二行").unwrap();
    assert_eq!(plain.debaters, None);
    assert_eq!(plain.question, "第一行\n第二行");

    // `--` 之后的一切都是问题，所以一个问题可以长得像旗标。
    let after = parse_discuss_line("-- --看起来像参数的题目").unwrap();
    assert_eq!(after.question, "--看起来像参数的题目");

    assert!(
        parse_discuss_line("--nope x")
            .unwrap_err()
            .contains("--nope")
    );
    assert!(split_names("保守").unwrap_err().contains("两个名字"));
    assert_eq!(
        split_names("保守, 激进").unwrap(),
        ["保守".to_owned(), "激进".to_owned()],
        "逗号后面的空白没关系"
    );
}

#[test]
fn the_discuss_subcommand_takes_the_same_selection() {
    let parsed = discuss_args(&["--debaters", "保守,激进", "问题"]).unwrap();
    assert_eq!(parsed.debaters.unwrap(), ["保守", "激进"]);
    assert_eq!(parsed.words, vec!["问题".to_owned()]);
    assert!(
        discuss_args(&["--debaters", "只有一个", "问题"])
            .unwrap_err()
            .contains("两个名字")
    );
}

// --- 讨论的参数（spec §15） ----------------------------------------------

use super::discuss::{DiscussArgs, parse_discuss};
use super::interactive::question;

fn discuss_args(args: &[&str]) -> Result<DiscussArgs, String> {
    parse_discuss(&args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>())
}

#[test]
fn a_discussion_question_is_every_word_that_is_not_a_flag() {
    let parsed = discuss_args(&["把", "X", "换掉", "风险在哪"]).unwrap();
    assert_eq!(parsed.words.join(" "), "把 X 换掉 风险在哪");
    assert_eq!(
        question(&parsed.words).as_deref(),
        Some("把 X 换掉 风险在哪")
    );
    // 没有要问的：调用方改为读 stdin。
    assert_eq!(question(&[]), None);
    assert_eq!(question(&["   ".to_owned()]), None);
}

#[test]
fn a_discussion_reads_the_renderer_and_the_paths_it_is_given() {
    let parsed =
        discuss_args(&["--tui", "--cwd", "/tmp/x", "--config", "/tmp/c.toml", "q"]).unwrap();
    assert!(parsed.tui);
    assert!(!parsed.plain);
    assert_eq!(parsed.cwd.unwrap().to_str(), Some("/tmp/x"));
    assert_eq!(parsed.config.unwrap().to_str(), Some("/tmp/c.toml"));
    assert_eq!(parsed.words, vec!["q".to_owned()]);
}

#[test]
fn a_discussion_rejects_what_it_cannot_honour() {
    // 两个渲染器，一个进程。
    assert!(
        discuss_args(&["--plain", "--tui", "q"])
            .unwrap_err()
            .contains("互斥")
    );
    // 一个不存在的旗标，以及一个缺了值的旗标。
    assert!(
        discuss_args(&["--model", "kimi-k3"])
            .unwrap_err()
            .contains("--model")
    );
    assert!(discuss_args(&["--cwd"]).unwrap_err().contains("--cwd"));
    // 没有 `--continue`：一次讨论是一个问题、一个 harness。
    assert!(
        discuss_args(&["--continue"])
            .unwrap_err()
            .contains("--continue")
    );
}

#[test]
fn a_question_that_looks_like_a_flag_goes_after_a_double_dash() {
    let parsed = discuss_args(&["--", "--这段以横线开头"]).unwrap();
    assert_eq!(parsed.words, vec!["--这段以横线开头".to_owned()]);
}

// --- `/goal-new`（`.scratch/goal-loop/spec.md` §2） -----------------------

use super::goals::{GoalNewLine, parse_goal_new_line};

#[test]
fn goal_new_is_one_hyphenated_command_taking_a_name_and_a_source() {
    // 连字符形状：一条命令、一个词，于是 `/goal-` 一个前缀就能在 `/` 菜单里把它列出来
    // （空格形状的 `new` 补不出来）。
    assert_eq!(
        read("/goal-new sandbox .scratch/sandbox"),
        Submission::Goal("sandbox .scratch/sandbox".to_owned())
    );
    assert_eq!(
        parse_goal_new_line("sandbox .scratch/sandbox").unwrap(),
        GoalNewLine {
            name: "sandbox".to_owned(),
            sources: vec![".scratch/sandbox".to_owned()],
            force: false,
        }
    );
    assert_eq!(
        read("/goal-new sandbox .scratch/sandbox\n"),
        Submission::Goal("sandbox .scratch/sandbox".to_owned())
    );
}

#[test]
fn goal_new_takes_one_or_more_sources_in_the_order_they_are_written() {
    // 一个目标装几个 feature：第一个词是名字，剩下每一个词是一个来源。
    let line =
        parse_goal_new_line("seeds .scratch/usage-stats-format .scratch/terminal-title").unwrap();
    assert_eq!(line.name, "seeds");
    assert_eq!(
        line.sources,
        [".scratch/usage-stats-format", ".scratch/terminal-title"]
    );
    assert!(!line.force);

    // `--force` 照旧可以出现在任何位置，而且不占来源的位置。
    let forced = parse_goal_new_line("--force seeds .scratch/a .scratch/b").unwrap();
    assert!(forced.force);
    assert_eq!(forced.sources, [".scratch/a", ".scratch/b"]);

    assert_eq!(
        read("/goal-new seeds .scratch/a .scratch/b"),
        Submission::Goal("seeds .scratch/a .scratch/b".to_owned())
    );
}

#[test]
fn goal_new_without_a_name_and_a_source_is_a_usage_error() {
    for args in ["", "sandbox", "sandbox --force"] {
        assert!(
            parse_goal_new_line(args).is_err(),
            "`{args}` 不是一条合法的 /goal-new"
        );
    }
    // 退役的空格形状是未知命令，与 `/plan` 一样的读法：命令名就是命令名。
    assert_eq!(
        read("/goal new sandbox .scratch/sandbox"),
        Submission::Unknown("/goal new sandbox .scratch/sandbox".to_owned())
    );
}

// --- `/clear`（`.scratch/goal-loop/spec.md` §12） --------------------------

#[test]
fn clear_is_a_whole_submission_and_not_a_task() {
    assert_eq!(read("/clear"), Submission::Clear);
    assert_eq!(read("  /clear  "), Submission::Clear, "去掉空白照旧");
    // 内建命令要么是整条提交，要么什么都不算：它后面的一行绝不能被丢在地上。
    assert_eq!(
        read("/clear\n再写点什么"),
        Submission::Prompt("/clear\n再写点什么".to_owned())
    );
}

// --- 无人值守的前提：档位（`.scratch/goal-loop/spec.md` §5） ---------------

/// 一个从不被调用的 provider：拒绝发生在任何一次调用之前，所以这里被调到就是测试失败。
struct NoCalls;

#[async_trait::async_trait]
impl crate::provider::Provider for NoCalls {
    async fn send(
        &self,
        _request: crate::provider::ChatRequest,
    ) -> Result<crate::provider::EventStream, crate::provider::ProviderError> {
        Err(crate::provider::ProviderError::Transport {
            detail: "这个测试不该调到 provider".to_owned(),
        })
    }

    fn caps(&self) -> crate::provider::capability::ModelCaps {
        crate::provider::capability::caps_for("deepseek-flash").expect("内置模型")
    }
}

#[tokio::test]
async fn an_ask_mode_session_refuses_a_goal_loop_before_writing_anything() {
    let (_cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<CliCommand>(8);
    let (requests, _requests_rx) = tokio::sync::mpsc::unbounded_channel::<CliRequest>();
    // 票 06 的 e2e：`ask` 档下 `/loop` 拒绝，而**流上没有** `GoalSelected` —— 拒绝发生在写
    // 任何事件之前。走的是**组装入口 + 假 provider**，也就是 spec 说的那条接缝。
    use super::goals::GoalSetup;
    use super::interactive::run_goal_loop;
    use crate::config::SessionConfig;
    use crate::events::{EventPayload, SpeakerId};
    use crate::permissions::Policy;
    use crate::render::{RenderSinks, Renderer};
    use crate::session::SessionStore;
    use crate::tools::{self, PathLocks};
    use crate::{AssemblyParts, SessionScaffold, assemble};

    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let goals_dir = dir.path().join("goals");
    crate::goals::create(
        "sandbox",
        &[std::path::PathBuf::from(".scratch/sandbox")],
        &goals_dir,
        false,
    )
    .expect("清单从票生成");

    let store = SessionStore::new(dir.path().join("store"));
    let stored = store.create(&workspace).unwrap();
    let mut harness = assemble(AssemblyParts {
        provider: Box::new(NoCalls),
        speaker: SpeakerId::Debater("kimi".into()),
        config: SessionConfig::new("fake-model"),
        render: crate::render::RenderChannel::start(
            Renderer::headless(RenderSinks {
                stdout_result: Box::new(std::io::sink()),
                stderr_diagnostic: Box::new(std::io::sink()),
            }),
            stored.id.clone(),
        ),
        scaffold: SessionScaffold {
            cwd: workspace.clone(),
            log_path: stored.log_path.clone(),
            session_id: stored.id.clone(),
            tools: tools::builtin(false),
            locks: PathLocks::new(),
            policy: Policy::for_mode(Mode::Ask),
            asker: None,
            questions: None,
            hook: None,
            home: None,
        },
    })
    .await
    .expect("组装一个会话 ask 档的会话");

    let (console, _port, _events) = crate::render::console();
    let goals = GoalSetup {
        dir: Some(goals_dir),
        store,
        cwd: workspace,
        settings: crate::config::GoalSettings::default(),
    };
    let mut running = false;
    let refusal = run_goal_loop(
        &mut harness,
        &console,
        &mut cmd_rx,
        &requests,
        &goals,
        "sandbox",
        &mut running,
        &mut ExitRequest::default(),
    )
    .await
    .expect_err("ask 档下拒绝启动");

    assert!(refusal.contains("无人值守"), "{refusal}");
    assert!(refusal.contains("workspace"), "要说清换哪一档：{refusal}");
    assert!(!running, "被拒的启动没有把循环标成在跑");
    assert!(
        !harness
            .events()
            .iter()
            .any(|event| matches!(event.payload, EventPayload::GoalSelected { .. })),
        "拒绝发生在写事件之前：流上没有归属"
    );

    harness.shutdown().await;
}

#[tokio::test]
async fn an_auto_mode_session_starts_the_loop_and_lands_its_ownership_on_the_stream() {
    let (_cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<CliCommand>(8);
    let (requests, _requests_rx) = tokio::sync::mpsc::unbounded_channel::<CliRequest>();
    // 另一半 e2e：档位够了就真的启动 —— 归属落流、清单注入，然后跑一个回合。provider 这里
    // 每一次都失败，而 `provider_retries = 0` 让循环当场停下（不睡那两次重试），于是这条
    // 用例既证明「能启动」，也把「provider 失败 → 停下 + 收尾事件」走了一遍。
    use super::goals::GoalSetup;
    use super::interactive::run_goal_loop;
    use crate::config::SessionConfig;
    use crate::events::{EventPayload, SpeakerId};
    use crate::permissions::Policy;
    use crate::render::{RenderSinks, Renderer};
    use crate::session::SessionStore;
    use crate::tools::{self, PathLocks};
    use crate::{AssemblyParts, SessionScaffold, assemble};

    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let goals_dir = dir.path().join("goals");
    crate::goals::create(
        "sandbox",
        &[std::path::PathBuf::from(".scratch/sandbox")],
        &goals_dir,
        false,
    )
    .expect("清单从票生成");

    let store = SessionStore::new(dir.path().join("store"));
    let stored = store.create(&workspace).unwrap();
    let mut harness = assemble(AssemblyParts {
        provider: Box::new(NoCalls),
        speaker: SpeakerId::Debater("kimi".into()),
        config: SessionConfig::new("fake-model"),
        render: crate::render::RenderChannel::start(
            Renderer::headless(RenderSinks {
                stdout_result: Box::new(std::io::sink()),
                stderr_diagnostic: Box::new(std::io::sink()),
            }),
            stored.id.clone(),
        ),
        scaffold: SessionScaffold {
            cwd: workspace.clone(),
            log_path: stored.log_path.clone(),
            session_id: stored.id.clone(),
            tools: tools::builtin(false),
            locks: PathLocks::new(),
            policy: Policy::for_mode(Mode::Auto),
            asker: None,
            questions: None,
            hook: None,
            home: None,
        },
    })
    .await
    .expect("组装一个会话 auto 档的会话");

    let (console, _port, _events) = crate::render::console();
    let goals = GoalSetup {
        dir: Some(goals_dir),
        store,
        cwd: workspace,
        settings: crate::config::GoalSettings {
            provider_retries: 0,
            ..crate::config::GoalSettings::default()
        },
    };
    let mut running = false;
    run_goal_loop(
        &mut harness,
        &console,
        &mut cmd_rx,
        &requests,
        &goals,
        "sandbox",
        &mut running,
        &mut ExitRequest::default(),
    )
    .await
    .expect("auto 档下启动不拒绝");

    let written = harness.events();
    assert!(
        written
            .iter()
            .any(|event| matches!(event.payload, EventPayload::GoalSelected { .. })),
        "启动之后归属落流"
    );
    assert!(
        written
            .iter()
            .any(|event| matches!(event.payload, EventPayload::ContextInjected { .. })),
        "清单也注入了上下文"
    );
    assert!(
        written.iter().any(|event| matches!(
            &event.payload,
            EventPayload::GoalStopped { reason, count, .. }
                if *reason == crate::events::GoalStopReason::ProviderFailed && *count == 1
        )),
        "provider 失败一次就停下报告：{written:?}"
    );
    assert!(!running, "停下之后回到空闲");

    harness.shutdown().await;
}

#[tokio::test]
async fn the_budget_report_names_only_the_entries_that_are_still_open() {
    let (_cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<CliCommand>(8);
    let (requests, _requests_rx) = tokio::sync::mpsc::unbounded_channel::<CliRequest>();
    // 报告里点名的必须是**这一刻**没完成的那几条。进度由 `stop_goal_loop` 自己派生，所以
    // 这里钉的是它算对了：预造一条「已经认领目标、且已把 01 标完成」的流，再让额度为 0 ——
    // 循环在第一个回合开头就撞顶，报告不许再把 01 列成「还卡着」（它第一次真机跑出来时正是
    // 这么误导的：11 条全列上，而 01、02 其实已经做完）。
    use super::goals::GoalSetup;
    use super::interactive::run_goal_loop;
    use crate::config::SessionConfig;
    use crate::events::{
        Event, EventPayload, GoalStopReason, SCHEMA_VERSION, SpeakerId, ToolCallId,
    };
    use crate::permissions::Policy;
    use crate::render::{RenderSinks, Renderer};
    use crate::session::SessionStore;
    use crate::tools::{self, PathLocks};
    use crate::{AssemblyParts, SessionScaffold, assemble};

    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let goals_dir = dir.path().join("goals");
    crate::goals::create(
        "sandbox",
        &[std::path::PathBuf::from(".scratch/sandbox")],
        &goals_dir,
        false,
    )
    .expect("清单从票生成");

    let store = SessionStore::new(dir.path().join("store"));
    let stored = store.create(&workspace).unwrap();
    let seeded = [
        Event::new(
            1,
            SpeakerId::System,
            EventPayload::SessionStarted {
                session_id: stored.id.clone(),
                cwd: workspace.display().to_string(),
                schema_version: SCHEMA_VERSION,
                model: None,
                provider: None,
                effort: None,
                permission_mode: None,
                sandbox_mode: None,
            },
        ),
        Event::new(
            2,
            SpeakerId::System,
            EventPayload::GoalSelected {
                goal: "sandbox".to_owned(),
            },
        ),
        Event::new(
            3,
            SpeakerId::Debater("kimi".into()),
            EventPayload::ToolCallStarted {
                tool_call_id: ToolCallId::new("call-1"),
                tool_name: tools::TODO_TOOL.to_owned(),
                args: serde_json::json!({
                    "items": [{"id": "01", "content": "一条", "status": "completed"}]
                }),
            },
        ),
        Event::new(
            4,
            SpeakerId::Debater("kimi".into()),
            EventPayload::ToolCallCompleted {
                tool_call_id: ToolCallId::new("call-1"),
                ok: true,
                output: Some("todo：1 项（1 项已完成）".to_owned()),
                error: None,
                duration_ms: 1,
            },
        ),
    ];
    let mut jsonl = String::new();
    for event in &seeded {
        jsonl.push_str(&serde_json::to_string(event).unwrap());
        jsonl.push('\n');
    }
    std::fs::write(&stored.log_path, jsonl).unwrap();

    let mut harness = assemble(AssemblyParts {
        provider: Box::new(NoCalls),
        speaker: SpeakerId::Debater("kimi".into()),
        config: SessionConfig::new("fake-model").with_session_token_limit(0),
        render: crate::render::RenderChannel::start(
            Renderer::headless(RenderSinks {
                stdout_result: Box::new(std::io::sink()),
                stderr_diagnostic: Box::new(std::io::sink()),
            }),
            stored.id.clone(),
        ),
        scaffold: SessionScaffold {
            cwd: workspace.clone(),
            log_path: stored.log_path.clone(),
            session_id: stored.id.clone(),
            tools: tools::builtin(false),
            locks: PathLocks::new(),
            policy: Policy::for_mode(Mode::Auto),
            asker: None,
            questions: None,
            hook: None,
            home: None,
        },
    })
    .await
    .expect("组装一个接手的会话");

    let (console, _port, _front_end) = crate::render::console();
    let goals = GoalSetup {
        dir: Some(goals_dir),
        store,
        cwd: workspace,
        settings: crate::config::GoalSettings::default(),
    };
    let mut running = false;
    run_goal_loop(
        &mut harness,
        &console,
        &mut cmd_rx,
        &requests,
        &goals,
        "sandbox",
        &mut running,
        &mut ExitRequest::default(),
    )
    .await
    .expect("额度为 0 是一次正常的降级收尾");

    let (reason, stuck) = harness
        .events()
        .into_iter()
        .find_map(|event| match event.payload {
            EventPayload::GoalStopped { reason, stuck, .. } => Some((reason, stuck)),
            _ => None,
        })
        .expect("撞顶落了一条收尾");
    assert_eq!(reason, GoalStopReason::BudgetExhausted);
    assert!(
        !stuck.contains(&"01".to_owned()),
        "01 已经做完，不该出现在「还卡着」里：{stuck:?}"
    );
    assert!(
        stuck.contains(&"02".to_owned()),
        "没做的那些要在里面：{stuck:?}"
    );

    harness.shutdown().await;
}

// --- `/` 菜单（`.scratch/goal-loop/spec.md` §13） --------------------------

use super::interactive::slash_catalog;
use crate::render::wording::BUILT_IN_COMMANDS;

#[test]
fn the_menu_lists_the_built_in_commands_before_the_skills() {
    // 命令一组、技能一组；同前缀时命令优先，因为程序自带的那批更可预测。
    let skills = [
        ("clear-ish", "一个与 /c 同前缀的技能"),
        ("ask-matt", "审一遍"),
    ];
    let entries = slash_catalog(&BUILT_IN_COMMANDS, &skills, &[]);
    let names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();

    let declared: Vec<&str> = BUILT_IN_COMMANDS
        .iter()
        .map(|command| command.name)
        .collect();
    assert_eq!(
        &names[..declared.len()],
        declared.as_slice(),
        "命令在前，组内就是声明顺序（不按字母）"
    );
    assert_eq!(
        &names[declared.len()..],
        ["ask-matt", "clear-ish"],
        "技能在后，按名字"
    );

    let clear = names.iter().position(|name| *name == "clear").unwrap();
    let skill = names.iter().position(|name| *name == "clear-ish").unwrap();
    assert!(clear < skill, "同前缀时命令优先：{names:?}");
}

#[test]
fn the_menu_covers_exactly_the_commands_the_loop_parses() {
    // 防「加了命令忘了补全」：`submission()` 认哪些名字，`/` 菜单就列出哪些。两边各有一份
    // 真相（参数形状只能各自解析），所以一致性由这条测试钉住。
    let mut parsed: Vec<&str> = Vec::new();
    for name in [
        "undo", "discuss", "goal-new", "loop", "model", "effort", "clear", "quit", "exit",
    ] {
        let line = format!("/{name}");
        let submission = read(&line);
        assert!(
            !matches!(submission, Submission::Unknown(_) | Submission::Prompt(_)),
            "{line} 是一条内建命令，不是未知命令、也不是 prompt"
        );
        parsed.push(name);
    }
    let listed: Vec<&str> = BUILT_IN_COMMANDS
        .iter()
        .map(|command| command.name)
        .collect();
    assert_eq!(
        listed, parsed,
        "补全里列的就是循环解析的那些，一个不多一个不少"
    );

    // 一个真不存在的名字仍然是未知命令。
    assert_eq!(read("/nope"), Submission::Unknown("/nope".to_owned()));
}

// --- `/loop <名字>`（`.scratch/goal-loop/spec.md` §4） ---------------------

use super::goals::parse_loop_line;

#[test]
fn loop_takes_exactly_one_name() {
    assert_eq!(parse_loop_line(" sandbox ").unwrap(), "sandbox");
    assert_eq!(
        read("/loop sandbox"),
        Submission::Loop("sandbox".to_owned())
    );

    // 名字是一个不断开的词，所以一个都没有、多出一个都是用法错误 —— 而这条命令读到的
    // 是整条提交，与 `/discuss`、`/goal-new` 同一条规矩。
    for args in ["", "   ", "sandbox extra", "两个 词"] {
        let message = parse_loop_line(args).unwrap_err();
        assert!(message.contains("/loop"), "`{args}`：{message}");
    }
    assert_eq!(
        read("/loop sandbox\n"),
        Submission::Loop("sandbox".to_owned())
    );
}

/// 装载问的是**流上**记着的那个工作区，不是会话目录（票 33）。
#[test]
fn attaching_asks_the_stream_for_the_workspace() {
    let events = vec![crate::events::Event::new(
        1,
        SpeakerId::System,
        crate::events::EventPayload::SessionStarted {
            session_id: SessionId::new("s-1"),
            cwd: "/workspace".to_owned(),
            schema_version: crate::events::SCHEMA_VERSION,
            model: None,
            provider: None,
            effort: None,
            permission_mode: None,
            sandbox_mode: None,
        },
    )];
    assert_eq!(
        super::loading::attach_cwd(&events),
        Some(PathBuf::from("/workspace"))
    );
}

/// 空串不算「记住了工作区」：拒装，而不是拿一个空路径去问它在不在（票 33）。
#[test]
fn an_empty_recorded_workspace_is_no_workspace() {
    let events = vec![crate::events::Event::new(
        1,
        SpeakerId::System,
        crate::events::EventPayload::SessionStarted {
            session_id: SessionId::new("s-1"),
            cwd: String::new(),
            schema_version: crate::events::SCHEMA_VERSION,
            model: None,
            provider: None,
            effort: None,
            permission_mode: None,
            sandbox_mode: None,
        },
    )];
    assert_eq!(super::loading::attach_cwd(&events), None);
}

/// 计划表的六格：宿主表 → 那三档检查 → 流里有没有记下工作区（票 34）。
#[test]
fn the_attach_plan_reads_the_host_table_the_checks_and_the_workspace() {
    let id = SessionId::new("s-1");
    let cwd = PathBuf::from("/workspace");
    let ready = crate::session::AttachCheck::Ready;

    // 已在宿主表：切前台，不装载。
    assert_eq!(
        super::loading::attach_plan(
            super::loading::AttachKind::Existing,
            true,
            &id,
            Some(cwd.clone()),
            ready.clone()
        ),
        super::loading::AttachPlan::SwitchOnly { id: id.clone() }
    );
    // 不在表、cwd 有、检查放行：装它 —— 而「正在装载」只在盘上那条路说。
    assert_eq!(
        super::loading::attach_plan(
            super::loading::AttachKind::Existing,
            false,
            &id,
            Some(cwd.clone()),
            ready.clone()
        ),
        super::loading::AttachPlan::Attach {
            id: id.clone(),
            cwd: cwd.clone(),
            notices: vec![super::loading::Notice::Loading],
        }
    );
    assert_eq!(
        super::loading::attach_plan(
            super::loading::AttachKind::New,
            false,
            &id,
            Some(cwd.clone()),
            ready.clone()
        ),
        super::loading::AttachPlan::Attach {
            id: id.clone(),
            cwd: cwd.clone(),
            notices: Vec::new(),
        }
    );
    // 拒装三档：`check` 先说话，再轮到「流里没记下工作区」。
    assert_eq!(
        super::loading::attach_plan(
            super::loading::AttachKind::Existing,
            false,
            &id,
            Some(cwd.clone()),
            crate::session::AttachCheck::MissingCwd(PathBuf::from("/gone")),
        ),
        super::loading::AttachPlan::Refuse {
            reason: super::loading::Refusal::CwdGone(PathBuf::from("/gone")),
        }
    );
    assert_eq!(
        super::loading::attach_plan(
            super::loading::AttachKind::New,
            false,
            &id,
            Some(cwd.clone()),
            crate::session::AttachCheck::Unreadable("坏行".to_owned()),
        ),
        super::loading::AttachPlan::Refuse {
            reason: super::loading::Refusal::Unreadable("坏行".to_owned()),
        }
    );
    assert_eq!(
        super::loading::attach_plan(
            super::loading::AttachKind::Existing,
            false,
            &id,
            None,
            ready
        ),
        super::loading::AttachPlan::Refuse {
            reason: super::loading::Refusal::NoCwd,
        }
    );
}
