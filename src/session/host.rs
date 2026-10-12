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

/// 前台会话的**只读快照**：主循环解析一次提交要的那几样。
///
/// 它存在的理由：会话的 [`Harness`] 一旦交给宿主表的 task，主循环就不能再直接问它 —— 而
/// 「这一场叫什么、用哪个模型、有哪些技能、目标是什么」是解析每一次提交都要的。于是装好之后
/// 取一份快照，需要的时刻（`/model` 之后）再取一次。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontFacts {
    pub id: SessionId,
    pub model: String,
    /// 推理档位；`None` 是「默认」那一档。
    pub effort: Option<String>,
    pub skills: Vec<String>,
    /// 正在推进的目标名。
    pub goal: Option<String>,
}

/// 主循环能对一场会话做的事。
///
/// 决策票 04 §3 把它写成三个（`Submit(Submission)` / `Cancel` / `Close`）—— 那份形状的**意图**
/// 是「一次提交整条交给 task 办」，而这里按同一意图把它**展开**成主循环解析得出的具体动作：
/// 主循环仍独占前端事件接收端（那是单一消费者，参见票 15 的实现记录），于是它解析提交、发具体
/// 命令，而 task 侧只负责办。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionCommand {
    /// 跑一次动作：一句提示 / 一个裸技能 / 一次注入。
    Turn(SessionTurn),
    /// 把一条 `/` 命令的**原文**记进流（log-only，投影零产出）。
    ///
    /// 它落在这场会话自己的流上，所以必须由拥有 `Harness` 的那一方写。
    RecordCommand(String),
    /// 把一个技能正文装进上下文尾部（`/<skill> <task>` 的前半步）。
    LoadSkill(String),
    /// 撤销这场会话最近一次 `edit_file`。
    Undo,
    /// 换这一场用的模型。`effort` 是**当前**档位（换模型要重新校验它认不认这一档）。
    SwitchModel {
        model: String,
        effort: Option<String>,
    },
    /// 换思考档位。
    SwitchEffort(String),
    /// 取消这场会话当前在跑的动作。
    Cancel,
    /// 关闭并结束 task。
    Close,
}

/// 每场会话的那个**动作解释器**。
///
/// 宿主自己认识 `Cancel` 与 `Close`，其余命令都交给它办 —— 而 `Turn` 只是其中一种。之所以
/// 要这一层：会话级手势（换模型、装技能正文、撤销、记命令、跑一场讨论、目标循环）都要
/// `Config`、`ConsoleHandle`、`GoalSetup` 这些**进程级**的东西，而库层不认识它们。于是 task
/// 拿一个盒子，命令交给它办，而状态转换仍由宿主自己管（它持有 `watch` 的发送端）。
///
/// 默认的实现是 [`TurnActor`]：只跑回合。cli 那一侧会换上一个更全的。
#[async_trait::async_trait]
pub trait SessionActor<C = SessionCommand>: Send {
    /// 办一条命令。`Cancel` 与 `Close` 不会到达这里。
    ///
    /// `inbox` 是这一场自己的命令通道：跑长动作（一个回合、一场讨论、一趟目标循环）时它要一边
    /// 跑一边收**运行中的手势** —— 主循环把那边的 `FrontEndEvent` 转成命令转发过来（前端事件
    /// 接收端是单一消费者，只归它）。
    async fn act(
        &mut self,
        harness: &mut Harness,
        command: C,
        inbox: &mut mpsc::Receiver<C>,
    ) -> bool;
}

/// 默认的动作解释器：只会跑一次动作（一句提示 / 一个裸技能 / 一次注入）。
///
/// 它是单会话今天的全部行为，也是 [`SessionHost::adopt`] 的缺省。
pub struct TurnActor;

#[async_trait::async_trait]
impl SessionActor<SessionCommand> for TurnActor {
    async fn act(
        &mut self,
        harness: &mut Harness,
        command: SessionCommand,
        _inbox: &mut mpsc::Receiver<SessionCommand>,
    ) -> bool {
        match command {
            // 取消走既有的手势：信号是每场一份的，所以它只停这一场。
            SessionCommand::Cancel => harness.cancel_signal().cancel(),
            SessionCommand::Close => return false,
            SessionCommand::Turn(start) => {
                let outcome = match start {
                    SessionTurn::Prompt(text) => harness.run_turn(&text).await,
                    SessionTurn::Skill(name) => harness.run_skill(&name).await,
                    SessionTurn::Injected => harness.run_injected_turn().await,
                };
                if let Err(error) = outcome {
                    harness.notice(&format!("heng: {error}"));
                }
            }
            // 缺省解释器只会跑回合：其余会话级手势由 cli 那一侧的解释器办。
            _ => {}
        }
        true
    }
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
pub struct HostEntry<C = SessionCommand> {
    pub id: SessionId,
    /// 这一场的工作目录。
    pub cwd: PathBuf,
    /// 三态。接收端在这里 —— 发送端在拥有它的那个 task 手里。
    pub state: watch::Receiver<SessionState>,
    /// 主循环往这一场发命令的那条通道。命令的形状由调用方定 —— cli 那一侧装的是
    /// `Submit(Submission)` 与手势，缺省那一型只是「跑回合」。
    pub cmd: mpsc::Sender<C>,
    /// 只读预览（票 18）：流半读装起来的那一场 —— 凡写流的一律禁，三态不动。
    pub read_only: bool,
    /// 每场一份的取消手势（决策票 04 §3）。
    pub cancel: CancelSignal,
    /// 拥有这场会话的那个 task。
    pub join: JoinHandle<()>,
}

/// 宿主表。由主循环持有，位于 CLI 组装点之上。
pub struct SessionHost<C = SessionCommand> {
    entries: Vec<HostEntry<C>>,
    /// 全进程一个前台指针。
    foreground: watch::Sender<SessionId>,
    /// 保住一个接收端。
    ///
    /// **`watch::Sender::send` 在一个接收者都没有的时候是失败的**，而前台指针必须在没人订阅的
    /// 时候也改得动（单会话、以及还没有任何 task 去 `follow` 的那些时刻）。少了这一个，`switch`
    /// 会静默地什么都不做。
    _foreground_watch: watch::Receiver<SessionId>,
}

impl SessionHost<SessionCommand> {
    /// 把一场组装好的会话交给它自己的 task，并**入表**。
    ///
    /// `cwd` 由调用方给：表里记的是这一场的工作目录，而它不必等于进程的 cwd（票 18 的装载路径
    /// 会让两者分开）。
    pub fn adopt(&mut self, harness: Harness, cwd: PathBuf) -> Result<(), HostError> {
        self.adopt_with(harness, cwd, false, Box::new(TurnActor))
    }

    /// 同上，但这是一场**只读预览**（票 18 的第二档损坏流）。
    ///
    /// 只读不是一个第四态：三态照旧，表里只多一个标志 —— 凡写流的一律禁（提交、所有 `/`
    /// 命令、`Shift+Tab`、页内改名、新建），各回一句。
    pub fn adopt_read_only(&mut self, harness: Harness, cwd: PathBuf) -> Result<(), HostError> {
        self.adopt_with(harness, cwd, true, Box::new(TurnActor))
    }
}

impl<C: Send + 'static> SessionHost<C> {
    /// 建一张以 `foreground` 为前台的宿主表。表起初是空的 —— 第一场会话由 `adopt`（缺省那一
    /// 型）或 [`adopt_with`](Self::adopt_with) 入表。
    pub fn new(foreground: SessionId) -> Self {
        let (foreground, keep) = watch::channel(foreground);
        Self {
            entries: Vec::new(),
            foreground,
            _foreground_watch: keep,
        }
    }

    /// 同上，但换一个动作解释器、也换命令的类型 —— cli 那一侧走这条（它的命令里装着整条
    /// `Submission`，会话级手势都在那边）。
    pub fn adopt_with(
        &mut self,
        harness: Harness,
        cwd: PathBuf,
        read_only: bool,
        mut actor: Box<dyn SessionActor<C>>,
    ) -> Result<(), HostError> {
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
                // 状态由宿主自己管：解释器只负责把这条命令办完。
                let _ = state_tx.send(SessionState::RunningTurn);
                let keep_going = actor.act(&mut harness, command, &mut cmd_rx).await;
                // 一场「等在一个人答」的动作（票 22）会在中间把状态置成 `AwaitingHuman`；
                // 回到 `Idle` 是它收尾之后的事。
                let _ = state_tx.send(SessionState::Idle);
                if !keep_going {
                    break;
                }
            }
        });
        self.entries.push(HostEntry {
            id,
            cwd,
            read_only,
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

    pub fn get(&self, id: &SessionId) -> Option<&HostEntry<C>> {
        self.entries.iter().find(|entry| &entry.id == id)
    }

    pub fn entries(&self) -> &[HostEntry<C>] {
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

    /// 把一场会话从表里卸下来：发一条**结束命令**、等它那个 task 结束。
    ///
    /// 宿主不认识命令的语义，所以「结束」得由调用方给 —— cli 那一型的结束命令就是
    /// `SessionCommand::Close`（见 [`SessionHost::close`] 那一层）。
    ///
    /// **不写盘、不删盘** —— 盘上那份还在，随时能再装载（票 18）。
    pub async fn close_with(&mut self, id: &SessionId, shutdown: C) -> Result<(), HostError> {
        let index = self
            .entries
            .iter()
            .position(|entry| &entry.id == id)
            .ok_or_else(|| HostError::NotHosted(id.clone()))?;
        let entry = self.entries.remove(index);
        let _ = entry.cmd.send(shutdown).await;
        let _ = entry.join.await;
        Ok(())
    }
}

impl SessionHost<SessionCommand> {
    /// 关掉一场缺省命令类型的会话：发 `Close`、等它那个 task 结束。
    ///
    /// 前台自己不许关、正在跑回合的那一场怎么处理是票 21 的规则，这一层只保证「表里真的少了
    /// 一行、task 真的结束了」。
    pub async fn close(&mut self, id: &SessionId) -> Result<(), HostError> {
        self.close_with(id, SessionCommand::Close).await
    }
}
