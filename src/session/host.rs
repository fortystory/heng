//! 宿主表与前台指针：同一个进程里同时活着几场会话
//! （`.scratch/session-tab/spec.md` §1、决策票 04 §1–§3）。
//!
//! 三条形状钉在这里：
//!
//! * **一场会话 = 一个 [`Harness`] + 一个拥有它的 tokio task**。`Harness` 就是宿主，不造新词。
//! * **主循环能做的事只有三个**：跑一次动作（[`SessionCommand::Turn`]）、取消、关闭。命令走
//!   每场一条 `mpsc`，状态走每场一个 `watch`（保最新不丢 —— 渲染通道是 `broadcast`，满了会
//!   静默丢弃，而宿主表的快照不能被丢）。
//! * **前台指针 = 全进程一个 `watch<SessionId>`**：切换只是改它，**忙闲不阻挡**。
//!
//! **不变量**：同一个 [`SessionId`] 在表里只能有一行。否则同一路径两份 `EventLog` 会各自维护
//! `next_seq`、写出重复 `seq` 且互相不可见。

use std::path::PathBuf;

use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::Harness;
use crate::agent::CancelSignal;
use crate::events::SessionId;

/// 一场会话的三态（决策票 04 §3）。
///
/// `AwaitingHuman` 由 asker 那一层置上（票 22）；这一层先把它摆在这里，因为标记列与页顶读数
/// 都读它。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SessionState {
    /// 空闲，等人。
    #[default]
    Idle,
    /// 正在跑一个回合。
    RunningTurn,
    /// 停在等人答的那一步。
    AwaitingHuman,
}

/// 交给一场会话的一次动作。
///
/// 它比 CLI 的 `TurnStart` 少一层借用：命令要跨 task 边界，所以文本是 `String`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionTurn {
    /// 一句用户提示。
    Prompt(String),
    /// 一个裸技能。
    Skill(String),
    /// 「本轮由别处注入」的驱动（目标循环那条入口）。
    Injected,
}

/// 主循环能对一场会话做的事 —— **只有三个**（决策票 04 §3）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionCommand {
    /// 跑一次动作。
    Turn(SessionTurn),
    /// 取消这场会话当前在跑的动作。
    Cancel,
    /// 关闭并结束 task。
    Close,
}

/// 宿主表拒绝一次操作的理由。
#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("会话 {0} 已经在宿主表里了")]
    AlreadyHosted(SessionId),
    #[error("会话 {0} 不在宿主表里")]
    NotHosted(SessionId),
}

/// 宿主表的一行（决策票 04 §1）。
pub struct HostEntry {
    pub id: SessionId,
    /// 这一场的工作目录。
    pub cwd: PathBuf,
    /// 三态。接收端在这里 —— 发送端在拥有它的那个 task 手里。
    pub state: watch::Receiver<SessionState>,
    /// 主循环往这一场发命令的那条通道。
    pub cmd: mpsc::Sender<SessionCommand>,
    /// 每场一份的取消手势（决策票 04 §3）。
    pub cancel: CancelSignal,
    /// 拥有这场会话的那个 task。
    pub join: JoinHandle<()>,
}

/// 宿主表。由主循环持有，位于 CLI 组装点之上。
pub struct SessionHost {
    entries: Vec<HostEntry>,
    /// 全进程一个前台指针。
    foreground: watch::Sender<SessionId>,
}

impl SessionHost {
    /// 建一张以 `foreground` 为前台的宿主表。表起初是空的 —— 第一场会话由 [`adopt`](Self::adopt)
    /// 入表。
    pub fn new(foreground: SessionId) -> Self {
        let (foreground, _receiver) = watch::channel(foreground);
        Self {
            entries: Vec::new(),
            foreground,
        }
    }

    /// 把一场组装好的会话交给它自己的 task，并**入表**。
    ///
    /// `cwd` 由调用方给：表里记的是这一场的工作目录，而它不必等于进程的 cwd（票 18 的装载路径
    /// 会让两者分开）。
    pub fn adopt(&mut self, harness: Harness, cwd: PathBuf) -> Result<(), HostError> {
        let id = harness.session_id().clone();
        if self.contains(&id) {
            return Err(HostError::AlreadyHosted(id));
        }
        let cancel = harness.cancel_signal();
        let (state_tx, state_rx) = watch::channel(SessionState::Idle);
        let (cmd_tx, mut cmd_rx) = mpsc::channel(8);
        let join = tokio::spawn(async move {
            let mut harness = harness;
            while let Some(command) = cmd_rx.recv().await {
                match command {
                    SessionCommand::Turn(start) => {
                        let _ = state_tx.send(SessionState::RunningTurn);
                        let outcome = match start {
                            SessionTurn::Prompt(text) => harness.run_turn(&text).await,
                            SessionTurn::Skill(name) => harness.run_skill(&name).await,
                            SessionTurn::Injected => harness.run_injected_turn().await,
                        };
                        if let Err(error) = outcome {
                            harness.notice(&format!("heng: {error}"));
                        }
                        // 一场「等在一个人答」的动作（票 22）会在中间把状态置成
                        // `AwaitingHuman`；回到 `Idle` 是它收尾之后的事。
                        let _ = state_tx.send(SessionState::Idle);
                    }
                    // 取消走既有的手势：信号是每场一份的，所以它只停这一场。
                    SessionCommand::Cancel => harness.cancel_signal().cancel(),
                    SessionCommand::Close => break,
                }
            }
        });
        self.entries.push(HostEntry {
            id,
            cwd,
            state: state_rx,
            cmd: cmd_tx,
            cancel,
            join,
        });
        Ok(())
    }

    /// 现在的前台是哪一场。
    pub fn foreground(&self) -> SessionId {
        self.foreground.borrow().clone()
    }

    /// 前台指针的接收端。每个 task 持一份，于是「切到前台即唤醒」不用额外的广播。
    pub fn follow_foreground(&self) -> watch::Receiver<SessionId> {
        self.foreground.subscribe()
    }

    /// 切前台。**忙闲不阻挡** —— 目标正在跑回合也照样切（决策票 04 §2）。
    ///
    /// 它只改指针：已在表里的那一场因此是零延迟的，「不在表里就先装载」是票 18 的事。
    pub fn switch(&self, id: &SessionId) {
        let _ = self.foreground.send(id.clone());
    }

    pub fn contains(&self, id: &SessionId) -> bool {
        self.entries.iter().any(|entry| &entry.id == id)
    }

    pub fn get(&self, id: &SessionId) -> Option<&HostEntry> {
        self.entries.iter().find(|entry| &entry.id == id)
    }

    pub fn entries(&self) -> &[HostEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 有几场正在跑回合 —— 页顶读数 `M 场在跑` 数的就是它。
    pub fn running(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| *entry.state.borrow() == SessionState::RunningTurn)
            .count()
    }

    /// 把一场会话从表里卸下来：发 `Close`、等它那个 task 结束。
    ///
    /// **不写盘、不删盘** —— 盘上那份还在，随时能再装载（票 18）。前台自己不许关、正在跑回合
    /// 的那一场怎么处理是票 21 的规则，这一层只保证「表里真的少了一行、task 真的结束了」。
    pub async fn close(&mut self, id: &SessionId) -> Result<(), HostError> {
        let index = self
            .entries
            .iter()
            .position(|entry| &entry.id == id)
            .ok_or_else(|| HostError::NotHosted(id.clone()))?;
        let entry = self.entries.remove(index);
        let _ = entry.cmd.send(SessionCommand::Close).await;
        let _ = entry.join.await;
        Ok(())
    }
}
