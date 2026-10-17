# 20: 问卷与权限门每会话

Type: implement
Status: ready-for-agent
Part of: ../map.md
Blocked by: 19

**要建的：** 问卷与权限门从「全屏一个」改成**每会话一份**，全屏只画焦点那份。

- `pending`（问卷 / 权限门）从单例改成按 `SessionId` 的表，**每份自带自己的 reply 通道**
  （reply 唯一这件事不变）。
- 全屏同一时刻只显示**焦点会话**的那一份；**接管期间可以切焦点，题面跟着焦点走**。
- 其余等待中的会话由状态行的全局读数点名（票 21）。

## 验收

- [ ] 两个格各自在等人类时，切焦点 → 画的是焦点那一格的题面（帧）。
- [ ] 在 A 的题上作答只回到 A；B 的问题不受影响（pty）。
- [ ] 切走再切回 A，A 的问题与已填的作答还在。
- [ ] 问卷立着时 `Ctrl-C` / `Esc` 的既有语义不变。

## 依赖

[19 集合生灭与满员拒绝](19-tiles-lifecycle-and-limit.md)。决定见
[`issues/06-grilling-single-instance-surfaces.md`](06-grilling-single-instance-surfaces.md)。
