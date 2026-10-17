# 18: 平铺集合与焦点手势

Type: implement
Status: ready-for-agent
Part of: ../map.md
Blocked by: 17

**要建的：** 平铺集合能加、能移、能换焦点。

- 集合是 `Vec<SessionId>`：有序、按加入顺序、**不允许重复**。
- **`[+ 加入]` 格参与布局**：集合 n 个时按 n + 1 格排；点它打开会话选择器（复用 picker 通道）。
- **移出**：焦点格标题行的 `×`。
- **换焦点**：`Ctrl-1`…`Ctrl-4` 直接跳第 N 格；鼠标点格子。
- 三条边界：**集合为空**时进平铺自动把焦点加入；**集合非空而焦点不在里面**时拒绝进平铺并提示；
  **只剩一个**时自动回标签态。

## 验收

- [ ] 点 `[+ 加入]` 选一个会话后格数 +1、焦点不变；集合 4 个时该格不画。
- [ ] 点焦点格 `×` 后格数 -1、焦点给集合里的下一个。
- [ ] `Ctrl-1…4` 后焦点格的 `◆` 移到第 N 格（帧）。
- [ ] 空集合时按 `Ctrl-R` 自动加入焦点进平铺（pty）。
- [ ] 焦点不在集合时按 `Ctrl-R` 被拒绝并提示（pty）。
- [ ] 移出到只剩一个时自动回标签态（pty）。

## 依赖

[17 平铺两态与几何](17-tiles-two-modes-and-geometry.md)。决定见
[`issues/07-grilling-pane-gestures-and-focus.md`](07-grilling-pane-gestures-and-focus.md)。
