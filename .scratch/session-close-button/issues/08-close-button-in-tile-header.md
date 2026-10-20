# 08: 平铺态格标题行的关闭标记

Type: implement
Status: ready-for-agent
Part of: ../map.md
Blocked by: 05, 06

**要建的：** 平铺态下把指针移到某一格的标题行，那一格出现一个红色 `✖`；点它关掉那一会话。

## 验收

- [ ] 悬浮某格 → 格内第 2 列出现 `✖`，标题从第 4 列起、预算 = 格宽 − 4 **不变**（挤 0 列）。
- [ ] 点标记 → 发出那一会话的关闭请求；目标是焦点格时按
  [06 关掉前台那一会话后，前台落到相邻](06-close-foreground-and-land-on-neighbor.md) 的规则先切后关。
- [ ] 焦点标记 `◆` / `·` 退场 —— 焦点格靠 ACCENT + 粗体认。
- [ ] 关掉焦点格 → 焦点格落到相邻格、其余格重排；只剩一格时自动回标签态。
- [ ] 窄档（格宽 32）一帧断言；标题的截断宽不变。
- [ ] 悬浮与不悬浮两态各一帧断言；点标记发对会话 id。

## 依赖

[05 左栏「会话」页的关闭标记](05-close-button-in-sessions-page.md)、
[06 关掉前台那一会话后，前台落到相邻](06-close-foreground-and-land-on-neighbor.md)。决定见
[`../spec.md`](../spec.md) 的「A 关闭按钮与三处落点」与
[三处落点的关闭按钮长什么样](02-prototype-close-button-shapes.md)（平铺那两帧）。
