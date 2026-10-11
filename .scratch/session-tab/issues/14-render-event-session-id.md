# 14: 渲染事件带 session id 与重放语义修正

Type: implement
Status: done
Blocked by: —
Part of: ../map.md

**要打通的行为**：渲染通道上的每一条事件都说得出它属于哪一场会话；重放期间进来的实时事件**不丢也不重**。单会话下行为与今天一致。

**来源**：[spec.md](../spec.md) §6 与 §10。

## 验收

- [ ] `RenderEvent` 的每个变体（`Delta` / `Logged` / `Diagnostic` / `WorkspaceChanged` / `Notice` / `Identity`）都带上 session id
- [ ] 渲染器**只 apply「当前前台」的 `Logged`**；别的会话的事件只用来更新状态读数与标记
- [ ] 重放期间 `live_event` **不再丢弃 `Logged`**，改为一并压进 `live_buffer`（现状是直接丢，重建期间的内容会静默消失）
- [ ] 重放收尾排空 `live_buffer` 时**只 apply `seq > S`**（`S` = 重放快照的最大 `seq`）—— 不重不漏
- [ ] 收到新的重放请求时**先把在飞的旧重放收尾**（分隔行 + 排空它的 buffer），再开新的 —— 现状是直接覆盖，旧批次的缓冲会泄到下一次
- [ ] `tests/history_replay.rs` 全绿；新增「重放期间来的实时事件在收尾时补上、且不重复」的用例

## 实现记录

- `RenderEvent` 六个变体全都带上 `session: SessionId`，外加一个 `session()` 访问器（每个
  变体各自 `..` 之外还得有一个问得出口的地方）。`Logged` 从元组变体改成结构变体
  —— 它原来是 `Logged(Event)`，多一个身份只能这么长。
- `RenderHandle` 持这个身份（`channel(session)` 造它），于是 `logged` / `text_delta` /
  `diagnostic` / `notice` / `identity` / `workspace_changed` 六个出口自动带上；三个渲染器
  里只有 plain 需要自己存一份（它会在 `Lagged` 时自己造一条诊断）。`Renderer::spawn` 因此
  多接一个 `SessionId`。
- `TuiState` 多了 `front: SessionId`（进程级），初值取 `SessionFacts::session_id`；`live_event`
  按它分派：不是前台的事件直接不进这份投影。**状态读数与标记留给票 22** —— 这一票只钉住
  「进不进投影」。
- `live_event` 在重放期间**不再丢弃**进流事件：全部压进 `live_buffer`。`Replay` 多一个
  `max_seq`（= 快照最大 `seq`，记作 `S`），收尾排空 buffer 时只放 `seq > S` —— 快照已经
  持有的那条不会画第二遍（既有用例 `a_logged_event_arriving_mid_replay_is_not_painted_twice`
  正是这条判据）。
- 新的 `ConsoleRequest::Replay` 先把在飞的旧重建 `take` 出来收尾（分隔行 + 排空它的 buffer +
  视口回末尾）再开新的；直接覆盖会把旧批次的缓冲泄到下一次。
- 新增三条用例（`tests/history_replay.rs`）：不重不漏、别的会话进不来、新重建先收尾旧的。
  把两处判据临时短路跑一遍，三条各自失败过。
