//! 对话视图里**文件链接**与**行内代码**分得开：文件链接上链接色，行内代码保持静音灰
//! （`.scratch/link-emphasis/spec.md` §1、§2）。
//!
//! 这一份从**公开的那几个接缝**上断言整件事：色板（`palette::LINK`）、候选识别与上色
//! （`links::mark`、`links::is_file_path`）、Markdown 渲染（`markdown::to_lines`）。模块内
//! 的那些用例在各自模块里；这里是「读者看得见的那件事」从头到尾的一条断言 ——
//! **链接是青的，代码是灰的**，一眼就分得出谁是谁。

use heng::render::links;
use heng::render::markdown::to_lines;
use heng::render::palette;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

mod common;

use common::{buffer, fresh, said};
use ratatui::buffer::Buffer;

/// 测试用的可用宽度：一个像样的主列。
const W: u16 = 78;

/// 一条行里内容是 `needle` 的那一片 span。
fn span_with<'a, 'b>(line: &'a Line<'b>, needle: &str) -> &'a Span<'b> {
    line.spans
        .iter()
        .find(|span| span.content.as_ref() == needle)
        .unwrap_or_else(|| panic!("这一行里没有 {needle}：{line:?}"))
}

/// 那一片 span 的前景色。
fn fg_of(line: &Line<'_>, needle: &str) -> Option<Color> {
    span_with(line, needle).style.fg
}

/// **含有** `needle` 的那一片 span 的前景色（一行里那一段字与别处连在一片时用这个）。
fn fg_in(line: &Line<'_>, needle: &str) -> Option<Color> {
    line.spans
        .iter()
        .find(|span| span.content.contains(needle))
        .unwrap_or_else(|| panic!("这一行里没有 {needle}：{line:?}"))
        .style
        .fg
}

/// 屏幕上那段文字所在的那一格的前景色。
fn fg_on_screen(frame: &Buffer, width: u16, height: u16, needle: &str) -> Color {
    for y in 0..height {
        let line = common::row_text(frame, y, width);
        let Some(at) = line.find(needle) else {
            continue;
        };
        let x = heng::render::width::text_columns(&line[..at]) as u16;
        return frame[(x, y)].fg;
    }
    panic!("屏幕上找不到 {needle}");
}

/// 链接色是内容域新加的一档；它与行内代码的静音灰**不是**同一档 —— 那一档新颜色存在的全部
/// 理由就是这个。**不钉色值**（改色板时这条不该红；`tui-visual-language` spec §测试决定）。
#[test]
fn the_link_colour_is_its_own_and_not_the_quiet_grey() {
    assert_ne!(palette::LINK, palette::CODE_QUIET, "链接与行内代码要分得开");
}

/// 「这是不是一个文件路径」用的是链接层那一份判据，不另起炉灶。
#[test]
fn the_file_predicate_is_the_one_the_link_layer_uses() {
    assert!(links::is_file_path("seed.md"), "带扩展名的文件名是路径");
    assert!(links::is_file_path("docs/render.md"), "带目录的路径是路径");
    assert!(
        !links::is_file_path("https://example.com/x"),
        "URL 不是文件路径"
    );
    assert!(
        !links::is_file_path("mailto:ada@example.com"),
        "带 scheme 的不是路径"
    );
    assert!(!links::is_file_path("已经写好了"), "一句中文不是路径");
}

/// 正文里的一条路径候选：前景色变成链接色。它与同一行里那段行内代码**分得开**。
#[test]
fn a_file_path_in_body_text_becomes_a_link() {
    let mut rows = vec![Line::from("见 README.md 与 `cargo test`")];
    links::mark(&mut rows, &[false]);
    assert_eq!(
        fg_of(&rows[0], "README.md"),
        Some(palette::LINK),
        "正文里的路径候选是链接色：{:?}",
        rows[0]
    );
}

/// 同一段文字写在**行内代码**里：仍是静音灰。行内代码保持灰正是这一版要的那条区分。
#[test]
fn the_same_text_as_inline_code_stays_grey() {
    let mut rows = vec![Line::from(Span::styled(
        "见 README.md",
        Style::default().fg(palette::CODE_QUIET),
    ))];
    links::mark(&mut rows, &[false]);
    assert_eq!(
        fg_of(&rows[0], "README.md"),
        Some(palette::CODE_QUIET),
        "行内代码保持灰：{:?}",
        rows[0]
    );
}

/// 一条 URL 候选：前景色**不动**。它走浏览器那一路，不是文件链接。
#[test]
fn a_url_keeps_its_own_colour() {
    let mut rows = vec![Line::from("见 https://example.com/x")];
    links::mark(&mut rows, &[false]);
    assert_eq!(
        fg_of(&rows[0], "https://example.com/x"),
        None,
        "URL 不上链接色：{:?}",
        rows[0]
    );
}

/// 一条指向**文件**的 Markdown 链接：标签与目标回显都上链接色。标签是行内代码时**也**上
/// —— 「这是一个链接」比「这是一段代码」更该说清楚。
#[test]
fn a_markdown_link_to_a_file_is_a_link_colour() {
    let lines = to_lines("[`seed.md`](seed.md) 与 [文档](docs/render.md)", W);
    assert_eq!(
        fg_of(&lines[0], "seed.md"),
        Some(palette::LINK),
        "行内代码做的标签也是链接色：{:?}",
        lines[0]
    );
    assert_eq!(
        fg_of(&lines[0], "文档"),
        Some(palette::LINK),
        "标签是链接色：{:?}",
        lines[0]
    );
    assert_eq!(
        fg_of(&lines[0], " (docs/render.md)"),
        Some(palette::LINK),
        "目标回显也是链接色：{:?}",
        lines[0]
    );
}

/// 目标是 **URL** 的链接：标签与回显**都照旧**（回显是退后一档的灰）。
#[test]
fn a_markdown_link_to_a_url_is_not_a_link_colour() {
    let lines = to_lines("[文档](https://example.com/x)", W);
    assert_ne!(
        fg_of(&lines[0], "文档"),
        Some(palette::LINK),
        "URL 链接不上链接色：{:?}",
        lines[0]
    );
    assert_eq!(
        fg_of(&lines[0], " (https://example.com/x)"),
        Some(palette::CODE_QUIET),
        "回显照旧是灰的：{:?}",
        lines[0]
    );
}

/// **代码块**里的路径：不上链接色。代码块是一段代码，不是正文 —— 它显式说出自己的前景色
/// 是「终端默认」，于是链接层认得出它（`.scratch/link-emphasis/spec.md` §1）。
///
/// 两种代码块都要钉住：**没有语言**的围栏块（整个是普通文字）与**有语言**的围栏块（普通词
/// 与标点落在没有语法色的那一档上）—— 两者都曾经被染成链接色。
#[test]
fn a_path_in_a_code_block_is_not_a_link_colour() {
    for block in ["```\ncp README.md\n```", "```sh\ncp README.md\n```"] {
        let lines = to_lines(block, W);
        let row = lines
            .iter()
            .find(|line| {
                line.spans
                    .iter()
                    .any(|span| span.content.contains("README.md"))
            })
            .unwrap_or_else(|| panic!("代码块里没有那一行：{lines:?}"));
        assert_ne!(
            fg_in(row, "README.md"),
            Some(palette::LINK),
            "代码块里的路径不上链接色：{row:?}"
        );
    }
}

/// 一段**独立的**行内代码：仍是静音灰。只有「行内代码在文件链接的标签里」那一处被染成
/// 链接色。
#[test]
fn a_lonely_inline_code_is_still_grey() {
    let lines = to_lines("跑 `cargo test` 就好", W);
    assert_eq!(
        fg_of(&lines[0], "cargo test"),
        Some(palette::CODE_QUIET),
        "独立的行内代码不染：{:?}",
        lines[0]
    );
}

/// 真机走查的那一屏：对话视图里，一行正文里的 `README.md` 是**青的**，同一行的
/// `cargo test` 是**灰的** —— 两者一眼分得开。
#[test]
fn in_the_conversation_view_a_file_link_is_cyan_and_inline_code_is_grey() {
    let mut state = fresh();
    state.apply(said("跑完了：README.md，跑 `cargo test` 看看"));
    let frame = buffer(120, 24, &mut state);
    assert_eq!(
        fg_on_screen(&frame, 120, 24, "README.md"),
        palette::LINK,
        "正文里的文件链接是链接色"
    );
    assert_eq!(
        fg_on_screen(&frame, 120, 24, "cargo test"),
        palette::CODE_QUIET,
        "同一行的行内代码是灰的"
    );
}

/// **引用块**里的文件链接：标签与目标回显都**保持叙述灰**。「引用里的一切都是叙述灰」先于
/// 这一档 —— 引用块整体一个声音，链接仍有下划线、仍点得开
/// （`.scratch/link-emphasis/spec.md` §1 的那一条例外）。
#[test]
fn a_file_link_inside_a_quote_stays_the_narrative_grey() {
    let lines = to_lines("> 引用里的 [文档](seed.md)", W);
    assert_eq!(
        fg_of(&lines[0], "文档"),
        Some(palette::CODE_QUIET),
        "引用里的标签是叙述灰：{:?}",
        lines[0]
    );
    assert_eq!(
        fg_of(&lines[0], " (seed.md)"),
        Some(palette::CODE_QUIET),
        "引用里的回显是叙述灰：{:?}",
        lines[0]
    );
}

/// **嵌套图片**（`[![图](pic.png)](seed.md)`）：图片的 alt 与占位符保持本来的颜色 —— 它们是
/// 图片，不是链接；外层那条链接的回显是链接色。
#[test]
fn a_nested_image_keeps_its_own_colours_inside_a_file_link() {
    let lines = to_lines("[![图](pic.png)](seed.md)", W);
    assert_ne!(
        fg_of(&lines[0], "图"),
        Some(palette::LINK),
        "图片的 alt 不上链接色：{:?}",
        lines[0]
    );
    assert_eq!(
        fg_of(&lines[0], " (seed.md)"),
        Some(palette::LINK),
        "外层链接的回显是链接色：{:?}",
        lines[0]
    );
}
