# 20: 问卷与权限门每会话

Type: implement
Status: done
Part of: ../map.md
Blocked by: 19

**要建的：** 问卷与权限门从「全屏一个」改成**每会话一份**，全屏只画焦点那份。

- `pending`（问卷 / 权限门）从单例改成按 `SessionId` 的表，**每份自带自己的 reply 通道**
  （reply 唯一这件事不变）。
- 全屏同一时刻只显示**焦点会话**的那一份；**接管期间可以切焦点，题面跟着焦点走**。
- 其余等待中的会话由状态行的全局读数点名（票 21）。

## 验收

- [x] 两个格各自在等人类时，切焦点 → 画的是焦点那一格的题面（帧）。
- [x] 在 A 的题上作答只回到 A；B 的问题不受影响（pty）。
- [x] 切走再切回 A，A 的问题与已填的作答还在。
- [x] 问卷立着时 `Ctrl-C` / `Esc` 的既有语义不变。

## 依赖

[19 集合生灭与满员拒绝](19-tiles-lifecycle-and-limit.md)。决定见
[`issues/06-grilling-single-instance-surfaces.md`](06-grilling-single-instance-surfaces.md)。

## 落地注记（2026-10-17）

- **`pending` 跟着会话住进 `SessionLight`**：切走时那一份（连它自带的 reply 通道）收进轻状态
  表，切回原样取回 —— 于是「每会话一份」成立，而全屏同一时刻仍只有焦点那一份。
  `questionnaire_bottom` 每帧按当前 pending 重算，换会话时置空。
- 「另一个会话在等」由**状态行全局读数**点名（票 21 的「另有 N 个等人类」）：`AwaitingHuman`
  由 asker 那一层置上（`.scratch/session-panes/issues/22` 的依赖），渲染端不另造一套。
- 作答只回到原会话：reply 通道随 `Pending` 一起搬，测试里两个 oneshot 分别收到/收不到答案。
- `Ctrl-C` / `Esc` 的语义没动（它们读焦点那一份 pending，既有问卷测试覆盖）。
- 测试：`render_layout` 的 `each_session_keeps_its_own_pending_question`（两格各一份题面、切回
  原样、作答只回原会话、另一个的通道不受影响）。
