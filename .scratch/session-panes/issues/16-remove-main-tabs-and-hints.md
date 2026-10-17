# 16: 删主列页签条与提示行改造

Type: implement
Status: ready-for-agent
Part of: ../map.md
Blocked by: 15

**要建的：** 主列只剩**对话视图**，页签条那条线整条拆掉；提示行改成两套按「键盘在谁手里」分。

- 删：`MainTab`、`main_tab` 字段、`SwitchMainTab` 命中、`set_main_tab` / `switch_main_tab`、
  `draw_main_tab_bar`、`Regions.main_tabs`、`MAIN_TABS_ROWS`；滚轮 / 翻页 / 贴底里按 `main_tab`
  分派的分支回落到对话视图。
- 外壳开销不变：顶部条 2 行取代主列页签条的 2 行。
- `KEY_HINTS`：`ctrl-v 轨迹` → **`ctrl-v 右栏`**；`ctrl-o 左栏` 前插 **`ctrl-r 平铺`**（共 9 条）。
- `TRACE_HINTS` → 改名 **`RAIL_HINTS`**：条目照旧，`ctrl-v 对话` → **`ctrl-v 交还`**。
- 判据从「页」改成**「键盘在谁手里」**：右栏拿键盘时用 `RAIL_HINTS`，否则 `KEY_HINTS`。

## 验收

- [ ] 主列顶部不再有页签条；对话视图占满主列。
- [ ] 120 列提示行显示 9 条 `KEY_HINTS`；窄档按优先级从尾部丢。
- [ ] 右栏拿住键盘时提示行换成 `RAIL_HINTS`，末条是 `ctrl-v 交还`。
- [ ] `Shift+Tab` / 滚轮 / 翻页在只剩对话视图后照旧可用。
- [ ] 至少 2 条帧断言 + 1 条 pty 提示行断言；既有「主列两页」测试按新形状改写。

## 依赖

[15 顶部页签条](15-top-tab-strip.md)。决定见
[`issues/12-grilling-main-column-after-move.md`](12-grilling-main-column-after-move.md)。
