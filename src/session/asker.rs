//! 按会话包装的应答者（`.scratch/session-tab/spec.md` §1、§3、票 22）。
//!
//! 一场后台会话跑出「要人答」的东西时，问题**不该**弹到另一场的脸上；而它也不该被自动答掉。
//! 这一层做的就是这两件事：不是前台就**不往前端送请求**，把 future 挂在 `foreground` 的
//! `watch` 上、把这一场置成 `AwaitingHuman`；等它成为前台，再把请求**原样**送出去。
//!
//! 「原样」是这句话的要点：请求**从没被消费过**，所以前端不需要记 N 个悬浮请求 —— 谁切到前台
//! 谁弹自己那一个。回答仍然走既有的 reply 通道，宿主表不代答。

use std::sync::{Arc, Mutex};

use tokio::sync::watch;

use super::host::SessionState;
use crate::events::SessionId;
use crate::permissions::{Answer, Asker, PermissionRequest};

/// 一个按会话包装的应答者。
pub struct SessionAsker {
    /// 真正往下送的应答者（TUI 或 plain 那一个）。
    inner: Arc<dyn Asker>,
    /// 这一场是谁。
    session: SessionId,
    /// 全进程一个前台指针。
    foreground: watch::Receiver<SessionId>,
    /// 这一场的三态发送端。
    ///
    /// 它是一个**槽**而不是一个值：应答者在组装期就建好了，而三态的那对通道由宿主表在
    /// `adopt` 里创建 —— 所以调用方装完之后用 [`attach`](Self::attach) 填进来。
    state: Mutex<Option<watch::Sender<SessionState>>>,
}

impl SessionAsker {
    /// 包一个。`foreground` 由 `SessionHost::subscribe_foreground` 给。
    pub fn new(
        inner: Arc<dyn Asker>,
        session: SessionId,
        foreground: watch::Receiver<SessionId>,
    ) -> Self {
        Self {
            inner,
            session,
            foreground,
            state: Mutex::new(None),
        }
    }

    /// 这一场入表之后，把它的三态发送端填进来。
    pub fn attach(&self, state: watch::Sender<SessionState>) {
        *self.state.lock().expect("三态槽") = Some(state);
    }

    fn set_state(&self, next: SessionState) {
        if let Some(state) = self.state.lock().expect("三态槽").as_ref() {
            state.send_replace(next);
        }
    }
}

#[async_trait::async_trait]
impl Asker for SessionAsker {
    async fn ask(&self, request: &PermissionRequest) -> Answer {
        let mut foreground = self.foreground.clone();
        if *foreground.borrow() != self.session {
            // 不是前台：**不送**。挂在 watch 上等着，同时让列表上那一行说出「它在等人」。
            self.set_state(SessionState::AwaitingHuman);
            // 取消（观众走了）也算一种唤醒 —— 那时 `inner` 会照它自己的纪律答。
            let _ = foreground.wait_for(|id| id == &self.session).await;
            // 回到跑着的状态：前台那一场正在等这个答案。
            self.set_state(SessionState::RunningTurn);
        }
        self.inner.ask(request).await
    }
}
