# 14: 渲染事件带 session id 与重放语义修正

Type: implement
Status: ready-for-agent
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
