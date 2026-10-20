# 12: 测试辅助收拢进 `tests/common/`

Type: implement
Status: done
Part of: ../map.md
Blocked by: 11

**要建的：** `tests/render_layout.rs` 里还留着一份自己的 `facts` / `screen` / `row_text` /
`buffer` / `find_cell` / `click` —— 与 `tests/common/mod.rs` 里的那份重复。把它也换成共用模块，
一个造状态、画一帧、读屏、合成指针的写法只留一处。

## 验收

- [x] `tests/render_layout.rs` 不再自带那份辅助，改用 `tests/common/`；文件里其它测试一个字
  不用改（除非某个辅助两边签名不同 —— 那时以 `common` 那份为准，并记下改动）。
- [x] `cargo test` 全绿，用例数与收拢前相同（不因收拢丢用例）。
- [x] `tests/common/mod.rs` 的模块文档与实际内容一致（它现在不做键盘事件 —— 那条文档已经
  改过；若最后还是不做，就别再写「键盘」二字）。

## 依赖

[11 词条、文档与被推翻条款的改写](11-terms-docs-and-overturned-clauses.md)（收尾票：文档与代码都
定下来之后再整理）。这票是票 `05` 落地时**浮现出来**的一张 —— 抽取共用辅助时只收了
`tests/session_page.rs` 与 `tests/pointer_hover.rs`，`tests/render_layout.rs` 那份太大，没动。

## 落地注记（2026-10-20）

- **收进 `tests/common/` 的三个**：`buffer` / `row_text`（两边一模一样）与 `screen`。
- **`screen` 两边签名不同**（这边返回 `Vec<String>`、那边返回一整段 `String`）—— 按票面「以 `common`
  那份为准」，`common` 里**加**了一个 `screen_lines()`（逐行那一份），`common::screen()` 由它拼出。
  `tests/render_layout.rs` 里改用 `screen_lines`，那 24 处调用与它的注释一并跟上。
- **留在原地的**：`facts`（这一份带 `tool_schemas` 与 `budget_limit`，与 `common` 那份**语义不同** ——
  换成通用的会改掉详情覆盖层那一面的行为）、`tool_schemas` / `state` / `facts_with_roster` / `idle` /
  `framed` / `screen_text` / `menu_is_open` / `cells` / `transcript_rows` / `next_face` —— 它们是那个
  文件专有的探针，不是「造状态、画一帧、读屏」这条主线。
- **审查后补收的两个**：`click`（两边一模一样：按下 + 抬起）与 `find_cell` —— 后者**语义不同**：
  `common` 那份找的是**子串**（`needle` 多长都行），这一份找的是**一个字符**。于是 `common` 里另起
  一个 `find_cell_char`（单字符那一档），这一份改用它 —— 两份都在，不再同名不同义。
- **还留在原地的三个**：`press` / `release` / `drag_to` —— 这一份是**造事件**（返回
  `MouseEvent`），`common` 那三个是**直接操作 state**（`press(state, …)`）：不是一份东西。
  `common` 那三个是票 10 为了「焦点变化 / 拖选不选中标记」加的。
- **用例数不变**：`tests/render_layout.rs` 收拢前后都是 308 条，全量 1897 条一条不少。
- **`row_text` 两边等价**：`common` 那份按**显示列宽**推进（宽字符跳过一格），这一份原来经 `cells`
  逐格读（宽字符的第二格是空串）—— 结果相同（那一格只是不 push 而已）。
- **模块文档**与实际内容一致：它不做键盘事件（那句早已改掉）。
