# 11 — `/` 菜单：去掉左右两条竖线

Type: implement
Status: ready-for-walkthrough
Part of: ../spec.md
Blocked by: —

> 规格：[`../spec.md`](../spec.md) §11（票 10 的续写）。

## 目标

真机反馈（2026-10-10，紧接票 10）：

> 把左右边框也去掉吧

票 10 让菜单与对话区同宽，于是那两条竖线（各占一列）成了整宽里唯一还对不齐的两列：框的左缘比
文字起的那一列右一格，右缘则压在主列最右一列上。

## 现状（2026-10-10 核实）

- `chrome_block()` 是**四个浮层表面共用**的框（问卷覆盖层、菜单、详情覆盖层、文件查看器），
  `Borders::ALL` + 四个空角。
- `draw_menu` 画的是 `chrome_block()`，内容画在 `layout::inner(area)`（四边各内缩一格）。
- `layout::Regions::menu()` 的高度是 `rows + BORDER_COLUMNS`，其中 `BORDER_COLUMNS = 2` —— 那
  2 是**行**（上下两条线），不是列。

## 具体行为

- **菜单自己的框**：`Borders::TOP | Borders::BOTTOM`，仍是 `border::LIGHT_TRIPLE_DASHED` 的
  `┄` 与 [`palette::CHROME`]。上下两条线横跨整个对话区，右端顶到主列右缘。
- **内容可用宽度就是对话区宽度**：左右不再有边框列，于是 `inner` 从 `width - 2` 变成 `width`。
- **左右那两列「不碰边框」的留白一起退场**：它们本来就是边框的一部分 —— 留着它，候选名会比
  转录正文右移一列，而这一票要消掉的正是那点错位。于是名字从主列左缘那一列起、铺到右缘那一列
  止，与转录正文、与输入区严格同一列。
- **高度一行不改**：上下两条线仍各占一行，`rows + 2`。
- **另外三个浮层不动**：`chrome_block()` 原样留给问卷与详情覆盖层。

## 测试

- `tests/render_layout.rs`：
  - `menu_frame` 这条 helper 不再靠 needle 定位，改成认「成对虚线」—— 少了竖线之后角也没了，
    `overlay_frame` 那套「两端是空角」的判据对菜单不再成立。菜单开没开的判据随之从
    `contains("┆ /undo")` 换成它在不在。
  - `the_menu_keeps_its_corners_over_text_that_is_not_ascii`（票 09 那条）改成
    `the_menu_keeps_its_dashed_edges_over_text_that_is_not_ascii`：内容行左右两端都是**文字或
  空白**、上下两行是完整虚线，一个字形都不许掉。
  - 新增 `the_menu_draws_no_vertical_border_anywhere`：整块区域逐格扫，没有一格 `┆`。

## 不做什么

- 不动问卷、详情覆盖层、文件查看器的框。
- 不动高度阶梯、次序、过滤与键位语义。

## 评论

- **落地（2026-10-10）**：菜单有了自己的框函数 `menu_block()`（`Borders::TOP | BOTTOM` 加
  `border::Set { horizontal_top: "┄", horizontal_bottom: "┄", ..border::EMPTY }`）。字段名踩过一次：
  `border::Set` 里那两条横线叫 `horizontal_top` / `horizontal_bottom`，不是 `top` / `bottom`。
  `chrome_block()` 一个字没动，问卷与详情覆盖层照旧四边俱全。
- **渲染那一块不再用 `layout::inner(area)`**：它四边各缩一格，而菜单两侧没有边 —— 现在是
  `Rect::new(area.x, area.y + 1, area.width, height - 2)`，左右铺满。
- **`MENU_CHROME_ROWS`（新）**：菜单那两条边各占一行。它曾经借 `BORDER_COLUMNS`，而那个常数说的
  是**列**（四边各一列）—— 行数与列数碰巧都是 2，借用会把两笔账混成一处。
- **比票里原定的多做了一件**：`menu_row` 左右各留的那一列「不碰边框」空白也去掉了。看渲染结果时
  才发现 —— 竖线没了而留白还在，候选名于是比转录正文右移一列，那正是这一票要消掉的错位。
- **测试基建换掉两样**：一是 `menu_box` 改成 `menu_frame`（竖线与角都没了，改成认「needle 上下两条
  同起同止的虚线」，并排除 needle 落在输入区第一行那种情形）；二是那些 `contains("┆ /undo")` 的
  「菜单开没开」判据 —— 菜单行不再有那个 `┆`，改成读几何的 `menu_is_open`。判据从**文本**换成
  **几何**之后顺带修掉一个隐患：草稿里也有 `/undo`，从前那条断言分不清「菜单里的 /undo」与
  「草稿里的 /undo」。
- **实测**：`cargo test` **1774 passed / 0 failed**（`render_layout` 378 条）；`cargo fmt --check`
  干净；`cargo clippy --all-targets` 无新增（41 条 `collapsible_if`，与改动前逐条相同）；
  `check-doc-size` / `check-language` / `lifecycle-check` 全绿。
- **留给真机**：走查清单 ⑪ 第 6 条改写，并新增第 10 条。左右两侧没有竖线之后，菜单与转录只剩
  上下那两条虚线分隔 —— 那个观感要人看。
