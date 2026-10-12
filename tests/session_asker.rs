//! 后台会话的询问**挂住**、切到前台即唤醒（`.scratch/session-tab/spec.md` §1、票 22）。
//!
//! 这一层要钉住的是两句话：**后台那一场的询问不弹到别人脸上**，以及**它永远不会被自动答掉**
//! —— 请求从没被消费过，所以切过去时原样弹出来。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use heng::events::SessionId;
use heng::permissions::{Answer, Asker, PermissionRequest};
use heng::session::SessionAsker;
use heng::session::host::SessionState;
use serde_json::json;
use tokio::sync::watch;

/// 真正往下送的作答者：记下收到过哪些问题，然后照答。
#[derive(Default)]
struct FakeAsker {
    asked: Mutex<Vec<String>>,
}

impl FakeAsker {
    fn asked(&self) -> Vec<String> {
        self.asked.lock().expect("账").clone()
    }
}

#[async_trait::async_trait]
impl Asker for FakeAsker {
    async fn ask(&self, request: &PermissionRequest) -> Answer {
        self.asked
            .lock()
            .expect("账")
            .push(request.tool_name.clone());
        Answer::Allow
    }
}

fn request(tool: &str) -> PermissionRequest {
    PermissionRequest {
        request_id: "p-1".to_owned(),
        tool_call_id: "c-1".to_owned(),
        tool_name: tool.to_owned(),
        args: json!({}),
        reason: "测试".to_owned(),
        speaker: None,
        escalation: None,
    }
}

#[tokio::test]
async fn a_background_session_holds_its_question_until_it_is_the_foreground() {
    let (foreground, foreground_rx) = watch::channel(SessionId::new("s-1"));
    let inner = Arc::new(FakeAsker::default());
    let asker = Arc::new(SessionAsker::new(
        Arc::clone(&inner) as Arc<dyn Asker>,
        SessionId::new("s-2"),
        foreground_rx,
    ));
    let (state_tx, state_rx) = watch::channel(SessionState::Idle);
    asker.attach(state_tx);

    // 后台那一场发问：它挂在 watch 上，问题**不往前端送**。
    let asking = tokio::spawn({
        let asker = Arc::clone(&asker);
        async move { asker.ask(&request("bash")).await }
    });
    let mut state = state_rx.clone();
    tokio::time::timeout(
        Duration::from_secs(2),
        state.wait_for(|state| *state == SessionState::AwaitingHuman),
    )
    .await
    .expect("状态该变成待答")
    .expect("watch 还活着");
    assert!(inner.asked().is_empty(), "后台的询问不该弹到别的会话脸上");

    // 切到前台：请求原样送出去，答案回来，状态回到「在跑」。
    foreground.send_replace(SessionId::new("s-2"));
    let answer = tokio::time::timeout(Duration::from_secs(2), asking)
        .await
        .expect("切到前台该唤醒它")
        .expect("任务不崩");
    assert_eq!(answer, Answer::Allow);
    assert_eq!(inner.asked(), vec!["bash".to_owned()], "问题原样送到前端");
    tokio::time::timeout(
        Duration::from_secs(2),
        state.wait_for(|state| *state == SessionState::RunningTurn),
    )
    .await
    .expect("回答之后回到在跑")
    .expect("watch 还活着");
}

#[tokio::test]
async fn the_foreground_session_goes_straight_through() {
    // 前台那一场不必等：它直接送进既有的请求通道，状态也不经过待答那一档。
    let (_foreground, foreground_rx) = watch::channel(SessionId::new("s-1"));
    let inner = Arc::new(FakeAsker::default());
    let asker = SessionAsker::new(
        Arc::clone(&inner) as Arc<dyn Asker>,
        SessionId::new("s-1"),
        foreground_rx,
    );
    let (state_tx, state_rx) = watch::channel(SessionState::Idle);
    asker.attach(state_tx);

    let answer = asker.ask(&request("read")).await;
    assert_eq!(answer, Answer::Allow);
    assert_eq!(inner.asked(), vec!["read".to_owned()]);
    assert_eq!(
        *state_rx.borrow(),
        SessionState::Idle,
        "前台不问「待答」那一路：状态没被动过"
    );
}
