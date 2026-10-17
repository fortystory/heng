# 17: 平铺两态与几何

Type: implement
Status: ready-for-agent
Part of: ../map.md
Blocked by: 16

**要建的：** `Ctrl-R`（或顶部条的平铺按钮）在**标签态**与**平铺态**之间切；平铺按固定排法画格。

- 排法：1 独占 / 2 左右 / 3 L 形 / 4 上下各二。
- **几何不够自动回标签态**：任一格宽度 < 32（平铺态下限）或高度不足；3 / 4 格需要屏高 ≥ 30。
- **平铺态左栏自动收起**；按 `Ctrl-O` 给一句回执（不画左栏）。
- **平铺态右栏照开**，pane 区宽按「≥ 32 × 列数 + 间隔」算：94 列扣右栏 29 → 主列 65 → 2 格各 32。
- 格与格之间一条分隔；焦点格标题前 `◆`、非焦点格 `·`。

## 验收

- [ ] 120×40 的 1 / 2 / 3 / 4 格帧各一，格尺寸符合排法。
- [ ] 94×40 帧：右栏照开、2 格各 32；80×24 帧：回标签态。
- [ ] 平铺态左栏不画；按 `Ctrl-O` 出回执且布局不变。
- [ ] 任一格低于 32 列或高度不足时自动回标签态（帧）。
- [ ] 至少 6 条新的帧断言，`cargo test` 全绿。

## 依赖

[16 删主列页签条与提示行改造](16-remove-main-tabs-and-hints.md)。决定见
[`issues/05-prototype-panes-layout.md`](05-prototype-panes-layout.md) 与
[`issues/08-grilling-right-rail-keyboard.md`](08-grilling-right-rail-keyboard.md)。
