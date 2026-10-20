# 01：文件链接上链接色，行内代码保持灰

Type: implement
Status: done
Blocked by: —
Part of: ../spec.md

> 规格：[`.scratch/link-emphasis/spec.md`](../spec.md) §1、§2。
> 本票**只管颜色**：认候选、画下划线、点开那些是既有行为（[`clickable-links`](../clickable-links/spec.md)），
> 本票一个字不动。点开走文件查看器是 [票 02](02-open-in-file-viewer.md) 的事。

## 目标

- `src/render/palette.rs` 的**内容域**新加一档 `LINK = Color::LightCyan`（名字与理由按 spec §2 写）。
- `src/render/links.rs`：候选是 `Target::Path` 且**它所在 span 没有前景色**时，那几列的前景色改成
  `LINK`；下划线照旧（今天的候选一律带下划线，一字不动）。
- `src/render/markdown.rs`：Markdown 链接的**目标是文件路径**时，**标签与目标回显**（` (seed.md)`）
  都上 `LINK`；标签是行内代码时**也**上。目标是 URL 的链接**不上**。

## 现状（开工前先复核）

- 候选识别与下划线：[`src/render/links.rs`](../../../src/render/links.rs) 的 `Target` / `hotspots()` /
  `mark()` / `underline()`；`underline()` 逐字符重建 span，今天只 `patch(Modifier::UNDERLINED)`。
- 内容域色板：[`src/render/palette.rs`](../../../src/render/palette.rs) 的「内容域」一节；行内代码是
  `Event::Code` → `fg(CODE_QUIET)`（[`src/render/markdown.rs`](../../../src/render/markdown.rs)）。
- Markdown 链接的画法：`Tag::Link` → `open_frame()`（压 `Modifier::UNDERLINED`），
  `TagEnd::Link` → `close_link()`（标签之后追加灰色的 ` (url)` 回显）。
- 链接层只在**对话视图**跑：`src/render/tui.rs` 的 `draw_transcript` 与 `draw_focused_tile_body`
  两处 `links::mark(&mut rows, &folded)`。

## 具体行为

1. **色板**：`pub const LINK: Color = Color::LightCyan;`，放在内容域那几档旁边，注释写清它回答
   「这是文件链接」、以及与 `DEBATERS[0]` 撞值是刻意的。
2. **链接层上色**：`underline()` 改成能同时改前景色 —— 候选是 `Target::Path`、且候选那几个字符当前
   **没有前景色**（`style.fg` 是 `None` 或 `Some(Color::Reset)`）时，前景色改成 `LINK`；
   有前景色（行内代码 `CODE_QUIET`、引用块的叙述灰、语法高亮的代码块）时**一个字不动**。
   URL 候选（`Target::Url`）**不上色**，只保留今天的下划线。
3. **Markdown 链接上色**：`open_frame()` 记下目标时，顺手判一次「这是不是一个文件路径」
   （复用 `links.rs` 的判据，不要在 markdown.rs 里另写一套路径形状的规则 —— 需要在 `links.rs`
   暴露一个公开的判据函数）。目标是文件时：`close_link()` 给**标签那几个 span** 打上 `LINK`，
   追加的**目标回显**也用 `LINK`（今天是 `CODE_QUIET`）。目标是 URL 时两者都照旧。
4. **行内代码不上色**：独立的行内代码仍是 `CODE_QUIET`；只有「行内代码在文件链接的标签里」那一处
   被染成 `LINK`（spec §1）。

## 验收

- 单元测试（放在改动的那几个模块里，`#[cfg(test)]`）：
  - `links.rs`：正文里的一条路径候选 → 前景色是 `LINK`；同一段文字作为行内代码时 → 仍是
    `CODE_QUIET`；一条 URL 候选 → 前景色不动。
  - `markdown.rs`：`[seed.md](seed.md)` → 标签与 ` (seed.md)` 回显都是 `LINK`；
    `[x](https://example.com)` → 标签与回显都不是 `LINK`；`` `seed.md` `` → `CODE_QUIET`。
- `cargo test` 全绿；`cargo clippy --all-targets` 无警告；`cargo fmt --check` 干净。
- 手工看一屏：对话里一行正文引用了 `README.md`（青），同一行里有一处 `` `cargo test` ``（灰），
  两者一眼分得开。

## 评论

- **落地**：三个源文件加一个新测试文件。
  - `src/render/palette.rs`：内容域新加 `LINK = Color::LightCyan`。它回答「这是什么」（一条指向
    工作区文件的链接），所以住内容域；与界面域 `DEBATERS[0]` 撞值是**刻意的**（域间允许撞值）——
    链接另有一层下划线，两层不打架。
  - `src/render/links.rs`：公开判据 `is_file_path()`（内部仍是 `looks_like_path` 那一族规则，一个字
    没改）；`underline()` 多收一个 `link` 标志 —— 候选是 `Target::Path`、且它所在 span **没有前景色**
    （`None` 或 `Color::Reset`）时前景色升到 `LINK`；有前景色（行内代码 `CODE_QUIET`、引用块的叙述
    灰、代码块的语法色）**一个字不动**；`Target::Url` 不上色，下划线照旧。上色与下划线**在同一次
    逐字符重建里**做 —— 没新建第二个重建循环。
  - `src/render/markdown.rs`：`Frame` 加 `is_link` 一项（图片是假、链接是真）；目标是**文件路径**时
    标签与目标回显都上 `LINK`（标签是行内代码时**也**上），目标是 **URL** 时两者照旧（回显仍是
    `CODE_QUIET`）；图片的 alt 不染链接色。
  - `tests/link_colour.rs`（新建）：从公开的那几个接缝上断言整件事。
- **两处实现上的选择**：
  - **Markdown 那边是「写下的时候」染，不是「收尾的时候」染**：`style()` 与行内代码的取色都问一句
    「现在是不是在一段文件链接里」（`in_file_link()`，看**最里层**那一帧）。于是图片的 alt 天然不
    染 —— 写它的时候最里层是图片帧，不是链接帧；也不用在收尾时回头去改已经写好的那几片 span。
  - **「这是不是一个文件路径」只有一份判据**：`links.rs` 公开 `is_file_path()`，`markdown.rs` 调它，
    没有另写一套路径形状的规则（spec §1）。
- **spec §1 那张表在代码里长什么样**：前景色是 `None` 或 `Color::Reset` 算**正文**（上链接色），其余
  一律**保持原样**。于是用户发言与工具输出里的一条路径也是青的，而行内代码、引用块、代码块里的
  路径一律还是灰的 / 语法色。
- **验收**（第一轮）：
  - `cargo test`：**1924 passed / 3 failed**。那三条 **failed 都不是这一票造成的**，来历见下。
  - `cargo clippy --all-targets`：这一票的三个文件**零告警**（当时全仓唯一一条告警在
    `src/render/tui.rs` —— [票 02](02-open-in-file-viewer.md) 正在改的那一处；现已没有）。
  - `cargo fmt --check`：这一票的四个文件干净（全仓剩下的 diff 同上，也在 `tui.rs`）。
  - `python3 scripts/check-language.py`：OK。
  - 新测试 **18 条**：`links.rs` 三条（正文里的路径候选是 `LINK`；同一段文字作为行内代码时仍是
    `CODE_QUIET`；一条 URL 候选前景色不动）、`markdown.rs` 五条（文件目标的链接：标签上 `LINK`、
    目标回显上 `LINK`；URL 目标：标签与回显都不是 `LINK`；独立的行内代码仍是 `CODE_QUIET`；图片的
    alt 不染）、`palette.rs` 一条（`LINK` 与 `CODE_QUIET` 不撞值 —— 那一档新颜色存在的全部理由就是
    这个）、`tests/link_colour.rs` 九条（色板的值与区分、公开判据、正文 / 行内代码 / URL 三种候选、
    Markdown 链接的文件目标与 URL 目标、独立行内代码，外加一条**帧级**的：真画一帧对话视图，从屏幕
    缓冲里读那一格的前景色 —— `README.md` 是青的、同一行的 `cargo test` 是灰的）。
  - 那 18 条**逐条验过会红**：把实现分别破坏（`is_body` 放宽成永远为真、`link` 放宽成永远为真、
    `in_file_link` 短路、`Frame::is_link` 去掉、回显的上色去掉），对应的用例如期变红 —— 没有哪一条
    是恒真的。
- **三条 failed 的来历**（第一轮跑的时候）：`tests/session_close_button.rs` 两条与
  `tests/session_page.rs` 一条 —— **墙钟依赖**，与这一票无关。它们拿「90 分钟前」当夹具，而措辞按
  **本地日期**算（`wording.rs`：「昨天」按本地日期，不按 24 小时算）—— 00:00–01:30 之间跑，`90 分钟前`
  就落到昨天，措辞变成「昨天 23:41」而不是「1 小时前」，断言就红。换个时区（`TZ=America/Los_Angeles`）
  跑同样的三个文件，**全绿**；过了 01:30 再跑，**也全绿**。这是仓库里既有的脆弱夹具，不在本票的写入
  范围里 —— 维护者已在干净的 worktree 上复跑过，**基线也红**。
  （开工时还有第四条：`tests/render_layout.rs` 的
  `clicking_a_workspace_file_asks_to_open_its_absolute_path` —— 它钉的是「点一条工作区文件链接 →
  交给宿主 `xdg-open`」，而[票 02](02-open-in-file-viewer.md) 把这一档改成走文件查看器，那条断言按
  新行为就该作废。票 02 已经把它改掉了。）
- **真机走查没做的部分**：票面验收里那一条「手工看一屏」我**没有在真终端里走**（这个会话没有
  TTY）。等价的检查我用**帧级**那条用例自动化了：真的画一帧对话视图，从屏幕缓冲里读那一格的前景色
  —— `README.md` 是 `LightCyan`、同一行的 `cargo test` 是 `DarkGray`。**眼睛看过去**的那一下（青与灰
  在真实终端配色下够不够分得开）仍需人在真机上看一眼；spec 的手工清单那一条（㉝）与文档面是
  [票 03](03-docs-and-walkthrough.md) 的事。
- **我拿不准的**（第一轮）：**引用块里的 Markdown 链接**与**嵌套图片**。两处都请维护者裁定 ——
  见下面第二轮的记录：引用块已裁定为**保持叙述灰**（spec §1 已补成「例外只有一处：引用块」），
  嵌套图片已用用例钉住。

- **第二轮（维护者复核后改的一处）**：**引用块里的 Markdown 链接标签与目标回显保持叙述灰**，不上
  链接色。理由是「引用里的一切都是叙述灰」这条既有规矩（`src/render/markdown.rs` 的 `style()`）**先于**这一
  档 —— 引用块整体一个声音，链接另有下划线、仍点得开，不必再抢一档颜色。spec §1 已补成「例外
  只有一处：引用块」。
  - **改法**：判据收在一处 —— `Renderer::link_colour_here(file_link)`：**引用里不上**，其余照旧。
    三个取色的地方都问它：`style()`（标签与普通文字）、`code_colour()`（标签里的行内代码）、
    `close_link()`（那条目标回显 —— 那一帧已经被弹出，所以它问的是 `Frame::is_file_link()` 而不是
    `in_file_link()`）。于是「引用块先赢」在一处说了算，三处不会走散。
  - **补的用例六条**：`markdown.rs` 四条（引用里的文件链接 → 标签与回显都是 `CODE_QUIET`；引用里
    的行内代码标签 → 仍是 `CODE_QUIET`；嵌套图片 `[![图](pic.png)](seed.md)` → alt 与占位符保持
    本来的颜色、外层回显是 `LINK`；嵌套图片旁边的**外层文字标签** `[![图](pic.png) 文档](seed.md)`
    → 那一段文字是 `LINK`）、`tests/link_colour.rs` 两条（引用里的文件链接是叙述灰；嵌套图片的 alt
    不染而外层回显是链接色）。
  - **这六条也逐条验过会红**：把 `link_colour_here()` 的引用判据去掉 → 两条引用用例变红；把
    `Frame::is_link` 去掉 → 三条嵌套图片用例变红。
  - **验收**：`cargo test` 全量 **1933 passed / 0 failed**（最后一次跑时已经过了 01:30，那三条墙钟
    夹具的窗口过去了；维护者已在干净的 worktree 上复跑过，**基线也红**，与本票无关）。本票相关的
    四组 —— `links.rs` **19 passed**、`markdown.rs` **9 passed**、`palette.rs` **4 passed**、
    `tests/link_colour.rs` **11 passed**，全部 0 failed。`cargo clippy --all-targets` **零告警**（全仓
    干净）；`cargo fmt --check` **全仓干净**；`python3 scripts/check-language.py` OK。本票新增测试
    累计 **24 条**。

- **第三轮（两轴 review 之后，2026-10-21，Lead 改）**：
  - **代码块里的路径曾经被染成链接色**（真缺陷）：判据是「span 没有指定前景色」，而**没有
    语言**的围栏块整个是 `Class::Plain`、高亮块里的普通词与标点也是 —— 那一档本来没有前景
    色，于是代码块里的 `README.md` 变成了青的，而 spec §1 那张表写的是「代码块 → 不上」。
    改法：**代码块显式说出自己的前景色是终端默认**（`markdown.rs` 的 `code_span_style`，
    画出来与「没有前景色」一模一样），链接层的「正文」判据随之收窄成「**没有指定**前景色」。
    补一条用例把两种代码块（无语言 / `sh`）都钉住。
  - **判据函数少一层转发**：`is_file_path` 就是那条判据本身（原来 `looks_like_path` 私有、
    外面套一层只转发的 `is_file_path`）—— 那层转发删掉了。
  - **「文件弹窗」这个词不新造**：仓库词表里已有**文件查看器（Viewer）**（`CONTEXT.md`），
    本轮一律改用它，那条词条也补上了第二个入口（对话视图里的文件链接）。
  - **既有 spec 的回改**（仓库的规矩：`tui-visual-language` spec「实现时若发现某条既有 spec
    与这里的决定冲突，**回改那份 spec 正文**」）：`.scratch/clickable-links/spec.md` 五处与
    `.scratch/markdown-render/spec.md` §6 各补了一段记。
  - **一条断言不钉色值**：原来那条把 `palette::LINK` 的色值钉死了，改成钉语义（与
    `CODE_QUIET` 不同）—— 改色板时它不该红（`tui-visual-language` spec §测试决定）。
