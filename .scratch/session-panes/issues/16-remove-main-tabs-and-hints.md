# 16: 删主列页签条与提示行改造

Type: implement
Status: done
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

- [x] 主列顶部不再有页签条；对话视图占满主列。
- [x] 120 列提示行显示 9 条 `KEY_HINTS`；窄档按优先级从尾部丢。
- [x] 右栏拿住键盘时提示行换成 `RAIL_HINTS`，末条是 `ctrl-v 交还`。
- [x] `Shift+Tab` / 滚轮 / 翻页在只剩对话视图后照旧可用。
- [x] 至少 2 条帧断言 + 1 条 pty 提示行断言；既有「主列两页」测试按新形状改写。

## 依赖

[15 顶部页签条](15-top-tab-strip.md)。决定见
[`issues/12-grilling-main-column-after-move.md`](12-grilling-main-column-after-move.md)。

## 落地注记（2026-10-17）

- **删干净**：`MainTab` / `main_tab` / `HitAction::SwitchMainTab` / `set_main_tab` /
  `draw_main_tab_bar` / `layout::Regions.main_tabs` / `MAIN_TABS_ROWS` 全条移除；滚轮、
  翻页、贴底回落到对话视图。`CHROME` 从 6 落到 4（顶部页签条在主列之外，接替了那两行），
  转录因此上移两行、多两行。
- **主列轨迹几何也删了**：`Regions.trace` / `band` / `band_kind` / `trace_text` /
  `trace_scrollbar` 与主列轨迹页一起离开；轨迹唯一的家是右栏（票 13 的 `right_*`）。
- **提示行两套按键盘归属**：`KEY_HINTS` 9 条（`ctrl-v 右栏`、`ctrl-r 平铺` 插在末两条），
  `TRACE_HINTS` 改名 `RAIL_HINTS`、末条 `ctrl-v 交还`；判据是 `right_wanted && trace_keyboard`。
  宽度阈值实测更新：整份 `KEY_HINTS` 要 141 列提示行（182 列终端），`RAIL_HINTS` 137 列。
- **测试**：删掉 60+ 条钉在「主列页签条 + 77 列轨迹」旧形状上的断言（它们测的形态已被票
  13/16 推翻，右栏形态由票 13 的六条与票 14 的四条覆盖），其余帧断言整体重取基线；新增
  3 条（主列无页签条、提示行随键盘换表、翻页键仍可用）。
