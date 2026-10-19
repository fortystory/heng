//! 流程页的模型（`.scratch/flow-page/spec.md` §2–§3）：agent 一次回合的相位机，以及它画成
//! 什么行。
//!
//! 它是**纯渲染器状态**：吃的是渲染通道上已经到手的事件，不新增事件、不碰 `agent` 与事件
//! schema。相位机只认**主发言者**的回合事件 —— 执行者与讨论者的同名事件会让节点乱跳，所以
//! 一律忽略。
//!
//! 每会话一份（住在 [`crate::render::tui`] 的 `View` 里）：切会话、历史重播都靠它自然
//! 重建，不另存状态。

use chrono::{DateTime, Utc};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::events::{Decision, Event, EventPayload, Role, SpeakerId, StopReason};
use crate::render::{DeltaKind, RenderEvent, palette, pane, width, wording};

/// 轨道上的五个节点，按流程顺序。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Node {
    /// 用户刚提交、`TurnStarted` 还没落流。
    Received,
    /// `TurnStarted` 之后、第一个增量之前：拼请求、估预算、等首 token。
    Waiting,
    /// 第一个增量之后、`MessageCompleted` 之前。
    Streaming,
    /// `ToolCallStarted` 之后、配对的完成之前（含权限门）。
    Tool,
    /// `TurnEnded` 那一下。
    Closing,
}

impl Node {
    /// 五个节点，画轨道的顺序。
    pub const ALL: [Node; 5] = [
        Node::Received,
        Node::Waiting,
        Node::Streaming,
        Node::Tool,
        Node::Closing,
    ];

    /// 它在轨道上的下标。
    pub fn index(self) -> usize {
        match self {
            Node::Received => 0,
            Node::Waiting => 1,
            Node::Streaming => 2,
            Node::Tool => 3,
            Node::Closing => 4,
        }
    }

    /// 节点名（措辞层）。
    pub fn label(self) -> &'static str {
        match self {
            Node::Received => wording::FLOW_NODE_RECEIVED,
            Node::Waiting => wording::FLOW_NODE_WAITING,
            Node::Streaming => wording::FLOW_NODE_STREAMING,
            Node::Tool => wording::FLOW_NODE_TOOL,
            Node::Closing => wording::FLOW_NODE_CLOSING,
        }
    }

    fn phase(self) -> Phase {
        match self {
            Node::Received => Phase::Received,
            Node::Waiting => Phase::Waiting,
            Node::Streaming => Phase::Streaming,
            Node::Tool => Phase::Tool,
            Node::Closing => Phase::Closing,
        }
    }
}

/// 一次回合的相位（`.scratch/flow-page/spec.md` §2）。
///
/// 与 [`Node`] 一一对应，`Idle` 是唯一没有节点的那个：空闲时轨道定格上一回合。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    /// 没有回合在跑。
    #[default]
    Idle,
    Received,
    Waiting,
    Streaming,
    Tool,
    Closing,
}

impl Phase {
    fn node(self) -> Option<Node> {
        match self {
            Phase::Idle => None,
            Phase::Received => Some(Node::Received),
            Phase::Waiting => Some(Node::Waiting),
            Phase::Streaming => Some(Node::Streaming),
            Phase::Tool => Some(Node::Tool),
            Phase::Closing => Some(Node::Closing),
        }
    }
}

/// 一个节点经过的时刻：进去一次、出来一次。两个都是事件信封上的 `at`，所以重播也正确。
#[derive(Debug, Clone, Copy, Default)]
struct Stamp {
    entered: Option<DateTime<Utc>>,
    finished: Option<DateTime<Utc>>,
}

/// 权限门在流程页上的三态：门直接放行、允许、拒绝。「正在问」由外部的
/// `SessionState::AwaitingHuman` 决定，不塞进这个值（同一个 `Spec.awaiting` 管四档文案）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Gate {
    /// 门直接放行：`Allow` 的路径，人没被问过。
    #[default]
    Passed,
    Allowed,
    Denied,
}

/// 一次回合的账：轨道的时间、工具与裁决、收尾原因。空闲时 [`FlowPanel::last`] 定格着上一份。
#[derive(Debug, Clone, Default)]
struct TurnRecord {
    stamps: [Stamp; 5],
    /// 当前回合的第几次迭代（`TurnStarted.iteration`）。
    iteration: u32,
    /// 这一次调用的首个增量距请求发出多少毫秒（`MessageCompleted.first_token_ms`，ADR 0020）。
    ///
    /// 等模型段与生成段的精确分界靠它 —— 重播没有 `Delta`，只能从这里推。
    first_token_ms: Option<u64>,
    /// 本回合跑过几次工具调用。
    tools: u32,
    /// 最近一次工具调用的名字。
    tool: Option<String>,
    /// 最近一次工具调用自己的运行时长（`ToolCallCompleted.duration_ms`）。
    tool_ms: Option<u64>,
    /// 最近一次权限门的状态（`.scratch/flow-page/spec.md` §2）。
    gate: Gate,
    /// 本回合怎么收的场。
    reason: Option<StopReason>,
    /// 实时收到的字符数（增量累加；完成时以 `MessageCompleted.text` 为准）。
    streamed: usize,
}

/// 画一帧需要的外部事实：回合号、上下文占比、等不等人、宽度与「现在」。
///
/// `now` 由调用方传入，于是相位与耗时都能在测试里钉死。
pub struct FrameInput<'a> {
    /// 已经收尾的回合数（`View::turn_ordinal` 的口径）。
    pub turn: u32,
    /// 上下文占比，已格式化（状态行那一份 [`wording::context_share_value`]）。
    pub share: &'a str,
    /// 此刻循环是不是停在等人答的那一步（`SessionState::AwaitingHuman`）。
    pub awaiting: bool,
    /// 本回合累计用量（输入、输出）；`None` = 还没有 `UsageRecorded`。宽档统计用它。
    pub usage: Option<(u64, u64)>,
    /// token 数的书写制式（`[ui] number_style`）。
    pub number_style: wording::NumberStyle,
    pub width: u16,
    pub height: u16,
    pub now: DateTime<Utc>,
}

/// 每会话一份的流程页状态。
#[derive(Debug, Default)]
pub struct FlowPanel {
    /// 主发言者：这份面板见到的第一个 `Debater` 的回合事件认领它，此后只认它。
    ///
    /// 执行者（`Executor`）与讨论的第二位发言者不认识，于是它们的回合事件进不来。
    main: Option<SpeakerId>,
    phase: Phase,
    current: TurnRecord,
    /// 上一回合的定格（空闲时画它）。
    last: Option<TurnRecord>,
}

impl FlowPanel {
    /// 吃一条渲染事件。`at` 是 [`RenderEvent::Delta`] 那一刻（增量不进流，信封上没有
    /// 时刻）；`Logged` 用事件自己的 `at`。
    pub fn observe(&mut self, event: &RenderEvent, at: DateTime<Utc>) {
        match event {
            RenderEvent::Logged { event, .. } => self.apply(event),
            RenderEvent::Delta {
                speaker,
                kind,
                text,
                ..
            } => {
                // 执行者与讨论第二人的增量不许推动主会话的相位（同一条「只认主发言者」）。
                if self.main.as_ref().is_none_or(|main| main != speaker) {
                    return;
                }
                if self.phase == Phase::Waiting {
                    self.enter(Node::Streaming, at);
                }
                if self.phase == Phase::Streaming && *kind == DeltaKind::Text {
                    self.current.streamed += text.chars().count();
                }
            }
            _ => {}
        }
    }

    /// 把一条进流事件并进相位机。
    fn apply(&mut self, event: &Event) {
        // 只认主发言者的回合事件；执行者与讨论第二人的同名事件一律挡在门外。
        // `MessageCompleted` 与增量另有自己的判据（下面逐条写明）。
        let gated = matches!(
            &event.payload,
            EventPayload::TurnStarted { .. }
                | EventPayload::ToolCallStarted { .. }
                | EventPayload::ToolCallCompleted { .. }
                | EventPayload::PermissionAsked { .. }
                | EventPayload::PermissionDecided { .. }
                | EventPayload::TurnEnded { .. }
        );
        if gated && !self.accept_from(&event.speaker_id) {
            return;
        }
        match &event.payload {
            EventPayload::TurnStarted { iteration, .. } => {
                if *iteration == 1 {
                    // 新回合：轨道与统计从零开始。重播走同一条路，于是最后一个回合覆盖前一个。
                    self.current = TurnRecord::default();
                }
                // 每一次迭代都是新的一次模型调用：上一圈的首 token 与字数不许跟过来
                // （`.scratch/flow-page/spec.md` §2）。
                self.current.first_token_ms = None;
                self.current.streamed = 0;
                self.current.iteration = *iteration;
                self.enter(Node::Waiting, event.at);
            }
            EventPayload::MessageCompleted {
                role,
                text,
                first_token_ms,
                ..
            } => {
                if *role != Role::Assistant {
                    return;
                }
                if self.main.as_ref().is_none_or(|m| m != &event.speaker_id) {
                    return;
                }
                self.current.first_token_ms = *first_token_ms;
                if !text.is_empty() {
                    self.current.streamed = text.chars().count();
                }
                // 首 token 那一刻就是「等模型」与「生成」的分界。重播没有增量，靠这一个
                // 字段把两段的账补出来（ADR 0020）。
                let cut = match (
                    self.current.stamps[Node::Waiting.index()].entered,
                    *first_token_ms,
                ) {
                    (Some(start), Some(ms)) => start + chrono::Duration::milliseconds(ms as i64),
                    _ => event.at,
                };
                if matches!(self.phase, Phase::Waiting | Phase::Streaming) {
                    self.enter(Node::Streaming, cut);
                    self.current.stamps[Node::Streaming.index()].finished = Some(event.at);
                }
            }
            EventPayload::ToolCallStarted { tool_name, .. } => {
                self.current.tools += 1;
                self.current.tool = Some(tool_name.clone());
                self.current.tool_ms = None;
                self.current.gate = Gate::Passed;
                self.enter(Node::Tool, event.at);
            }
            EventPayload::ToolCallCompleted { duration_ms, .. } => {
                self.current.tool_ms = Some(*duration_ms);
                // 工具完成不切相位：同一次迭代可能还有下一个调用，而下一个 `TurnStarted`
                // 或 `TurnEnded` 才是「工具这一站开走」的信号。
            }
            EventPayload::PermissionDecided { decision, .. } => {
                self.current.gate = if *decision == Decision::Allow {
                    Gate::Allowed
                } else {
                    Gate::Denied
                };
            }
            EventPayload::TurnEnded { reason } => {
                self.current.reason = Some(*reason);
                self.enter(Node::Closing, event.at);
                self.last = Some(self.current.clone());
            }
            _ => {}
        }
    }

    /// 用户提交了一句：进入「收到你的话」，新回合的轨道从这里开始。
    pub fn note_submitted(&mut self, at: DateTime<Utc>) {
        self.current = TurnRecord::default();
        self.enter(Node::Received, at);
    }

    /// 走完这一帧之后把 `Closing` 收成 `Idle`（收尾节点只在那一帧亮着）。
    ///
    /// 返回「真的变了没有」：调用方只在变了的时候请下一帧 —— 每帧都置脏会把一个空闲的
    /// 界面变成永不停帧的忙循环。
    pub fn settle(&mut self) -> bool {
        if self.phase == Phase::Closing {
            // 收尾是一瞬间的事：进入那一刻就结束。空闲定格的五个节点因此都有停留时长
            // （收尾那一格读 `0.0 s`）。
            let closing = Node::Closing.index();
            self.current.stamps[closing].finished = self.current.stamps[closing].entered;
            self.phase = Phase::Idle;
            return true;
        }
        false
    }

    /// 相位（测试用）。
    pub fn phase(&self) -> Phase {
        self.phase
    }

    fn enter(&mut self, node: Node, at: DateTime<Utc>) {
        let index = node.index();
        // 跳站（例如一次没有文本、直接调用工具的迭代）也要把前面几站标成已过，
        // 否则轨道上会出现「后面亮了、前面空着」的假像。时间取这一站进入的那一刻。
        for earlier in &mut self.current.stamps[..index] {
            earlier.entered.get_or_insert(at);
            earlier.finished.get_or_insert(at);
        }
        let stamp = &mut self.current.stamps[index];
        stamp.entered = Some(at);
        stamp.finished = None;
        if let Some(old) = self.phase.node()
            && old != node
        {
            self.current.stamps[old.index()].finished.get_or_insert(at);
        }
        self.phase = node.phase();
    }

    /// 认主发言者：第一个 `Debater` 的回合事件**认领**这份面板，此后只认它。
    ///
    /// 名字里的 `accept` 是刻意的 —— 它不只是判等，第一次还会写下主发言者。
    fn accept_from(&mut self, speaker: &SpeakerId) -> bool {
        if let Some(main) = &self.main {
            return main == speaker;
        }
        let SpeakerId::Debater(_) = speaker else {
            return false;
        };
        self.main = Some(speaker.clone());
        true
    }

    /// 画出这一帧：头部一行、轨道若干行、贴底的说明区。
    pub fn frame(&self, input: &FrameInput<'_>) -> FlowFrame {
        // 空闲时定格上一回合；这时没有当前节点，五站全按「已过」画。
        let record = if self.phase == Phase::Idle {
            self.last.as_ref()
        } else {
            Some(&self.current)
        };
        let Some(record) = record else {
            return FlowFrame {
                head: self.head(input, None),
                track: Self::blank_track(input),
                note: self.note(input, None),
            };
        };
        FlowFrame {
            head: self.head(input, Some(record)),
            track: self.track(record, input),
            note: self.note(input, Some(record)),
        }
    }

    /// 头部：左 `回合 N · 迭代 M`，右当前节点已跑多久；空闲时只有「空闲」。
    fn head(&self, input: &FrameInput<'_>, record: Option<&TurnRecord>) -> Line<'static> {
        let dim = Style::default().fg(palette::MUTED);
        if self.phase == Phase::Idle {
            return Line::from(Span::styled(format!(" {}", wording::FLOW_IDLE), dim));
        }
        let record = record.expect("非空闲时总有一份当前回合");
        let ordinal = if self.phase == Phase::Received {
            input.turn + 1
        } else {
            input.turn
        };
        let left = format!(
            " {} {} · {} {}",
            wording::FLOW_TURN,
            ordinal,
            wording::FLOW_ITERATION,
            record.iteration
        );
        let right = self
            .current_node()
            .and_then(|node| self.node_ms(record, node, input.now))
            .map(|ms| wording::flow_elapsed(&wording::duration_text(ms)))
            .unwrap_or_default();
        let room = input
            .width
            .saturating_sub(width::text_columns(&left) as u16)
            .saturating_sub(width::text_columns(&right) as u16);
        Line::from(Span::styled(
            format!("{left}{}{right}", " ".repeat(room as usize)),
            dim,
        ))
    }

    /// 相位对应的当前节点；空闲是 `None`。
    fn current_node(&self) -> Option<Node> {
        self.phase.node()
    }

    /// 轨道：五节点竖向排列，当前节点展开两行子块，工具之后一条回边。
    ///
    /// 高度阶梯（`.scratch/flow-page/spec.md` §3）：`≥ 14` 行画全量；`10..14` 行只留当前
    /// 节点与前后各一个（被裁那一端画 `⋮`）；`< 10` 行只画当前节点一行。
    fn track(&self, record: &TurnRecord, input: &FrameInput<'_>) -> Vec<Line<'static>> {
        if input.height < 10 {
            let node = self.current_node().unwrap_or(Node::Closing);
            let state = if self.current_node().is_some() {
                NodeState::Current
            } else {
                NodeState::DoneOf(record.reason)
            };
            let side = self.side_note(record, node, input);
            return vec![Self::node_line(node, state, side, input.width)];
        }
        if input.height < 14 {
            return self.window_track(record, input);
        }
        self.full_track(record, input)
    }

    /// 全量轨道：五站、当前节点子块、回边。
    fn full_track(&self, record: &TurnRecord, input: &FrameInput<'_>) -> Vec<Line<'static>> {
        let current = self.current_node();
        let mut lines = Vec::new();
        for (index, node) in Node::ALL.iter().enumerate() {
            let state = self.node_state_for(record, *node);
            let side = self.side_note(record, *node, input);
            lines.push(Self::node_line(*node, state, side, input.width));
            if Some(*node) == current {
                lines.extend(self.dropdown(record, *node, input));
            }
            if index + 1 < Node::ALL.len() {
                lines.push(Self::connector());
                if *node == Node::Tool {
                    lines.push(Self::back_edge());
                }
            }
        }
        lines
    }

    /// 窗口轨道：只留当前节点与前后各一站，被裁那一端画 `⋮`。子块在窗口里不展开。
    fn window_track(&self, record: &TurnRecord, input: &FrameInput<'_>) -> Vec<Line<'static>> {
        // 空闲定格没有当前节点：窗口停在最后一站（刚收起来的那个收尾）。
        let center = self
            .current_node()
            .map_or(Node::Closing.index(), Node::index);
        let lo = center.saturating_sub(1);
        let hi = (center + 1).min(Node::ALL.len() - 1);
        let mut lines = Vec::new();
        if lo > 0 {
            lines.push(Self::truncated());
        }
        for index in lo..=hi {
            let node = Node::ALL[index];
            let state = self.node_state_for(record, node);
            let side = self.side_note(record, node, input);
            lines.push(Self::node_line(node, state, side, input.width));
            if index < hi {
                lines.push(Self::connector());
            }
        }
        if hi < Node::ALL.len() - 1 {
            lines.push(Self::truncated());
        }
        lines
    }

    fn node_state_for(&self, record: &TurnRecord, node: Node) -> NodeState {
        match self.current_node() {
            Some(current) if node == current => NodeState::Current,
            Some(current) if node.index() < current.index() => NodeState::Done,
            Some(_) => NodeState::Todo,
            // 空闲：上一回合的定格，五站全已过；收尾那一站按停止原因上色。
            None => NodeState::DoneOf(record.reason),
        }
    }

    /// 被窗口裁掉的那一端。
    fn truncated() -> Line<'static> {
        Line::from(Span::styled(
            wording::flow_truncated_line(),
            Style::default().fg(palette::MUTED),
        ))
    }

    /// 空闲且从没跑过回合时，轨道仍然画出那些空节点：布局不因为「还没开始」而塌掉。
    fn blank_track(input: &FrameInput<'_>) -> Vec<Line<'static>> {
        let nodes: &[Node] = if input.height < 14 {
            &Node::ALL[..2]
        } else {
            &Node::ALL
        };
        let mut lines = Vec::new();
        for (index, node) in nodes.iter().enumerate() {
            lines.push(Self::node_line(*node, NodeState::Todo, None, 0));
            if index + 1 < nodes.len() {
                lines.push(Self::connector());
                if *node == Node::Tool {
                    lines.push(Self::back_edge());
                }
            }
        }
        lines
    }

    /// 一个节点的侧注：已过与当前带停留时长，流式带字数。
    fn side_note(&self, record: &TurnRecord, node: Node, input: &FrameInput<'_>) -> Option<String> {
        let done = record.stamps[node.index()].finished.is_some();
        if !done && Some(node) != self.current_node() {
            return None;
        }
        let ms = self.node_ms(record, node, input.now)?;
        let duration = wording::duration_text(ms);
        match node {
            Node::Streaming if record.streamed > 0 => {
                Some(wording::flow_streaming_note(&duration, record.streamed))
            }
            _ => Some(duration),
        }
    }

    /// 一个节点这一次经过的毫秒数；没进过就是 `None`。
    ///
    /// 等模型段的定格值优先用 `first_token_ms`；生成段则扣掉它 —— 重播没有增量事件，
    /// 两段的账只能从这一个字段推（ADR 0020）。
    fn node_ms(&self, record: &TurnRecord, node: Node, now: DateTime<Utc>) -> Option<u64> {
        let stamp = record.stamps[node.index()];
        let entered = stamp.entered?;
        let finished = stamp.finished;
        // 进行中的那一段用「现在」结算；其它都是定格值，重播因此不会显示成一个巨大的数。
        let end = finished.unwrap_or(now);
        let total = (end - entered).num_milliseconds().max(0) as u64;
        match node {
            Node::Waiting => record.first_token_ms.or(Some(total)),
            // 生成段的 `entered` 已经被钉在首 token 那一刻（`MessageCompleted` 用
            // `first_token_ms` 算出的切点），所以这一段的长度就是 `finished - entered`，
            // 不再扣一次 —— 双重扣减会把生成时长算成负的又 clamp 回零。
            Node::Streaming => Some(total),
            _ => Some(total),
        }
    }

    /// 当前节点的子块：窄档至多两行，宽档三行。
    fn dropdown(
        &self,
        record: &TurnRecord,
        node: Node,
        input: &FrameInput<'_>,
    ) -> Vec<Line<'static>> {
        let wide = input.width >= crate::render::layout::RAIL_WIDE;
        match node {
            Node::Waiting => vec![
                Self::sub(true, wording::FLOW_REQUEST_SHAPE),
                Self::sub(false, wording::FLOW_BUDGET_CHECK),
            ],
            Node::Tool => {
                let mut lines = Vec::new();
                // 询问中（还没有裁决、也没在等人）与等人答是两句话：一个是循环在问自己，
                // 一个是键盘已经交出去了。
                let gate = match (record.gate, input.awaiting) {
                    (Gate::Allowed, _) => wording::FLOW_PERMISSION_ALLOWED,
                    (Gate::Denied, _) => wording::FLOW_PERMISSION_DENIED,
                    (Gate::Passed, true) => wording::FLOW_PERMISSION_WAITING,
                    (Gate::Passed, false) => wording::FLOW_PERMISSION_PASSED,
                };
                lines.push(Self::sub(true, gate));
                let tool = record.tool.as_deref().unwrap_or(wording::FLOW_NODE_TOOL);
                let tool_line = wording::flow_tool_line(tool, record.tool_ms);
                if wide {
                    // 宽档多一行：结果还没回来这件事本身也说出来。
                    lines.push(Self::sub(true, &tool_line));
                    lines.push(Self::sub(false, wording::FLOW_TOOL_RESULT));
                } else {
                    lines.push(Self::sub(false, &tool_line));
                }
                lines
            }
            _ => Vec::new(),
        }
    }

    /// 说明区：小节线 + 当前节点那句话 + 统计行。高度交给排版，`height` 决定最多放几行。
    fn note(&self, input: &FrameInput<'_>, record: Option<&TurnRecord>) -> Vec<Line<'static>> {
        let text = match self.phase {
            Phase::Idle => wording::flow_note_idle(),
            Phase::Received => wording::flow_note_received(),
            Phase::Waiting => wording::flow_note_waiting(),
            Phase::Streaming => {
                let chars = record.map_or(0, |record| record.streamed);
                wording::flow_note_streaming(chars)
            }
            Phase::Tool => {
                if input.awaiting {
                    wording::flow_note_awaiting()
                } else {
                    wording::flow_note_tool(record.and_then(|record| record.tool.as_deref()))
                }
            }
            Phase::Closing => wording::flow_note_closing(record.and_then(|record| record.reason)),
        };
        // 高度阶梯（`.scratch/flow-page/spec.md` §3）：`< 10` 行只留一句话、不画小节线；
        // `10..14` 行画小节线 + 一行说明；`14..18` 行丢统计；`≥ 18` 行全量。
        let room = input.height;
        if room < 10 {
            let mut lines = pane::wrap_text(&text, input.width as usize);
            lines.truncate(1);
            return lines;
        }
        let mut lines = vec![Self::rule(input.width)];
        let mut wrapped = pane::wrap_text(&text, input.width as usize);
        if room < 14 {
            wrapped.truncate(1);
        }
        lines.extend(wrapped);
        if room >= 18
            && let Some(record) = record
        {
            lines.push(Self::stats(record, input));
        }
        lines
    }

    /// 统计行：迭代数 · 工具数 · 上下文占比（宽档换成输入/输出 token）。
    fn stats(record: &TurnRecord, input: &FrameInput<'_>) -> Line<'static> {
        let wide = input.width >= crate::render::layout::RAIL_WIDE;
        Line::from(Span::styled(
            format!(
                " {}",
                wording::flow_stats(
                    record.iteration,
                    record.tools,
                    input.share,
                    wide,
                    input.usage,
                    input.number_style,
                )
            ),
            Style::default().fg(palette::MUTED),
        ))
    }

    fn node_line(node: Node, state: NodeState, side: Option<String>, width: u16) -> Line<'static> {
        let (marker, style) = state.mark();
        let left = format!(" {marker} {}", node.label());
        let mut spans = vec![
            Span::styled(format!(" {}", marker), style),
            Span::raw(" "),
            Span::styled(node.label(), style),
        ];
        if let Some(side) = side {
            let room = width
                .saturating_sub(width::text_columns(&left) as u16)
                .saturating_sub(width::text_columns(&side) as u16);
            spans.push(Span::raw(" ".repeat(room as usize + 1)));
            spans.push(Span::styled(side, Style::default().fg(palette::MUTED)));
        }
        Line::from(spans)
    }

    fn connector() -> Line<'static> {
        Line::from(Span::styled(
            wording::flow_connector_line(),
            Style::default().fg(palette::MUTED),
        ))
    }

    /// 回边：agent 是一个循环，不是流水线（`.scratch/flow-page/spec.md` §3）。
    fn back_edge() -> Line<'static> {
        Line::from(Span::styled(
            wording::flow_back_edge_line(),
            Style::default().fg(palette::MUTED),
        ))
    }

    /// 当前节点子块的一行（`first` 决定 `├` 还是 `└`）。
    fn sub(first: bool, text: &str) -> Line<'static> {
        Line::from(Span::styled(
            wording::flow_sub_line(first, text),
            Style::default().fg(palette::MUTED),
        ))
    }

    /// 说明区上面那条小节线：内容档实线，颜色是框架色（`palette::CHROME`）。
    fn rule(width: u16) -> Line<'static> {
        Line::from(Span::styled(
            format!(
                " {}",
                wording::FLOW_RULE.repeat(width.saturating_sub(2) as usize)
            ),
            Style::default().fg(palette::CHROME),
        ))
    }
}

/// 一个节点画成什么样子。
#[derive(Debug, Clone, Copy)]
enum NodeState {
    /// 已过：`✓`，MUTED。
    Done,
    /// 已过、而且这一回合是以异常收的场：`✗`，按原因上色。
    DoneOf(Option<StopReason>),
    /// 当前：`◆`，ACCENT + BOLD。
    Current,
    /// 未到：`○`，MUTED。
    Todo,
}

impl NodeState {
    fn mark(self) -> (&'static str, Style) {
        match self {
            NodeState::Done => (wording::FLOW_MARK_DONE, Style::default().fg(palette::MUTED)),
            NodeState::DoneOf(reason) => match reason {
                Some(StopReason::Aborted) => (
                    wording::FLOW_MARK_FAILED,
                    Style::default().fg(palette::WARN),
                ),
                Some(
                    StopReason::Error
                    | StopReason::BudgetExhausted
                    | StopReason::MaxIterations
                    | StopReason::MistakeLimit,
                ) => (wording::FLOW_MARK_FAILED, Style::default().fg(palette::BAD)),
                _ => (wording::FLOW_MARK_DONE, Style::default().fg(palette::MUTED)),
            },
            NodeState::Current => (
                wording::FLOW_MARK_CURRENT,
                Style::default()
                    .fg(palette::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            NodeState::Todo => (wording::FLOW_MARK_TODO, Style::default().fg(palette::MUTED)),
        }
    }
}

/// 一帧流程页画出来的东西。
#[derive(Debug, Clone)]
pub struct FlowFrame {
    /// 头部一行：`回合 N · 迭代 M` 与当前节点已跑时长。
    pub head: Line<'static>,
    /// 轨道：五节点、当前节点子块、回边。
    pub track: Vec<Line<'static>>,
    /// 贴底的说明区：小节线、说明文字、统计行。
    pub note: Vec<Line<'static>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kimi() -> SpeakerId {
        SpeakerId::Debater("kimi".into())
    }

    fn executor() -> SpeakerId {
        SpeakerId::Executor("kimi-1".into())
    }

    fn event(seq: u64, speaker: SpeakerId, payload: EventPayload) -> Event {
        Event::new(seq, speaker, payload)
    }

    fn turn_started(seq: u64, speaker: SpeakerId, iteration: u32) -> Event {
        event(
            seq,
            speaker,
            EventPayload::TurnStarted {
                agent: kimi(),
                iteration,
            },
        )
    }

    fn message(text: &str, first_token_ms: Option<u64>) -> Event {
        event(
            2,
            kimi(),
            EventPayload::MessageCompleted {
                role: Role::Assistant,
                text: text.to_owned(),
                reasoning: None,
                first_token_ms,
            },
        )
    }

    fn tool_started(seq: u64, speaker: SpeakerId, name: &str) -> Event {
        event(
            seq,
            speaker,
            EventPayload::ToolCallStarted {
                tool_call_id: crate::events::ToolCallId::new("c-1"),
                tool_name: name.to_owned(),
                args: serde_json::json!({}),
            },
        )
    }

    fn tool_completed(seq: u64) -> Event {
        event(
            seq,
            kimi(),
            EventPayload::ToolCallCompleted {
                tool_call_id: crate::events::ToolCallId::new("c-1"),
                ok: true,
                output: None,
                error: None,
                duration_ms: 800,
            },
        )
    }

    fn permission(seq: u64, allow: bool) -> Event {
        event(
            seq,
            kimi(),
            EventPayload::PermissionDecided {
                request_id: "r-1".to_owned(),
                decision: if allow {
                    Decision::Allow
                } else {
                    Decision::Deny
                },
                source: crate::events::DecisionSource::User,
                reason: None,
            },
        )
    }

    fn turn_ended(seq: u64, reason: StopReason) -> Event {
        event(seq, kimi(), EventPayload::TurnEnded { reason })
    }

    fn observe(panel: &mut FlowPanel, event: &Event) {
        panel.observe(
            &RenderEvent::Logged {
                session: crate::events::SessionId::new("s-1"),
                event: event.clone(),
            },
            event.at,
        );
    }

    fn delta(panel: &mut FlowPanel, text: &str) {
        panel.observe(
            &RenderEvent::Delta {
                session: crate::events::SessionId::new("s-1"),
                speaker: kimi(),
                kind: DeltaKind::Text,
                text: text.to_owned(),
            },
            Utc::now(),
        );
    }

    fn input(width: u16, height: u16) -> FrameInput<'static> {
        FrameInput {
            turn: 0,
            share: "36%",
            awaiting: false,
            usage: None,
            number_style: wording::NumberStyle::Cn,
            width,
            height,
            now: Utc::now(),
        }
    }

    fn text(lines: &[Line<'_>]) -> String {
        lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn text_of(frame: &FlowFrame) -> String {
        format!(
            "{}\n{}\n{}",
            frame.head,
            text(&frame.track),
            text(&frame.note)
        )
    }

    fn frame_text(panel: &FlowPanel, input: &FrameInput<'_>) -> String {
        text_of(&panel.frame(input))
    }

    #[test]
    fn a_turn_walks_the_track_in_order() {
        let mut panel = FlowPanel::default();
        let now = Utc::now();
        panel.note_submitted(now);
        assert_eq!(panel.phase(), Phase::Received);
        assert_eq!(panel.phase().node(), Some(Node::Received));

        observe(&mut panel, &turn_started(1, kimi(), 1));
        assert_eq!(panel.phase(), Phase::Waiting);

        delta(&mut panel, "好");
        assert_eq!(panel.phase(), Phase::Streaming);

        observe(&mut panel, &message("好的", Some(120)));
        assert_eq!(panel.phase(), Phase::Streaming, "消息完成不切相位");

        observe(&mut panel, &tool_started(3, kimi(), "bash"));
        assert_eq!(panel.phase(), Phase::Tool);

        observe(&mut panel, &turn_ended(4, StopReason::Completed));
        assert_eq!(panel.phase(), Phase::Closing, "收尾节点亮在那一帧");
        panel.settle();
        assert_eq!(panel.phase(), Phase::Idle);
    }

    #[test]
    fn an_executor_or_a_second_debater_never_moves_the_nodes() {
        let mut panel = FlowPanel::default();
        let now = Utc::now();
        panel.note_submitted(now);
        // 执行者的回合事件先到：面板还没认主发言者，于是它被挡掉。
        observe(&mut panel, &turn_started(1, executor(), 1));
        assert_eq!(panel.phase(), Phase::Received);
        // 主发言者认领之后，执行者的事件仍然进不来。
        observe(&mut panel, &turn_started(2, kimi(), 1));
        assert_eq!(panel.phase(), Phase::Waiting);
        observe(&mut panel, &tool_started(3, executor(), "bash"));
        assert_eq!(panel.phase(), Phase::Waiting);
        // 讨论的第二位发言者同样进不来。
        let other = SpeakerId::Debater("other".into());
        observe(&mut panel, &turn_started(4, other, 1));
        assert_eq!(panel.phase(), Phase::Waiting);
    }

    #[test]
    fn a_new_turn_resets_the_track() {
        let mut panel = FlowPanel::default();
        observe(&mut panel, &turn_started(1, kimi(), 1));
        observe(&mut panel, &turn_ended(2, StopReason::Completed));
        panel.settle();
        observe(&mut panel, &turn_started(3, kimi(), 1));
        assert_eq!(panel.phase(), Phase::Waiting);
        let text = frame_text(&panel, &input(42, 24));
        assert!(text.contains("◆ 等模型响应"), "{text}");
        assert!(
            !text.contains("✓ 流式输出") && !text.contains("✓ 工具调用"),
            "上一回合的痕迹清掉了：{text}"
        );
    }

    #[test]
    fn the_frame_marks_exactly_one_current_node() {
        let mut panel = FlowPanel::default();
        observe(&mut panel, &turn_started(1, kimi(), 1));
        let text = frame_text(&panel, &input(42, 24));
        assert_eq!(text.matches('◆').count(), 1, "只有一个当前节点：{text}");
        assert!(text.contains("◆ 等模型响应"), "{text}");
        assert!(text.contains("✓ 收到你的话"), "前一个节点已过：{text}");
    }

    #[test]
    fn an_idle_panel_without_history_has_no_current_node() {
        let panel = FlowPanel::default();
        let text = frame_text(&panel, &input(42, 24));
        assert_eq!(text.matches('◆').count(), 0, "空闲没有当前节点：{text}");
        assert_eq!(text.matches('○').count(), 5, "五站全未到：{text}");
        assert!(text.contains(wording::FLOW_IDLE), "{text}");
    }

    #[test]
    fn the_current_node_carries_its_side_note_and_subblock() {
        let mut panel = FlowPanel::default();
        observe(&mut panel, &turn_started(1, kimi(), 1));
        let text = frame_text(&panel, &input(42, 24));
        assert!(text.contains(wording::FLOW_REQUEST_SHAPE), "子块：{text}");
        assert!(text.contains(wording::FLOW_BUDGET_CHECK), "子块：{text}");
        assert!(text.contains("0.0 s"), "侧注有停留时长：{text}");

        observe(&mut panel, &tool_started(2, kimi(), "bash"));
        observe(&mut panel, &permission(3, true));
        let text = frame_text(&panel, &input(42, 24));
        assert!(text.contains(wording::FLOW_PERMISSION_ALLOWED), "{text}");
        assert!(text.contains("bash · 执行中"), "{text}");
        assert!(
            text.contains(wording::FLOW_BACK_EDGE),
            "回边始终画着：{text}"
        );
    }

    #[test]
    fn a_waiting_human_says_so_in_the_tool_subblock() {
        let mut panel = FlowPanel::default();
        observe(&mut panel, &turn_started(1, kimi(), 1));
        observe(&mut panel, &tool_started(2, kimi(), "bash"));
        let mut asking = input(42, 24);
        asking.awaiting = true;
        let text = text_of(&panel.frame(&asking));
        assert!(text.contains(wording::FLOW_PERMISSION_WAITING), "{text}");
        assert!(text.contains(&wording::flow_note_awaiting()), "{text}");
        // 没在等人（例如策略直接放行/拒绝）时才是「询问中」。
        let deciding = input(42, 24);
        let text = text_of(&panel.frame(&deciding));
        assert!(text.contains(wording::FLOW_PERMISSION_PASSED), "{text}");
    }

    #[test]
    fn the_streaming_node_counts_the_characters_it_got() {
        let mut panel = FlowPanel::default();
        observe(&mut panel, &turn_started(1, kimi(), 1));
        delta(&mut panel, "一二三");
        let text = frame_text(&panel, &input(42, 24));
        assert!(text.contains("3 字"), "已收字数进侧注：{text}");
    }

    #[test]
    fn the_note_and_stats_sit_at_the_bottom() {
        let mut panel = FlowPanel::default();
        observe(&mut panel, &turn_started(1, kimi(), 1));
        let frame = panel.frame(&input(42, 24));
        let note = text(&frame.note);
        assert!(note.contains(&wording::flow_note_waiting()), "{note}");
        assert!(note.contains("1 次迭代"), "统计：{note}");
        assert!(note.contains("0 次工具"), "统计：{note}");
        assert!(note.contains("36%"), "上下文占比：{note}");
    }

    #[test]
    fn an_idle_panel_freezes_the_last_turn() {
        let mut panel = FlowPanel::default();
        observe(&mut panel, &turn_started(1, kimi(), 1));
        observe(&mut panel, &message("好的", Some(120)));
        observe(&mut panel, &tool_started(3, kimi(), "bash"));
        observe(&mut panel, &tool_completed(4));
        observe(&mut panel, &turn_ended(5, StopReason::Completed));
        panel.settle();
        let text = frame_text(&panel, &input(42, 24));
        assert!(text.contains(wording::FLOW_IDLE), "{text}");
        assert_eq!(text.matches('✓').count(), 5, "五站全已过：{text}");
        assert!(text.contains("1 次工具"), "统计跟着上一回合：{text}");
        assert!(text.contains(&wording::flow_note_idle()), "{text}");
    }

    #[test]
    fn the_wide_tier_adds_tokens_and_a_third_tool_line() {
        let mut panel = FlowPanel::default();
        observe(&mut panel, &turn_started(1, kimi(), 1));
        observe(&mut panel, &tool_started(2, kimi(), "bash"));

        let mut wide = input(60, 24);
        wide.usage = Some((12_400, 1_800));
        let text = format!(
            "{}\n{}",
            text(&panel.frame(&wide).track),
            text(&panel.frame(&wide).note)
        );
        assert!(text.contains("输入"), "宽档统计带 token：{text}");
        assert!(text.contains("输出"), "宽档统计带 token：{text}");
        assert!(
            text.contains(wording::FLOW_TOOL_RESULT),
            "工具第三行：{text}"
        );

        let narrow = frame_text(&panel, &input(42, 24));
        assert!(
            !narrow.contains(wording::FLOW_TOOL_RESULT),
            "窄档只有两行：{narrow}"
        );
        assert!(!narrow.contains("输入"), "窄档统计不带 token：{narrow}");
    }

    #[test]
    fn the_two_segments_use_the_first_token_cut_not_double_subtraction() {
        // 首 token 500 ms、整次调用 2 s → 等模型 0.5 s、生成 1.5 s。生成段如果再把 500 ms
        // 扣一次就会变成 1.0 s（或负数被 clamp 成 0）。
        let mut panel = FlowPanel::default();
        let mut started = turn_started(1, kimi(), 1);
        started.at = DateTime::from_timestamp(1_000, 0).expect("合法时刻");
        let mut done = message("好的", Some(500));
        done.at = DateTime::from_timestamp(1_002, 0).expect("合法时刻");
        observe(&mut panel, &started);
        observe(&mut panel, &done);

        let text = frame_text(&panel, &input(42, 24));
        assert!(text.contains("0.5 s"), "等模型段用首 token：{text}");
        assert!(text.contains("1.5 s"), "生成段是剩下的那段：{text}");
        assert!(!text.contains("1.0 s"), "不许把首 token 再扣一次：{text}");
    }

    #[test]
    fn a_second_iteration_forgets_the_previous_deltas() {
        let mut panel = FlowPanel::default();
        observe(&mut panel, &turn_started(1, kimi(), 1));
        delta(&mut panel, "一二三");
        assert!(frame_text(&panel, &input(42, 24)).contains("3 字"));
        observe(&mut panel, &turn_started(3, kimi(), 2));
        let text = frame_text(&panel, &input(42, 24));
        assert!(!text.contains("3 字"), "第二次迭代的字数从零起：{text}");
        assert!(
            !text.contains("首 token") && !text.contains("0.5 s"),
            "上一圈的首 token 不许跟过来：{text}"
        );
    }

    #[test]
    fn an_executor_delta_does_not_push_the_phase() {
        let mut panel = FlowPanel::default();
        observe(&mut panel, &turn_started(1, kimi(), 1));
        panel.observe(
            &RenderEvent::Delta {
                session: crate::events::SessionId::new("s-1"),
                speaker: executor(),
                kind: DeltaKind::Text,
                text: "执行者在说话".to_owned(),
            },
            Utc::now(),
        );
        assert_eq!(
            panel.phase(),
            Phase::Waiting,
            "执行者的增量不是主会话的流式"
        );
    }

    #[test]
    fn the_idle_freeze_gives_the_closing_node_a_duration_too() {
        let mut panel = FlowPanel::default();
        observe(&mut panel, &turn_started(1, kimi(), 1));
        observe(&mut panel, &turn_ended(2, StopReason::Completed));
        panel.settle();
        let text = frame_text(&panel, &input(42, 24));
        assert_eq!(text.matches('✓').count(), 5, "五站全已过：{text}");
        assert!(text.contains("0.0 s"), "收尾那一格也有停留时长：{text}");
    }

    #[test]
    fn an_aborted_turn_marks_the_closing_node() {
        let mut panel = FlowPanel::default();
        observe(&mut panel, &turn_started(1, kimi(), 1));
        observe(&mut panel, &turn_ended(2, StopReason::Aborted));
        panel.settle();
        let text = frame_text(&panel, &input(42, 24));
        assert!(text.contains('✗'), "取消的收尾画成叉：{text}");
        let frame = panel.frame(&input(42, 24));
        let closing = frame
            .track
            .iter()
            .find(|line| line.to_string().contains(Node::Closing.label()))
            .expect("收尾那一行在");
        let mark = closing.spans.first().expect("记号").content.to_string();
        assert!(mark.contains('✗'), "{mark:?}");
        assert!(text.contains(&wording::flow_note_idle()), "{text}");
    }
}
