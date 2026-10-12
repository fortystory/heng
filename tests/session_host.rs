//! 宿主表与前台指针：一场会话 = 一个 `Harness` + 一个拥有它的 task
//! （`.scratch/session-tab/spec.md` §1、票 15 的宿主接缝）。
//!
//! 这一层用**真的** `Harness` 驱动（假 provider 照脚本应答），因为要证的正是「表里那一行
//! 真的拥有那一场会话」—— task 收命令、跑回合、把状态写回 `watch`。

mod support;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;

use heng::events::{EventPayload, SessionId, SpeakerId, read_events};
use heng::permissions::{Mode, Policy};
use heng::provider::StreamEvent;
use heng::render::{RenderSinks, Renderer};
use heng::session::host::{SessionActor, SessionCommand, SessionHost, SessionState, SessionTurn};
use heng::tools::{PathLocks, builtin};
use heng::{AssemblyParts, Harness, SessionScaffold, assemble};
use support::{CaptureBuf, FakeProvider, Reply};
use tokio::sync::Notify;

/// 一场挂在 `root` 下的真会话，外加它的日志路径。
struct Hosted {
    harness: Harness,
    log_path: PathBuf,
    cwd: PathBuf,
}

async fn session(root: &Path, id: &str, replies: Vec<Reply>) -> Hosted {
    let session_dir = root.join(id);
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&session_dir).unwrap();
    std::fs::create_dir_all(&workspace).unwrap();
    let log_path = session_dir.join("log.jsonl");
    let harness = assemble(AssemblyParts {
        provider: Box::new(FakeProvider::new(replies)),
        speaker: SpeakerId::Debater("kimi".into()),
        config: heng::config::SessionConfig::new("fake-model"),
        render: heng::render::RenderChannel::start(
            Renderer::headless(RenderSinks {
                stdout_result: Box::new(CaptureBuf::default()),
                stderr_diagnostic: Box::new(CaptureBuf::default()),
            }),
            SessionId::new(id),
        ),
        scaffold: SessionScaffold {
            cwd: workspace.clone(),
            log_path: log_path.clone(),
            session_id: SessionId::new(id),
            tools: builtin(false),
            locks: PathLocks::new(),
            policy: Policy::for_mode(Mode::Auto),
            asker: None,
            questions: None,
            hook: None,
            home: None,
        },
    })
    .await
    .unwrap();
    Hosted {
        harness,
        log_path,
        cwd: workspace,
    }
}

fn turn(text: &str) -> SessionCommand {
    SessionCommand::Turn(SessionTurn::Prompt(text.to_owned()))
}

#[tokio::test]
async fn the_same_session_id_can_only_have_one_row() {
    let dir = tempfile::tempdir().unwrap();
    let first = session(dir.path(), "s-1", vec![Reply::text("在")]).await;
    let mut host = SessionHost::new(SessionId::new("s-1"));
    host.adopt(first.harness, first.cwd).unwrap();
    assert_eq!(host.len(), 1);

    // 同一个 id 再入表一次会被拒：同一路径两份 `EventLog` 会各自维护 `next_seq`。
    let again = session(dir.path(), "s-1", vec![Reply::text("在")]).await;
    let refused = host.adopt(again.harness, again.cwd);
    assert!(refused.is_err(), "同一个会话只能有一行");
    assert_eq!(host.len(), 1);
}

#[tokio::test]
async fn a_turn_runs_in_the_task_and_the_state_comes_back_to_idle() {
    let dir = tempfile::tempdir().unwrap();
    let hosted = session(dir.path(), "s-1", vec![Reply::text("收到")]).await;
    let log_path = hosted.log_path.clone();
    let mut host = SessionHost::new(SessionId::new("s-1"));
    host.adopt(hosted.harness, hosted.cwd).unwrap();

    let id = SessionId::new("s-1");
    host.get(&id).unwrap().cmd.send(turn("你好")).await.unwrap();
    wait_for_text(&log_path, "收到").await;
    // 那一场自己的流上落了这一回合：task 真的拥有它。
    let events = read_events(&log_path).unwrap();
    assert!(
        events.iter().any(|event| matches!(
            &event.payload,
            EventPayload::MessageCompleted { text, .. } if text.contains("收到")
        )),
        "回合的结果落在这一场自己的流上：{events:?}"
    );
}

/// 等这一场的流上出现一段文本。
///
/// 跨 task 的完成信号只有流本身（状态通道会「保最新不丢」，所以一次短回合可能整段错过
/// `RunningTurn`）—— 于是这里轮询它，而不是猜一个时长。
async fn wait_for_text(path: &Path, needle: &str) {
    for _ in 0..2_000 {
        let hit = read_events(path)
            .map(|events| {
                events.iter().any(|event| {
                    matches!(
                        &event.payload,
                        EventPayload::MessageCompleted { text, .. } if text.contains(needle)
                    )
                })
            })
            .unwrap_or(false);
        if hit {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("流上没等到 {needle}");
}

/// `wait_for` 的借用版：拿的是接收端本身，而不是整行。
async fn wait_for_entry(
    state: &mut tokio::sync::watch::Receiver<SessionState>,
    want: SessionState,
) {
    for _ in 0..10_000 {
        if *state.borrow() == want {
            return;
        }
        if state.changed().await.is_err() {
            break;
        }
    }
    panic!("没等到状态 {want:?}");
}

#[tokio::test]
async fn a_running_turn_shows_up_in_the_state_and_the_running_count() {
    // 一条永不结束的流把回合**停在** `RunningTurn` 上：取消把它收下来。
    let dir = tempfile::tempdir().unwrap();
    let opened = Arc::new(Notify::new());
    let hosted = session(
        dir.path(),
        "s-1",
        vec![Reply::Stall(
            Arc::clone(&opened),
            vec![StreamEvent::TextDelta("半句话".to_owned())],
        )],
    )
    .await;
    let mut host = SessionHost::new(SessionId::new("s-1"));
    host.adopt(hosted.harness, hosted.cwd).unwrap();
    let id = SessionId::new("s-1");

    host.get(&id)
        .unwrap()
        .cmd
        .send(turn("跑起来"))
        .await
        .unwrap();
    let mut state = host.get(&id).unwrap().state.clone();
    wait_for_entry(&mut state, SessionState::RunningTurn).await;
    assert_eq!(host.running(), 1, "读数数的是在跑的那几场");

    // 取消：回合收尾，状态回到空闲。
    host.get(&id).unwrap().cancel.cancel();
    wait_for_entry(&mut state, SessionState::Idle).await;
    assert_eq!(host.running(), 0);
}

#[tokio::test]
async fn switching_only_moves_the_pointer() {
    let dir = tempfile::tempdir().unwrap();
    let first = session(dir.path(), "s-1", vec![Reply::text("在")]).await;
    let second = session(dir.path(), "s-2", vec![Reply::text("在")]).await;
    let mut host = SessionHost::new(SessionId::new("s-1"));
    host.adopt(first.harness, first.cwd).unwrap();
    host.adopt(second.harness, second.cwd).unwrap();

    assert_eq!(host.foreground().as_str(), "s-1");
    let mut follow = host.follow_foreground();
    host.switch(&SessionId::new("s-2"));
    follow.changed().await.unwrap();
    assert_eq!(host.foreground().as_str(), "s-2");
    assert_eq!(host.len(), 2, "切换不卸载任何一场");
}

#[tokio::test]
async fn closing_takes_the_row_out_and_ends_its_task() {
    let dir = tempfile::tempdir().unwrap();
    let hosted = session(dir.path(), "s-1", vec![Reply::text("在")]).await;
    let mut host = SessionHost::new(SessionId::new("s-1"));
    host.adopt(hosted.harness, hosted.cwd).unwrap();
    let id = SessionId::new("s-1");

    host.close(&id).await.unwrap();
    assert!(host.is_empty());
    assert!(!host.contains(&id));
    // 关掉之后它不再是前台也没关系：表空了就是空宿主态（票 21）。
    let refused = host.close(&id).await;
    assert!(refused.is_err(), "不在表里的关不掉");
}

#[tokio::test]
async fn two_sessions_run_their_own_turns() {
    let dir = tempfile::tempdir().unwrap();
    let first = session(dir.path(), "s-1", vec![Reply::text("一号回答")]).await;
    let second = session(dir.path(), "s-2", vec![Reply::text("二号回答")]).await;
    let first_log = first.log_path.clone();
    let second_log = second.log_path.clone();
    let mut host = SessionHost::new(SessionId::new("s-1"));
    host.adopt(first.harness, first.cwd).unwrap();
    host.adopt(second.harness, second.cwd).unwrap();

    host.get(&SessionId::new("s-1"))
        .unwrap()
        .cmd
        .send(turn("一号"))
        .await
        .unwrap();
    host.get(&SessionId::new("s-2"))
        .unwrap()
        .cmd
        .send(turn("二号"))
        .await
        .unwrap();

    wait_for_text(&first_log, "一号回答").await;
    wait_for_text(&second_log, "二号回答").await;
    let text_of = |path: &Path| {
        read_events(path)
            .unwrap()
            .into_iter()
            .filter_map(|event| match event.payload {
                EventPayload::MessageCompleted { text, .. } => Some(text),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let first_text = text_of(&first_log);
    let second_text = text_of(&second_log);
    assert!(first_text.contains("一号回答"), "{first_text}");
    assert!(!first_text.contains("二号回答"), "两场的流不串");
    assert!(second_text.contains("二号回答"), "{second_text}");
    assert!(!second_text.contains("一号回答"), "两场的流不串");
}

#[tokio::test]
async fn a_read_only_preview_keeps_its_three_states() {
    // 第二档损坏流（半读）装起来是一场**只读预览**：表里多一个标志，三态不动。
    let dir = tempfile::tempdir().unwrap();
    let hosted = session(dir.path(), "s-1", vec![Reply::text("在")]).await;
    let mut host = SessionHost::new(SessionId::new("s-1"));
    host.adopt_read_only(hosted.harness, hosted.cwd).unwrap();
    let entry = host.get(&SessionId::new("s-1")).unwrap();
    assert!(entry.read_only, "只读标志在表里");
    assert_eq!(
        *entry.state.borrow(),
        SessionState::Idle,
        "只读不是第四个状态"
    );
}

#[tokio::test]
async fn the_attach_check_refuses_to_load_what_is_already_hosted() {
    use heng::session::AttachCheck;
    use heng::session::attach::check;

    let dir = tempfile::tempdir().unwrap();
    let hosted = session(dir.path(), "s-1", vec![Reply::text("在")]).await;
    let log_path = hosted.log_path.clone();
    let workspace = hosted.cwd.clone();
    let mut host = SessionHost::new(SessionId::new("s-1"));
    host.adopt(hosted.harness, hosted.cwd).unwrap();

    assert_eq!(
        check(&host, &SessionId::new("s-1"), Some(&workspace), &log_path),
        AttachCheck::AlreadyHosted(SessionId::new("s-1")),
        "已经在表里的一场绝不重走装载：那会把进行中的工具调用读成崩溃遗留"
    );
}

#[tokio::test]
async fn a_loaded_session_can_be_given_a_fresh_facts_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let mut hosted = session(dir.path(), "s-1", vec![Reply::text("在")]).await;
    let log_path = hosted.log_path.clone();
    hosted.harness.record_facts().unwrap();

    let events = read_events(&log_path).unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event.payload, EventPayload::SessionFactsChanged { .. })),
        "装载之后补的那一条快照落在流上：{events:?}"
    );
}

/// 一个记账用的解释器：把到达的命令记下来，并且**自己**跑一次 `Turn`
/// （缺省的 `TurnActor` 干的就是这一件事）。
struct RecordingActor {
    seen: Arc<Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl SessionActor for RecordingActor {
    async fn act(
        &mut self,
        harness: &mut Harness,
        command: SessionCommand,
        _inbox: &mut tokio::sync::mpsc::Receiver<SessionCommand>,
    ) -> bool {
        let label = match &command {
            SessionCommand::Turn(_) => "turn".to_owned(),
            other => format!("{other:?}"),
        };
        self.seen
            .lock()
            .unwrap()
            .push(format!("{}:{label}", harness.session_id()));
        if let SessionCommand::Turn(SessionTurn::Prompt(text)) = command {
            let _ = harness.run_turn(&text).await;
        }
        true
    }
}

#[tokio::test]
async fn the_host_hands_commands_to_the_actor_that_owns_the_session() {
    // 解释器拿到的 harness 就是这一场：它报得出自己的 id，也真的能把回合跑完。
    let dir = tempfile::tempdir().unwrap();
    let hosted = session(dir.path(), "s-1", vec![Reply::text("收到")]).await;
    let log_path = hosted.log_path.clone();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut host = SessionHost::new(SessionId::new("s-1"));
    host.adopt_with(
        hosted.harness,
        hosted.cwd,
        false,
        Box::new(RecordingActor {
            seen: Arc::clone(&seen),
        }),
    )
    .unwrap();

    let id = SessionId::new("s-1");
    host.get(&id).unwrap().cmd.send(turn("你好")).await.unwrap();
    wait_for_text(&log_path, "收到").await;
    assert_eq!(seen.lock().unwrap().as_slice(), ["s-1:turn"]);
}

#[tokio::test]
async fn the_front_facts_snapshot_carries_what_the_loop_parses_with() {
    // 主循环解析每一次提交都要这几样，而装好之后 `Harness` 就归 task 了 —— 于是先取一份。
    let dir = tempfile::tempdir().unwrap();
    let hosted = session(dir.path(), "s-1", vec![Reply::text("在")]).await;
    let facts = hosted.harness.front_facts();
    assert_eq!(facts.id.as_str(), "s-1");
    assert_eq!(facts.model, "fake-model");
    assert_eq!(facts.effort, None, "没给档位就是「默认」那一档");
    assert!(
        facts.skills.is_empty(),
        "临时工作区里没有技能：{:?}",
        facts.skills
    );
    assert_eq!(facts.goal, None);
}

/// 与 [`session`] 同一场，但走**装载**那条路（`load`）：它会补一条会话事实快照、并判只读。
async fn load_session(root: &Path, id: &str, replies: Vec<Reply>) -> (heng::Loaded, PathBuf) {
    let session_dir = root.join(id);
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&session_dir).unwrap();
    std::fs::create_dir_all(&workspace).unwrap();
    let log_path = session_dir.join("log.jsonl");
    let loaded = heng::load(AssemblyParts {
        provider: Box::new(FakeProvider::new(replies)),
        speaker: SpeakerId::Debater("kimi".into()),
        config: heng::config::SessionConfig::new("fake-model"),
        render: heng::render::RenderChannel::start(
            Renderer::headless(RenderSinks {
                stdout_result: Box::new(CaptureBuf::default()),
                stderr_diagnostic: Box::new(CaptureBuf::default()),
            }),
            SessionId::new(id),
        ),
        scaffold: SessionScaffold {
            cwd: workspace,
            log_path: log_path.clone(),
            session_id: SessionId::new(id),
            tools: builtin(false),
            locks: PathLocks::new(),
            policy: Policy::for_mode(Mode::Auto),
            asker: None,
            questions: None,
            hook: None,
            home: None,
        },
    })
    .await
    .unwrap();
    (loaded, log_path)
}

#[tokio::test]
async fn loading_a_session_snapshots_its_facts() {
    let dir = tempfile::tempdir().unwrap();
    let (loaded, log_path) = load_session(dir.path(), "s-1", vec![Reply::text("在")]).await;
    assert!(!loaded.read_only, "好流不是只读预览");
    assert!(
        loaded.filled.is_empty(),
        "新流五格齐全：{:?}",
        loaded.filled
    );
    let events = read_events(&log_path).unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event.payload, EventPayload::SessionFactsChanged { .. })),
        "装载之后补了一条完整快照：{events:?}"
    );
}

#[tokio::test]
async fn a_half_read_stream_loads_as_a_read_only_preview() {
    // 中间有坏行的流：严格那条路会直接报错，而它该降级成只读预览、不是装不起来（票 18）。
    let dir = tempfile::tempdir().unwrap();
    let (_, log_path) = load_session(dir.path(), "s-1", vec![Reply::text("在")]).await;
    let lines: Vec<String> = std::fs::read_to_string(&log_path)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    let mut patched = String::new();
    for (index, line) in lines.iter().enumerate() {
        if index == 1 {
            patched.push_str("这不是一行 JSON\n");
        }
        patched.push_str(line);
        patched.push('\n');
    }
    std::fs::write(&log_path, patched).unwrap();

    let (loaded, _) = load_session(dir.path(), "s-1", vec![Reply::text("在")]).await;
    assert!(loaded.read_only, "半读的流装起来是只读预览");
    // 读得出来的那些历史仍在（宽松那一档跳过了坏行）。
    assert!(
        loaded
            .harness
            .events()
            .iter()
            .any(|event| matches!(event.payload, EventPayload::SessionStarted { .. })),
        "坏行被跳过，前面那段历史还在"
    );
}

#[tokio::test]
async fn loading_a_second_session_hosts_it_and_moves_the_foreground() {
    // 票 20 装载那条路的机制：不在表里的一场走 `load`（票 18 的共用路径）装起来、入表、
    // 切前台 —— 而**旧的留在表里不卸载**。
    let dir = tempfile::tempdir().unwrap();
    let first = session(dir.path(), "s-1", vec![Reply::text("在")]).await;
    let workspace = first.cwd.clone();
    let mut host = SessionHost::new(SessionId::new("s-1"));
    host.adopt(first.harness, first.cwd).unwrap();

    let (loaded, _) = load_session(dir.path(), "s-2", vec![Reply::text("在")]).await;
    host.adopt(loaded.harness, workspace).unwrap();
    assert_eq!(host.foreground().as_str(), "s-1", "入表本身不改前台");

    host.switch(&SessionId::new("s-2"));
    assert_eq!(host.foreground().as_str(), "s-2");
    assert_eq!(host.len(), 2, "旧的留在表里");
    assert!(host.contains(&SessionId::new("s-1")), "切走的那一场不卸载");
}

#[tokio::test]
async fn the_close_rules_refuse_only_the_foreground_and_the_busy() {
    use heng::session::close_check;
    use heng::session::host::CloseRefusal;

    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    let (first, _) = load_session(dir.path(), "s-1", vec![Reply::text("在")]).await;
    let mut host = SessionHost::new(SessionId::new("s-1"));
    host.adopt(first.harness, workspace.clone()).unwrap();

    // 前台自己：禁止 —— 它是脚下那块地板。
    assert_eq!(
        close_check(&host, &SessionId::new("s-1")),
        Err(CloseRefusal::Foreground(SessionId::new("s-1")))
    );

    // 空闲的非前台：单击即关（不二次确认）。
    let (second, _) = load_session(dir.path(), "s-2", vec![Reply::text("在")]).await;
    host.adopt(second.harness, workspace.clone()).unwrap();
    assert_eq!(close_check(&host, &SessionId::new("s-2")), Ok(()));

    // 只读预览：也能关。
    let (third, _) = load_session(dir.path(), "s-3", vec![Reply::text("在")]).await;
    host.adopt_read_only(third.harness, workspace.clone())
        .unwrap();
    assert_eq!(close_check(&host, &SessionId::new("s-3")), Ok(()));

    // 正在跑回合的非前台：拒绝。
    let opened = Arc::new(Notify::new());
    let (busy, _) = load_session(
        dir.path(),
        "s-4",
        vec![Reply::Stall(
            opened,
            vec![StreamEvent::TextDelta("半句话".to_owned())],
        )],
    )
    .await;
    host.adopt(busy.harness, workspace).unwrap();
    host.get(&SessionId::new("s-4"))
        .unwrap()
        .cmd
        .send(turn("跑起来"))
        .await
        .unwrap();
    let mut state = host.get(&SessionId::new("s-4")).unwrap().state.clone();
    wait_for_entry(&mut state, SessionState::RunningTurn).await;
    assert_eq!(
        close_check(&host, &SessionId::new("s-4")),
        Err(CloseRefusal::Running(SessionId::new("s-4")))
    );
}

#[tokio::test]
async fn closing_the_last_session_empties_the_host_and_a_new_one_comes_back() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    let (first, _) = load_session(dir.path(), "s-1", vec![Reply::text("在")]).await;
    let mut host = SessionHost::new(SessionId::new("s-1"));
    host.adopt(first.harness, workspace.clone()).unwrap();

    // 唯一一场永远是前台，所以规则上关不掉；直接卸下来就是「关掉最后一场」。
    host.close_with(&SessionId::new("s-1"), SessionCommand::Close)
        .await
        .unwrap();
    assert!(host.is_empty(), "关掉最后一场就是空宿主");

    // 空宿主里再新建一场回来（票 21）：加载一场空会话、入表、切前台。
    let (fresh, _) = load_session(dir.path(), "s-2", vec![Reply::text("在")]).await;
    host.adopt(fresh.harness, workspace).unwrap();
    host.switch(&SessionId::new("s-2"));
    assert_eq!(host.len(), 1, "新建之后表里又有一场");
    assert_eq!(host.foreground().as_str(), "s-2");
}

#[tokio::test]
async fn the_exit_gate_refuses_while_anything_is_unfinished() {
    use heng::session::{ExitGate, exit_gate};

    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    let (first, _) = load_session(dir.path(), "s-1", vec![Reply::text("在")]).await;
    let mut host = SessionHost::new(SessionId::new("s-1"));
    host.adopt(first.harness, workspace.clone()).unwrap();

    // 一场都不忙：可以退。
    assert_eq!(exit_gate(&host), ExitGate::Clear);

    // 后台那一场跑起来：闸门拦下，并指向它。
    let opened = Arc::new(Notify::new());
    let (busy, _) = load_session(
        dir.path(),
        "s-2",
        vec![Reply::Stall(
            opened,
            vec![StreamEvent::TextDelta("半句话".to_owned())],
        )],
    )
    .await;
    host.adopt(busy.harness, workspace).unwrap();
    host.get(&SessionId::new("s-2"))
        .unwrap()
        .cmd
        .send(turn("跑起来"))
        .await
        .unwrap();
    let mut state = host.get(&SessionId::new("s-2")).unwrap().state.clone();
    wait_for_entry(&mut state, SessionState::RunningTurn).await;

    assert_eq!(
        exit_gate(&host),
        ExitGate::Running {
            count: 1,
            first: SessionId::new("s-2"),
        },
        "拦下并指向在跑的那一场"
    );

    // 取消之后放行。
    host.get(&SessionId::new("s-2")).unwrap().cancel.cancel();
    wait_for_entry(&mut state, SessionState::Idle).await;
    assert_eq!(exit_gate(&host), ExitGate::Clear, "全部收尾之后能退");
}
