//! 宿主表与前台指针：一场会话 = 一个 `Harness` + 一个拥有它的 task
//! （`.scratch/session-tab/spec.md` §1、票 15 的宿主接缝）。
//!
//! 这一层用**真的** `Harness` 驱动（假 provider 照脚本应答），因为要证的正是「表里那一行
//! 真的拥有那一场会话」—— task 收命令、跑回合、把状态写回 `watch`。

mod support;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use heng::events::{EventPayload, SessionId, SpeakerId, read_events};
use heng::permissions::{Mode, Policy};
use heng::provider::StreamEvent;
use heng::render::{RenderSinks, Renderer};
use heng::session::host::{SessionCommand, SessionHost, SessionState, SessionTurn};
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
