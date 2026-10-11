# 10 — `/` 菜单：宽度就是对话区那么宽

Type: implement
Status: ready-for-walkthrough
Part of: ../spec.md
Blocked by: —

> 规格：[`../spec.md`](../spec.md) §11（票 09 那一段的续写）。

## 目标

真机反馈（2026-10-10）：

> 优化下'/'的提示弹窗, 让它的宽度为整个对话区域的宽度

票 09 把上限从 72 抬到 96，宽度**按最宽的一条算**。这解决的是「描述掉字」，没解决「菜单与
输入框对不齐」：窄的那几条于是只有两三列宽，而它正浮在一行**满宽**的输入区上。

## 现状（2026-10-10 核实）

- `draw_menu`（`src/render/tui.rs`）算 `widest`（最宽那一条要占的列数），再 `min(MENU_MAX_WIDTH)`。
- `layout::Regions::menu` 把它夹在主列宽度内，横向锚在光标列上（`min(right - width)`）。
- `layout::MENU_MAX_WIDTH = 96` 只服务这一处。

## 具体行为

- **宽度 = 对话区宽度**：`width = panes.input.width`（输入区与主列同宽）。`widest` 与
  `MENU_MAX_WIDTH` 一起退场 —— 再没有「按内容定宽」这件事。
- 于是横向不再跟光标：`menu()` 里那个 `min(right - width)` 落到 `left`，菜单左边缘与主列
  左边缘对齐。纵向**照旧**跟着光标（它仍向上开，贴着输入区）。
- 一行怎么排版一个字不改：`menu_row` 仍按 `inner = width - 2` 截断，行尾那一列留白照旧。宽度
  变大只会让描述有更多地方放，`text_columns(&label) + gap + 2 <= inner` 这个「放不下就让名字
  独占一行」的判据自动更容易成立。
- `@` 菜单同一个盒子，一样受益。

## 测试

- `tests/render_layout.rs`：`the_menu_spans_the_conversation_column` —— 菜单框的左边与宽
  度分别等于输入区的（120 列终端带左栏、80 列无左栏各一次），且不越出主列右缘。
- `a_long_description_gets_the_wider_menu_before_it_is_cut`（票 09 那条）照旧成立：200 列
  终端上菜单宽 159，仍宽过旧上限。

## 不做什么

- 不改高度阶梯（仍一窗 12 行）、不改次序与过滤、不动 `Tab`/`Enter`/`Esc` 语义。
- 不给菜单加外边距或圆角 —— 要的是**对齐**，不是留白。

## 评论

- **落地（2026-10-10）**：宽度不再是画家传进去的一个数，而是布局自己的一个方法 ——
  `layout::Regions::menu_width()`（= `input.width`），`Regions::menu()` 不再接 width 参数。
  于是「阶梯只有一个家」这条仍然成立：绘制器只问「多宽」，不自己定。
- **退场的东西**：`layout::MENU_MAX_WIDTH`（96，连同它的注释）与 `draw_menu` 里那段
  `widest` 算式（连同它逐项列清的记账）。`MENU_MIN_WIDTH` 留着 —— 判据从「菜单该有多宽」变成
  「终端有多窄」，仍然值得有一个地板。
- **`menu()` 里的横向锚定整段删掉**：它算的是 `min(光标列 + anchor.column, 右缘 − 宽度)`，而宽度
  等于主列宽之后，那一串恒等于主列左缘。留着它只会让读的人以为框还跟着光标横向走。
- **被推翻的既有测试**：`the_menu_follows_the_cursor_column` 断言「再多打一个字，框跟着光标右移
  一列」—— 那正是这一票要拿掉的行为。改写成 `the_menu_holds_its_column_and_follows_the_cursor_down`：
  横向不动、宽度不变，纵向仍贴着光标（粘贴出一个第二行记号，框的下缘跟着下移一行）。
- **一条自己踩的坑**：新写的测试先按「40 个汉字 = 40 列」算描述的宽度，于是断言「描述整句都在」
  红了。真相是 **80 列** —— 汉字一列算两格。写这类断言前先问 `text_columns`，别数汉字。
- **实测**：`cargo test` **1773 passed / 0 failed**（其中 `render_layout` 376 条）；`cargo fmt
  --check` 干净；`cargo clippy --all-targets` 无新增（41 条 `collapsible_if` 与改动前逐条相同）；
  `scripts/check-doc-size.py` OK。
- **留给真机**：走查清单 ⑪ 第 9 条（左右对齐 + 纵向跟随）。这条没法在 `TestBackend` 里验「看起来
  对不齐」—— 那要一双眼睛。