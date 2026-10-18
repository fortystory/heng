//! 宿主表的事实投影（票 03）。
//!
//! 「宿主表现在长什么样」只有这一处算：[`HostTracker::update`] 吃一份 [`HostSample`]
//! （前台 + 每会话三态 / 只读），吐出 [`HostFacts`] —— cli 与渲染器都只读它，不再各自
//! 拼一份。80ms 那一次轮询采的也正是这个样本。
//!
//! 三条规则钉在这里（它们此前只能靠 [`docs/tui-manual-checklist.md`](../../docs/tui-manual-checklist.md)
//! ㊶ 真机走查）：
//!
//! 1. **未读的寿命**：一个会话「跑完」只留一条未读，直到用户下一次输入
//!    （[`HostTracker::attention_seen`]）或它又开跑；关闭的会话把它那条一起带走。
//! 2. **`N` 只算后台**：`waiting`（待答）与 `finished`（跑完未读）都不数前台 ——
//!    前台自己的状态由状态行 / 出口提示那头说。
//! 3. **单会话标题不变**：由 2 直接推出；单会话时两个计数恒为 0，标题状态段不出现。
//!
//! [`HostSample`] 不依赖具体的命令类型：`from_host` 是泛型的，测试可以直接造样本。

use std::collections::{HashMap, HashSet};

use crate::events::SessionId;

use super::host::{SessionHost, SessionState};

/// 一个已装载会话的事实：三态与只读。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedSession {
    pub id: SessionId,
    pub state: SessionState,
    /// 只读预览（票 18）：渲染器自己发起的那几处写动作要挡它。
    pub read_only: bool,
}

/// 一次采样的原料：前台 + 每会话的三态与只读。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostSample {
    pub foreground: SessionId,
    /// 按宿主表的装载顺序。
    pub sessions: Vec<HostedSession>,
}

impl HostSample {
    /// 从宿主表采一次样：只读三态与只读标志，不搬任务或通道。
    pub fn from_host<C: Send + 'static>(host: &SessionHost<C>) -> Self {
        Self {
            foreground: host.foreground(),
            sessions: host
                .entries()
                .iter()
                .map(|entry| HostedSession {
                    id: entry.id.clone(),
                    state: *entry.state.borrow(),
                    read_only: entry.read_only,
                })
                .collect(),
        }
    }
}

/// 宿主表此刻长什么样 —— 前端只从这一个值读宿主事实。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostFacts {
    pub foreground: SessionId,
    /// 按宿主表的装载顺序；顺序是会话页与顶部页签条画的行序。
    pub sessions: Vec<HostedSession>,
    /// 后台有几个在等人答（不含前台）。
    pub waiting: usize,
    /// 后台有几个刚跑完、还没被读（不含前台）。
    pub finished: usize,
}

impl HostFacts {
    /// 宿主表空着吗（票 21 的空宿主态）。
    pub fn host_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// 装载了几个。
    pub fn sessions_count(&self) -> usize {
        self.sessions.len()
    }

    /// 有几个正在跑回合（页顶读数 `M 个在跑`）。
    pub fn running_count(&self) -> usize {
        self.sessions
            .iter()
            .filter(|session| session.state == SessionState::RunningTurn)
            .count()
    }

    /// 一份只含前台会话的快照（测试与最小构造用）：`foreground` 自己就是唯一会话。
    pub fn foreground_only(foreground: SessionId, state: SessionState) -> Self {
        Self {
            sessions: vec![HostedSession {
                id: foreground.clone(),
                state,
                read_only: false,
            }],
            foreground,
            waiting: 0,
            finished: 0,
        }
    }

    /// 同 [`foreground_only`](Self::foreground_only)，但只设只读标志、三态是空闲。
    pub fn foreground_read_only(foreground: SessionId, read_only: bool) -> Self {
        Self {
            sessions: vec![HostedSession {
                id: foreground.clone(),
                state: SessionState::Idle,
                read_only,
            }],
            foreground,
            waiting: 0,
            finished: 0,
        }
    }

    /// 表里装着哪几个（会话页那三档标题色靠它）。
    pub fn hosted(&self) -> Vec<SessionId> {
        self.sessions
            .iter()
            .map(|session| session.id.clone())
            .collect()
    }

    /// 某个会话此刻的事实；不在表里时是 `None`。
    pub fn session(&self, id: &SessionId) -> Option<&HostedSession> {
        self.sessions.iter().find(|session| &session.id == id)
    }
}

/// 上一帧三态与未读集合的记账 —— [`HostFacts`] 的边沿都由它推出来。
#[derive(Debug, Default)]
pub struct HostTracker {
    states: HashMap<SessionId, SessionState>,
    unread: HashSet<SessionId>,
}

impl HostTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// 用一次采样推进一帧，返回这一帧的宿主事实。
    pub fn update(&mut self, sample: &HostSample) -> HostFacts {
        let mut present = HashSet::with_capacity(sample.sessions.len());
        for session in &sample.sessions {
            present.insert(session.id.clone());
            let before = self.states.insert(session.id.clone(), session.state);
            match (before, session.state) {
                // 刚跑完：记一条未读。
                (Some(SessionState::RunningTurn), SessionState::Idle) => {
                    self.unread.insert(session.id.clone());
                }
                // 又开跑了：这一条未读作废。
                (_, SessionState::RunningTurn) => {
                    self.unread.remove(&session.id);
                }
                _ => {}
            }
        }
        // 关掉的会话：上一帧的三态与它那条未读一起走。
        self.states.retain(|id, _| present.contains(id));
        self.unread.retain(|id| present.contains(id));
        // 规则 2：两个计数都只数**后台**；规则 3 由它推出（单会话时恒为 0）。
        let waiting = sample
            .sessions
            .iter()
            .filter(|session| {
                session.id != sample.foreground && session.state == SessionState::AwaitingHuman
            })
            .count();
        let finished = self
            .unread
            .iter()
            .filter(|id| **id != sample.foreground)
            .count();
        HostFacts {
            foreground: sample.foreground.clone(),
            sessions: sample.sessions.clone(),
            waiting,
            finished,
        }
    }

    /// 用户有过一次输入：所有「跑完」的未读到此为止。
    pub fn attention_seen(&mut self) {
        self.unread.clear();
    }

    /// 现在记着哪几条未读（测试与调试用）。
    pub fn unread(&self) -> &HashSet<SessionId> {
        &self.unread
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(foreground: &str, sessions: &[(&str, SessionState)]) -> HostSample {
        HostSample {
            foreground: SessionId::new(foreground),
            sessions: sessions
                .iter()
                .map(|(id, state)| HostedSession {
                    id: SessionId::new(*id),
                    state: *state,
                    read_only: false,
                })
                .collect(),
        }
    }

    /// 规则 1：未读从「跑完」那一刻起，活到用户下一次输入。
    #[test]
    fn unread_lives_until_the_next_input() {
        let mut tracker = HostTracker::new();
        let running = sample(
            "a",
            &[("a", SessionState::Idle), ("b", SessionState::RunningTurn)],
        );
        let idle = sample("a", &[("a", SessionState::Idle), ("b", SessionState::Idle)]);

        // 先登记，再走 Running → Idle 的边沿。
        assert_eq!(tracker.update(&running).finished, 0, "跑着的时候还没有未读");
        assert_eq!(tracker.update(&idle).finished, 1, "跑完记一条");
        assert_eq!(tracker.update(&idle).finished, 1, "同一条未读不重复记");
        assert_eq!(tracker.unread().len(), 1);

        // 用户下一次输入把它清掉。
        tracker.attention_seen();
        assert_eq!(tracker.update(&idle).finished, 0, "输入之后未读到此为止");

        // 再跑一轮又记一条；开跑那一刻先把旧的作废。
        tracker.update(&idle);
        tracker.update(&running);
        assert_eq!(tracker.update(&idle).finished, 1, "第二轮重新记一条");
        tracker.update(&running);
        assert_eq!(tracker.unread().len(), 0, "又开跑了，未读作废");
    }

    /// 规则 2：`N` 只算后台 —— 前台待答不计数。
    #[test]
    fn only_background_sessions_count() {
        let mut tracker = HostTracker::new();
        let facts = tracker.update(&sample(
            "a",
            &[
                ("a", SessionState::AwaitingHuman),
                ("b", SessionState::AwaitingHuman),
                ("c", SessionState::RunningTurn),
            ],
        ));
        assert_eq!(facts.waiting, 1, "只数后台那个待答的");
        assert_eq!(facts.running_count(), 1);
    }

    /// 规则 3：单会话标题不变 —— 唯一的那个会话即使待答 / 跑完，两个计数也恒为 0。
    #[test]
    fn a_single_session_never_moves_the_title() {
        let mut tracker = HostTracker::new();
        for state in [
            SessionState::Idle,
            SessionState::RunningTurn,
            SessionState::AwaitingHuman,
            SessionState::Idle,
            SessionState::AwaitingHuman,
        ] {
            let facts = tracker.update(&sample("a", &[("a", state)]));
            assert_eq!(facts.waiting, 0, "单会话待答不数（{state:?}）");
            assert_eq!(facts.finished, 0, "单会话跑完不数（{state:?}）");
        }
    }

    /// 关掉的会话把它的未读一起带走，不让它永久占着标题。
    #[test]
    fn a_closed_session_takes_its_unread_with_it() {
        let mut tracker = HostTracker::new();
        tracker.update(&sample(
            "a",
            &[("a", SessionState::Idle), ("b", SessionState::RunningTurn)],
        ));
        tracker.update(&sample(
            "a",
            &[("a", SessionState::Idle), ("b", SessionState::Idle)],
        ));
        assert_eq!(tracker.unread().len(), 1);
        let facts = tracker.update(&sample("a", &[("a", SessionState::Idle)]));
        assert_eq!(facts.finished, 0, "关掉之后未读不再算");
        assert_eq!(tracker.unread().len(), 0);
    }
}
