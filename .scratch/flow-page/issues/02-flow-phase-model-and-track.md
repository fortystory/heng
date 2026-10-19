# 02: 流程页的相位模型与五节点轨道

Type: implement
Status: done
Blocked by: 01

> 规格：[`../spec.md`](../spec.md) §2。帧形状：[`../prototype/frames.txt`](../prototype/frames.txt)
> 的「A 等权全图」三态。

**要建的：** 流程页活起来。新模块持 `FlowPanel`（每会话一份），从 TUI 已经收得到的事件推
出一个六态相位机；页面上画出五个节点的纵向轨道，当前节点 `◆`、已过 `✓`、未到 `○`，头部
`回合 N · 迭代 M` 与当前节点已跑时长。跑一次回合能亲眼看见节点一格一格推进。

## 具体行为

1. 新模块 `render::flow` 持 `FlowPanel`，放进 `View`（与时间轴、面板、todo 同级）。两个接口：
   `apply(event, state)` 是唯一入口，`frame(width, rows, now)` 是纯函数、产出头部与轨道行；
   `now` 由调用方传入，于是耗时在测试里可钉死。
2. 相位：`Idle / Received / Waiting / Streaming / Tool / Closing`。
   - 用户提交 → `Received`；`TurnStarted` → `Waiting`；
   - 实时收到 `Delta` → `Streaming`；`MessageCompleted` 之后等下一个事件；
   - `ToolCallStarted` → `Tool`；配对的 `ToolCallCompleted` → 回到等待/流式之间（有下一次
     调用就留 `Tool`，否则等 `TurnStarted` / `TurnEnded`）；
   - `TurnEnded` → `Closing`，下一帧回 `Idle`。
3. **只认主发言者的事件**（`SessionFacts.speaker_order` 的第一个）：`TurnStarted`、
   `TurnEnded`、`MessageCompleted`、`ToolCallStarted`、`ToolCallCompleted` 的 `speaker_id`
   对不上就整条忽略 —— 执行者与讨论者的同名事件不许让节点乱跳。
4. 头部：`回合 N · 迭代 M`。回合号读 `View::turn_ordinal`（与轨迹、用量的口径同一个，不另数）；
   迭代号读 `TurnStarted.iteration`。当前节点右对齐显示「进入现在多久」（`now - entered_at`）。
5. 轨道：五节点竖向排列，连接线 `│`（MUTED）。当前节点 `◆`（ACCENT + BOLD）、已过 `✓`
   （MUTED）、未到 `○`（MUTED）。节点名进措辞层，42 列档 ≤ 5 个中文字。
6. 这一票先不做：侧注时长、子块、回边、说明区、统计、空闲定格（票 03）。空闲态先画全 `○`
   + 头部「空闲」。
7. 历史重播同一条路径：重放 `Logged` 事件时状态机自然演化，播完停在 `Idle`。`Delta` 只在
   实时存在，重播没有它 —— 状态机不许因此错位。

## 验收

- [ ] 帧测试喂一段事件序列：同一帧里依次出现 `◆ 收到你的话` → `◆ 等模型响应` →
      `◆ 流式输出` → `◆ 工具调用` → 收尾，且任一帧只有一个 `◆`。
- [ ] 相位单测：三态、`TurnEnded` 的各个 `StopReason`、非主发言者事件被忽略、
      `first_token_ms = None` 的老流、重播后回 `Idle`。
- [ ] 空闲帧：五节点全 `○`，头部「空闲」。
- [ ] 42 与 60 两档帧都能画出完整五节点，节点名不折行。
- [ ] `cargo test` 全绿、`clippy` 与基线同、`fmt --check` 只留既有的两处漂移。
