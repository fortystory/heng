# 14: 右栏开关与键盘层序

Type: implement
Status: done
Part of: ../map.md
Blocked by: 13

**要建的：** 右栏可以开、关，并接住键盘。

- **`Ctrl-V` 改成右栏开关**：一次按下 = 开栏 + 把键盘交给右栏；再按 = 交还键盘 + 关栏。
- **点输入区 = 交还键盘但右栏留着**。
- 右栏里照轨迹既有层序：点右栏任何位置拿键盘，`Esc` 依次退「搜索 → 过滤 → 选中 → 交还」。
- 屏宽 < 80（右栏整栏隐藏）时 `Ctrl-V` 给一句回执；提示行里右栏那一段不画。

## 验收

- [x] pty：`Ctrl-V` 两次往返（开栏并拿键盘 → 交还并关栏）。
- [x] pty：点输入区后键盘回输入区，右栏仍然画着。
- [x] pty：右栏里 `Esc` 依次退三层，最后把键盘交还输入区。
- [x] 右栏隐藏时按 `Ctrl-V` 出一句回执，不改变布局。
- [x] `Ctrl-V` 不再触发主列两页的切换（那两页此时还在，由票 16 删）。
- [x] 既有 TUI 测试全绿。

## 依赖

[13 右栏容器与轨迹搬家](13-right-rail-and-trace.md)。决定见
[`issues/08-grilling-right-rail-keyboard.md`](08-grilling-right-rail-keyboard.md)。

## 落地注记（2026-10-17）

- **落点是帧断言，不是 pty**：仓库没有 Rust 侧的 pty 装置（`scripts/tui-startup-check.py`
  只冒烟启动），这一票的四条验收由 `tests/render_layout.rs` 的帧 / 按键 / 指针断言覆盖
  （光标位置就是「键盘在谁手里」的外部可见判据）。
- **`Ctrl-V`**：`toggle_right_rail` —— 关栏顺手交还键盘；开栏前先问排版放不放得下
  （< 80 列或主列会跌破 40），放不下只回一句 `右栏放不下`。
- **键盘判据**：`keyboard_in_the_input` 补上「右栏拿住键盘」这一项，顺手补齐研究票 02
  报的两个漏项（选择器、历史重放立着时光标照旧露在输入区）。
- **过渡期**：主列轨迹页还在（票 16 删），它拿键盘的老路径与右栏共用 `trace_keyboard`，
  判据是 `(right_wanted || main_tab == Trace) && trace_keyboard`。
- **被推翻的测试**：`ctrl_v_flips_the_main_tab_both_ways` 删除；`the_trace_page_hints_at_its_own_keys`
  里「Ctrl-V 切回对话页」改成点主列页签 —— 提示行判据本身由票 16 改。
