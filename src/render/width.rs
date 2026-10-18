//! 显示宽度的算术，以及它的两个出口：一段文字占多少终端列，怎么把它裁到某个宽度。
//!
//! 算术只住一处，因为渲染器有两处需要同一个答案 —— 窗格把带样式的行折到某个宽度，
//! 左栏、提示行与指示器把文字裁到某一个 —— 而第二份拷贝就是 `终` 被算成一列的由来。
//! 截断因此也在这里：`truncate_columns` 交回纯文本，`ellipsize_line` 保住每一片的样式。
//!
//! ## 两把尺子：按显示列 vs 按字符
//!
//! 截断有两种口径，代码必须分清：
//!
//! - **按显示列**（[`truncate_columns`] / [`ellipsize_line`]）：目标是「这一段在终端上能占
//!   多少列」，宽字符算两列，遇上 `万` 这种 CJK 一格不算一格。窗格折行、左栏裁文字、详情弹
//!   窗里算宽度都要走这把。
//! - **按字符**（[`truncate_chars`]）：目标是「这一段有几个 `char`」，宽字符不补列。文件
//!   `read_file` 那一档的「单行字符封顶」、工具输出预览、详情里说明「这一段截过」这种与终
//!   端列宽无关的口径走这把。
//!
//! 两者**不能互换** —— 把按字符的地方换成按列，CJK 文本会与 `read_file` 那边的字数限制不
//! 一致；把按列的地方换成按字符，终端屏上的列预算会被 CJK 字撑爆。这一段先写出判据，是
//! 因为 6 处 `chars().take` 的旧用法表面和按列都长得像，但语义是按字符。

use ratatui::buffer::CellWidth;
use ratatui::text::{Line, Span};

use super::wording;

/// `text` 的显示宽度，单位是终端列。
pub fn text_columns(text: &str) -> usize {
    text.cell_width() as usize
}

/// 单个字符的显示宽度，单位是终端列。
///
/// 控制字符算零列：画家会把它们滤掉，把它们数进去就会把光标放到文字并不在的地方。
/// 这也让这里避开 `cell_width` 那条路 —— 走到那里它会直接断言。
pub fn char_columns(ch: char) -> usize {
    if ch.is_control() {
        return 0;
    }
    let mut buf = [0u8; 4];
    ch.encode_utf8(&mut buf).cell_width() as usize
}

/// `text` 里能塞进 `width` 列的最长前缀。
pub fn truncate_columns(text: &str, width: usize) -> String {
    let mut used = 0;
    let mut end = 0;
    for (index, ch) in text.char_indices() {
        let columns = char_columns(ch);
        if used + columns > width {
            break;
        }
        used += columns;
        end = index + ch.len_utf8();
    }
    text[..end].to_owned()
}

/// `text` 里最前面的 `max_chars` 个 `char`。
///
/// **按字符而不是按显示列** —— 宽字符不算两列。文件 `read_file` 的单行字数封顶、工具
/// 输出预览、详情弹窗里说明「这一段截过」这些与终端列宽无关的口径走这把；要把文字在
/// 终端列预算里收紧，请用 [`truncate_columns`]。
///
/// `max_chars >= text.chars().count()` 时整段返回；否则停在边界那一格之后。
pub fn truncate_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

/// `text` 里从第 `from` 列到第 `to` 列（不含）的那一段。
///
/// 按**显示列**切，宽字符不切半：跨过边界的那个宽字素整个丢掉，而不是留下半个（拖选取文本时
/// 宁可少取一个字符，也不能取回半个）。`from >= to` 或整段都在范围外时是空串。
pub fn slice_columns(text: &str, from: usize, to: usize) -> String {
    let mut out = String::new();
    let mut column = 0usize;
    for ch in text.chars() {
        let columns = char_columns(ch);
        if column + columns > to {
            break;
        }
        if column >= from {
            out.push(ch);
        }
        column += columns;
    }
    out
}

/// 把一条带样式的行裁到 `width` 列，并用一个 `…` 收尾。
///
/// 与 [`truncate_columns`] 是同一把尺子的两个出口：那个交回纯文本，这一个保住每一片的样式
/// —— 轨迹页里被截的那条消息行还带着发言者的颜色，砍掉样式会一并砍掉「谁在说话」。
pub fn ellipsize_line(line: Line<'static>, width: usize) -> Line<'static> {
    let budget = width.saturating_sub(1);
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;
    for span in line.spans {
        let mut kept = String::new();
        for ch in span.content.chars() {
            let columns = char_columns(ch);
            if used + columns > budget {
                break;
            }
            kept.push(ch);
            used += columns;
        }
        if !kept.is_empty() {
            spans.push(Span::styled(kept, span.style));
        }
        if used >= budget {
            break;
        }
    }
    // `…` 跟着最后一片的样式，好让它读起来是那一行的一部分。
    let style = spans.last().map(|span| span.style).unwrap_or_default();
    spans.push(Span::styled(wording::ELLIPSIS, style));
    Line {
        spans,
        style: line.style,
        alignment: line.alignment,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 按字符截宽字符：不会切到半个 `char`，与 terminal 列宽无关。
    #[test]
    fn truncate_chars_keeps_a_wide_glyph_intact() {
        // 三个「衡」字（每个 2 列、3 字节）排成 9 列 / 9 字节的串。
        let text = "衡衡衡";
        assert_eq!(truncate_chars(text, 2), "衡衡");
        assert_eq!(truncate_chars(text, 4), "衡衡衡");
    }

    /// 最大值够用时整段返回。
    #[test]
    fn truncate_chars_passes_through_when_budget_is_enough() {
        assert_eq!(truncate_chars("abc", 3), "abc");
        assert_eq!(truncate_chars("abc", 99), "abc");
        assert_eq!(truncate_chars("", 5), "");
    }

    /// 与 `truncate_columns` 不同：按字符的那把不会把 CJK 字符按两列折算。
    #[test]
    fn truncate_chars_does_not_count_wide_glyphs_as_two() {
        // 4 个 char、6 列：`衡`(2) + `a`(1) + `衡`(2) + `b`(1)。
        let text = "衡a衡b";
        // 按列截 4 列：`衡`(2) + `a`(1) + `衡`(2) 整段用掉 5 列，下一个字符 `b` 单列就要越界 → 停在 "衡a"。
        assert_eq!(truncate_columns(text, 4), "衡a");
        // 按字符截 4 char：4 个 char 全部留下。这两把尺子对同一段串不能互换。
        assert_eq!(truncate_chars(text, 4), "衡a衡b");
    }
}
