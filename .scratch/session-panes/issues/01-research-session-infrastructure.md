# 宿主表与多会话基础设施：同屏多会话站在哪些件上

Type: research
Status: resolved
Part of: ../map.md
Blocked by: —

## 问题

[图](../map.md) 的冻结项 2 定了平铺是**同进程多会话**。要判断这条路要动多少地方，先得有一份底账：
今天这套「一个进程里几个会话」的设施里，**哪些已经按会话分开、哪些是全进程一份**。没有这份底账，
[全屏唯一的东西在平铺下怎么活](06-grilling-single-instance-surfaces.md) 与
[每会话的视图状态与两态切换](10-grilling-per-session-view-state.md) 就只能在猜。

**这是只读调研，不改任何代码。** 结论一律给 `文件:行号`。

要清点七件事：

1. **宿主表本身**。`SessionHost` / `HostEntry` / `SessionState` 的字段与容量
   （`src/session/host.rs`）；`adopt` / `adopt_read_only` / `adopt_with` / `switch` / `close_check`
   的语义；命令通道（每会话一条 `mpsc`）、状态回传（每会话一条 `watch`）、渲染通道（`broadcast`）
   各自的形状与「满了会怎样」。
2. **前台指针**。全进程唯一的 `watch<SessionId>` 被谁订阅（`follow_foreground` /
   `subscribe_foreground` 的调用点，含 `src/cli/interactive.rs` 的轮询）；渲染器拿到前台的路径；
   `Ctrl-C` 取消怎么找到「前台那个会话」（`src/render/tui.rs` 到 `CancelSignal` 这一路）；退出
   举手与终端标题读的是谁。要做「焦点 pane」，这一套够不够、缺什么。
3. **一个会话的运行件**。`Harness` 的构造与 `SessionParts`（`src/session/mod.rs`）；一次 `Turn`
   从命令到事件流的路径（`src/agent.rs` 的 `run_turn`）；每会话的 `EventLog` / `next_seq` 归属；
   写同一个 `log.jsonl` 的并发约束。
4. **渲染通道的扇出**。渲染事件带不带 `SessionId`（`session-tab` 票 14 的
   `render-event-session-id`）；`RenderHandle` 的订阅端；多份事件流同时活着时，今天的画家靠什么
   知道「这段该画在谁身上」。
5. **视图状态的所有者**。`TuiState` / `View` / `pane::Pane` 的字段里，哪些已经是「每会话一份」、
   哪些是单例；`switch` 与「切会话」的实际路径（`View::new` + `SessionLight` 收装 + 整条流重放）
   搬了什么、丢了什么。**口径**：`src/render/pane.rs` 的 `Pane` 是**转录滚动缓冲**（一个会话两个
   实例），**不是**屏幕分块 —— 本图说的「pane」与它撞词，命名归
   [轨迹搬走之后：主列、页签条与文档](12-grilling-main-column-after-move.md)。
6. **装载与关闭**。`cli::loading::attach`（`session-tab` 票 33 / 34）的输入与产出；装载路径依赖的
   单例（渲染器、driver、`PathLocks`、工作区锁）；`close_check` / `exit_gate` 的判据。同屏多会话
   是否与「渲染器唯一」「一个进程一个终端」冲突。
7. **可复用的先例**。讨论会话与执行者在同一进程里怎么并存（`src/agent/executor.rs`、
   `src/discussion.rs`、`src/permissions.rs` 的委派链继承）—— 它们是不是「多份流同时活着」的既有
   先例，能不能照搬。

## 作答

**（2026-10-17，research subagent 完成，只读；产物
[`research/01-session-infrastructure.md`](../research/01-session-infrastructure.md)，310 行。
被测 HEAD `ad5aaf6`。）**

1. **通道层的地基已经铺好，缺的是消费端**：`RenderEvent` 七种全带 `session: SessionId`
   （`src/render/mod.rs:86`–`:139`），但 `live_event` 第一句就把非前台事件丢掉
   （`src/render/tui.rs:4441`），投影只有一份 —— 多 pane 的核心改造是**按 `SessionId` 路由 +
   每个 pane 一份 `View`**。
2. **前台指针真的全进程唯一，而且只服务一个问题**：`watch<SessionId>`（`host.rs:260`）全仓只有
   一个生产订阅点（`interactive.rs:293`），真正 `wait_for` 的只有 `SessionAsker`
   （`asker.rs:62`–`:73`）；`follow_foreground`（`host.rs:349`）在生产代码里没有调用点。它够当
   「焦点」，**不够当「平铺集合」** —— 集合要有自己的容器（手势归
   [平铺的手势与焦点](07-grilling-pane-gestures-and-focus.md)，状态归
   [每会话的视图状态与两态切换](10-grilling-per-session-view-state.md)）。
3. **两个最容易被忽视的单例**：① `TuiState.cwd` / `home`（`tui.rs:2479`）从组装时钉死，
   `SwitchSession` **不更新它**（`tui.rs:6987`–`:7018`），而终端标题（`tui.rs:4042`）与文件索引的
   根（`tui.rs:500`、`:7905`）都读它 —— **今天切到别 cwd 的会话，标题与 `@` 候选还是旧的**（既有
   能力里的现存缺陷，本图顺手收）；② 整屏几何 `layout::plan` / `Regions`（`layout.rs:459`、
   `layout.rs:180`）没有 pane 维度。
4. **`Pane` 是转录滚动缓冲，不是屏幕分块**：`src/render/pane.rs:30` 的 `Pane` 一个会话两个实例
   （`tui.rs:793`、`:798`）。本图说的「pane」在今天的代码里**没有对应类型** —— 术语撞车，命名归
   [轨迹搬走之后：主列、页签条与文档](12-grilling-main-column-after-move.md) 第 3 条。
5. **宿主表本身没有上限**（`host.rs:258` 的 `Vec` 无容量约束）：「4」是**装载策略**，不在这层 ——
   冻结项 3 的上限要落在一个新的集合类型上，不是给宿主表加容量。
6. **没有「多份流」的既有先例**：执行者与父会话共用同一个 `SessionId`（`executor.rs:126`）、同一个
   `log`、同一个 `RenderHandle`，靠 `SpeakerId::Executor` 区分（`executor.rs:144`）；讨论者经
   `Session::fork` 同样共享 `id`（`session/mod.rs:301`）。能借的只有「共享句柄以克隆注入」的组装法
   （`loading.rs:45`–`:96` 已在用），借不到分派机制 —— 这一条**证实**了冻结项 10（多角色分屏不在
   本图）。
7. **修正票面两处**：`src/render/tui.rs` 实测 **17964 行**（不是 4000+）；`tui.rs` 里**没有
   `rebuild` 函数**，切会话的实际路径是 `View::new` + `SessionLight` 收装 + 整条流重放。
8. **顺带发现（不在票面，只记不做）**：`loading.rs` 有两份同形的 `attach` / `attach_plan` /
   `open_new` / `open_session`，`impl AssemblyKit` 里那一套是死代码；`HostEntry.cancel`
   （`host.rs:253`）在生产里没有读取点；跨进程的会话独占锁查不到（`store.rs` 无 flock，
   `EventLog::open` 直接 append）。这些归实现阶段或另一条线。

## 作答之后

- 产物表一（已按会话分开）与表二（全屏 / 全进程唯一）是 `/to-spec` 直接要读的两张底账。
- 「平铺集合」「pane 数上限」「焦点 pane」三个概念**在今天的代码里都没有对应类型或字段**。

---

**产物**：`.scratch/session-panes/research/01-session-infrastructure.md`，正文要落成两张表：
**「已按会话分开」** 与 **「全屏 / 全进程唯一」**，后者逐条给一句初判（「要改成每 pane 一份」还是
「只需要一个焦点投影」）。
