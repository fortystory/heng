# 21: 输入区草稿与状态行全局读数

Type: implement
Status: ready-for-agent
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

- [ ] A 打一半字切到 B：输入区显示 B 的草稿；切回 A 复原（pty）。
- [ ] 一个会话在跑、一个在等人类时，状态行读数是「另有 1 个在跑 · 1 个等人类」且不含焦点自己（帧）。
- [ ] `Ctrl-C` 只停焦点格那一个动作，另一个继续跑（pty）。
- [ ] 切焦点后终端标题换成焦点会话的标题。
- [ ] 平铺态输入区只有一份，宽度跨 pane 区。

## 依赖

[19 集合生灭与满员拒绝](19-tiles-lifecycle-and-limit.md)。决定见
[`issues/06-grilling-single-instance-surfaces.md`](06-grilling-single-instance-surfaces.md)。
