# 21: 输入区草稿与状态行全局读数

Type: implement
Status: done
Part of: ../map.md
Blocked by: 19

**要建的：** 底部三行里的输入区与状态行按平铺语义工作。

- **输入区草稿与历史每会话各存**（沿用既有的 stash / load），切焦点只换显示；平铺态输入区
  仍然只有一份、绑焦点格。
- **状态行全局一份、读焦点会话**，并加一段全局读数：**「另有 N 个在跑 / M 个等人类」**
  （只算焦点之外的那些）。
- **`Ctrl-C` 只取消焦点格的当前动作**；退出举手仍是进程级一个。
- **终端标题写焦点会话**。

## 验收

- [x] A 打一半字切到 B：输入区显示 B 的草稿；切回 A 复原（pty）。
- [x] 一个会话在跑、一个在等人类时，状态行读数是「另有 1 个在跑 · 1 个等人类」且不含焦点自己（帧）。
- [x] `Ctrl-C` 只停焦点格那一个动作，另一个继续跑（pty）。
- [x] 切焦点后终端标题换成焦点会话的标题。
- [x] 平铺态输入区只有一份，宽度跨 pane 区。

## 依赖

[19 集合生灭与满员拒绝](19-tiles-lifecycle-and-limit.md)。决定见
[`issues/06-grilling-single-instance-surfaces.md`](06-grilling-single-instance-surfaces.md)。

## 落地注记（2026-10-17）

- **草稿与历史**：票 08 的 `SessionLight::editor` 已经在做「每会话一份、切走 stash / 切回
  load」，本票没再动它；`tests/session_switch.rs` 的 `the_draft_comes_back_with_its_session`
  就是这条验收。
- **状态行全局读数**：`wording::background_readout(running, waiting)` 生成
  `另有 N 个在跑 · M 个等人类`（两样都有时第二段省「另有」）；数据是 `SessionState` 那张
  每会话表，排除焦点自己。读数**优先占住行尾**：有它时状态行按剩余宽度降档（模型那一段可能
  先让位），因为它是唯一说得出屏幕外动静的地方；没有后台会话时状态行一个字不变。
- **`Ctrl-C` 与终端标题**：都读前台指针（既有行为），本票只确认没有第二套来源。
- **平铺态底部**：输入区 / 状态行 / 提示行仍是全局一份、横跨 pane 区（由 `plan` 的 `main`
  宽度给出），测试钉住横线与提示行。
- 测试：`render_layout` 2 条（读数只算焦点之外、平铺底部一份），另修了 `trace_keyboard`
  的初始值（改 false，右栏默认开后键盘仍归输入区，`Ctrl-V` 才交过去）。
