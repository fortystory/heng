//! 左栏「会话」页的**关闭标记**：指针悬在某一行上，那一行的行首出现一个红色 `✖`；
//! 点它关掉**那一行**的会话（不落焦点、不切前台）；页顶那个 `关闭` 按钮退场。
//!
//! 票 [`05`](../.scratch/session-close-button/issues/05-close-button-in-sessions-page.md)；
//! 形状与列账在[形态票](../.scratch/session-close-button/issues/02-prototype-close-button-shapes.md)。

mod common;

use common::{
    buffer, click, find_leftmost_cell, fresh, host, host_read_only, hover, load, open_sessions,
    row, row_text, screen,
};
use heng::render::FrontEndEvent;
use heng::session::listing::{SessionMark, SessionRow};

/// 两行会话：都在宿主表里（关得掉）。
fn two_hosted() -> heng::render::TuiState {
    let mut state = fresh();
    load(
        &mut state,
        vec![
            row("s-b", "左栏会话页", "heng", SessionMark::Idle, 2),
            row("s-a", "改一下左栏", "heng", SessionMark::Idle, 90),
        ],
        2,
    );
    host(&mut state, "s-b", &["s-b", "s-a"]);
    state
}

/// 悬浮的那一行，行首出现关闭标记，标题从第 3 列起（宽档 40 列）。
#[test]
fn the_hovered_row_shows_a_close_mark() {
    let mut state = two_hosted();
    open_sessions(&mut state, 120, 24);
    let frame = buffer(120, 24, &mut state);
    let (column, row) = find_leftmost_cell(&frame, 120, 24, "改一下左栏");

    hover(&mut state, column, row);
    let frame = buffer(120, 24, &mut state);
    let line = row_text(&frame, row, 40);

    assert!(line.starts_with("✖ "), "行首是关闭标记：{line}");
    assert!(line.chars().nth(2) == Some('改'), "标题从第 3 列起：{line}");
    assert!(line.contains("1 小时前"), "时间列还在：{line}");
}

/// 窄档（28 列）也一样给标记，标题让一列。
#[test]
fn the_narrow_column_also_shows_a_close_mark() {
    let mut state = two_hosted();
    open_sessions(&mut state, 80, 24);
    let frame = buffer(80, 24, &mut state);
    let (column, row) = find_leftmost_cell(&frame, 80, 24, "改一下左栏");

    hover(&mut state, column, row);
    let frame = buffer(80, 24, &mut state);
    let line = row_text(&frame, row, 28);

    assert!(line.starts_with("✖ "), "行首是关闭标记：{line}");
    assert!(line.chars().nth(2) == Some('改'), "标题从第 3 列起：{line}");
    assert_eq!(
        heng::render::width::text_columns(&line),
        28,
        "行宽不变：{line}"
    );
}

/// 指针移开，标记消失。
#[test]
fn the_close_mark_disappears_when_the_pointer_moves_away() {
    let mut state = two_hosted();
    open_sessions(&mut state, 120, 24);
    let frame = buffer(120, 24, &mut state);
    let (column, row) = find_leftmost_cell(&frame, 120, 24, "改一下左栏");
    hover(&mut state, column, row);
    let frame = buffer(120, 24, &mut state);
    assert!(row_text(&frame, row, 40).starts_with('✖'), "悬着时有标记");

    hover(&mut state, 5, 23);
    let frame = buffer(120, 24, &mut state);

    assert!(
        !row_text(&frame, row, 40).contains('✖'),
        "指针移开后标记消失：{}",
        row_text(&frame, row, 40)
    );
}

/// 只在盘上、没装载的行**没有**标记 —— 它本来也关不掉。
#[test]
fn a_session_that_is_only_on_disk_has_no_close_mark() {
    let mut state = fresh();
    load(
        &mut state,
        vec![
            row("s-b", "左栏会话页", "heng", SessionMark::Idle, 2),
            row("s-a", "改一下左栏", "heng", SessionMark::Idle, 90),
        ],
        2,
    );
    // 宿主表里只有 s-b：s-a 只在盘上。
    host(&mut state, "s-b", &["s-b"]);
    open_sessions(&mut state, 120, 24);
    let frame = buffer(120, 24, &mut state);
    let (column, row) = find_leftmost_cell(&frame, 120, 24, "改一下左栏");

    hover(&mut state, column, row);
    let frame = buffer(120, 24, &mut state);

    assert!(
        !row_text(&frame, row, 40).contains('✖'),
        "没装载的那一行没有标记：{}",
        row_text(&frame, row, 40)
    );
}

/// 只读预览的会话关得掉（既有规则），所以它**有**标记。
#[test]
fn a_read_only_session_also_has_a_close_mark() {
    let mut state = fresh();
    load(
        &mut state,
        vec![row("s-ro", "只读那一个会话", "heng", SessionMark::Idle, 2)],
        1,
    );
    host_read_only(&mut state, "s-ro", &[("s-ro", true)]);
    open_sessions(&mut state, 120, 24);
    let frame = buffer(120, 24, &mut state);
    let (column, row) = find_leftmost_cell(&frame, 120, 24, "只读那一个会话");

    hover(&mut state, column, row);
    let frame = buffer(120, 24, &mut state);

    assert!(
        row_text(&frame, row, 40).starts_with('✖'),
        "只读会话也关得掉，所以有标记：{}",
        row_text(&frame, row, 40)
    );
}

/// 点标记关掉的是**那一行**：不是焦点行那个会话，也不切前台、不改焦点。
#[test]
fn clicking_the_close_mark_closes_that_row() {
    let mut state = two_hosted();
    open_sessions(&mut state, 120, 24);
    let frame = buffer(120, 24, &mut state);
    let (column, row) = find_leftmost_cell(&frame, 120, 24, "改一下左栏");
    hover(&mut state, column, row);
    let _ = buffer(120, 24, &mut state);

    // 落焦点在**另一行**上：关闭标记关的仍是被点的那一行。
    let (other, other_row) = find_leftmost_cell(&frame, 120, 24, "左栏会话页");
    click(&mut state, other + 2, other_row);
    state.take_events();
    let focus_before = state.focused_session().map(|id| id.as_str().to_owned());

    // 点第二行行首那个关闭标记。
    click(&mut state, 0, row);
    let events = state.take_events();

    assert!(
        events.iter().any(|event| matches!(
            event,
            FrontEndEvent::CloseSession { id } if id.as_str() == "s-a"
        )),
        "关的是被点的那一行：{events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, FrontEndEvent::OpenSession { .. })),
        "点标记不切前台：{events:?}"
    );
    assert_eq!(
        state.focused_session().map(|id| id.as_str()),
        focus_before.as_deref(),
        "点标记不改焦点行"
    );
}

/// 页顶只剩读数与 `新建` —— 那个 `关闭` 入口退场了。
#[test]
fn the_header_has_no_close_button_anymore() {
    let mut state = two_hosted();
    open_sessions(&mut state, 120, 24);
    let text = screen(120, 24, &mut state);

    assert!(text.contains("新建"), "右端只剩新建：{text}");
    assert!(!text.contains("关闭"), "页顶没有关闭入口了：{text}");
}

/// 前台那一行的标记位不再写字形 —— 前台由标题色说，焦点由粗体与时间列说。
#[test]
fn the_foreground_marker_is_gone() {
    let mut state = fresh();
    load(
        &mut state,
        vec![SessionRow {
            foreground: true,
            ..row("s-b", "左栏会话页", "heng", SessionMark::Idle, 2)
        }],
        1,
    );
    host(&mut state, "s-b", &["s-b"]);
    open_sessions(&mut state, 120, 24);
    let frame = buffer(120, 24, &mut state);
    let (_, row) = find_leftmost_cell(&frame, 120, 24, "左栏会话页");

    let line = row_text(&frame, row, 40);

    assert!(!line.starts_with('▍'), "前台标记退场了：{line}");
}

/// 指针落在**标记那一格**上时，悬浮目标还是那一行 —— 标记不会一点就消失，也点得到。
#[test]
fn the_close_mark_itself_stays_hovered() {
    let mut state = two_hosted();
    open_sessions(&mut state, 120, 24);
    let frame = buffer(120, 24, &mut state);
    let (column, row) = find_leftmost_cell(&frame, 120, 24, "改一下左栏");
    hover(&mut state, column, row);
    let frame = buffer(120, 24, &mut state);
    assert!(row_text(&frame, row, 40).starts_with('✖'), "悬着时有标记");

    // 指针挪到**标记那一格**上（行首第 0 列）。
    hover(&mut state, 0, row);
    let frame = buffer(120, 24, &mut state);
    assert_eq!(
        state.hovered_session().map(|id| id.as_str()),
        Some("s-a"),
        "标记那一格仍然指着那一行"
    );
    assert!(
        row_text(&frame, row, 40).starts_with('✖'),
        "标记不会一点就消失：{}",
        row_text(&frame, row, 40)
    );

    click(&mut state, 0, row);
    let events = state.take_events();
    assert!(
        events.iter().any(|event| matches!(
            event,
            FrontEndEvent::CloseSession { id } if id.as_str() == "s-a"
        )),
        "点标记能关掉那一行：{events:?}"
    );
}

/// 关闭标记**顶替**状态标记位：那一行的 `●` / `?` 此刻看不见。
#[test]
fn the_close_mark_takes_the_state_marks_place() {
    let mut state = fresh();
    load(
        &mut state,
        vec![
            row("s-run", "在跑的那一个会话", "heng", SessionMark::Running, 2),
            row("s-a", "改一下左栏", "heng", SessionMark::Idle, 90),
        ],
        2,
    );
    host(&mut state, "s-run", &["s-run", "s-a"]);
    open_sessions(&mut state, 120, 24);
    let frame = buffer(120, 24, &mut state);
    let (column, row) = find_leftmost_cell(&frame, 120, 24, "在跑的那一个会话");
    assert!(
        row_text(&frame, row, 40).starts_with('●'),
        "悬浮之前状态标记在那儿：{}",
        row_text(&frame, row, 40)
    );

    hover(&mut state, column, row);
    let frame = buffer(120, 24, &mut state);
    let line = row_text(&frame, row, 40);

    assert!(line.starts_with('✖'), "状态标记让位给关闭标记：{line}");
    assert!(!line.contains('●'), "那一行此刻不再显示「在跑」：{line}");
}

/// 标题让一列（宽档 27→26），**时间列不动**。
#[test]
fn the_title_gives_one_column_and_the_time_column_stays() {
    let mut state = fresh();
    // 标题用 ASCII：中文标题的字符数与显示列数不等，量列要用能对齐的那种。
    let long = "abcdefghijklmnopqrstuvwxyz0123456789";
    load(
        &mut state,
        vec![row("s-a", long, "heng", SessionMark::Idle, 90)],
        1,
    );
    host(&mut state, "s-a", &["s-a"]);
    open_sessions(&mut state, 120, 24);
    let frame = buffer(120, 24, &mut state);
    let (_, row) = find_leftmost_cell(&frame, 120, 24, "abcdefghij");

    assert_eq!(frame[(1, row)].symbol(), "a", "未悬浮时标题从第 2 列起");
    assert_eq!(
        title_width_in_row(&frame, row, 40),
        27,
        "未悬浮时标题占 27 列"
    );
    let time_before = last_text_start(&frame, row, 40);

    hover(&mut state, 1, row);
    let frame = buffer(120, 24, &mut state);

    assert_eq!(frame[(2, row)].symbol(), "a", "悬浮时标题从第 3 列起");
    assert_eq!(
        title_width_in_row(&frame, row, 40),
        26,
        "悬浮时标题占 26 列"
    );
    assert_eq!(last_text_start(&frame, row, 40), time_before, "时间列不动");
}

/// 那一行里标题占了几列（从标题首字起，连续的那一段）。
fn title_width_in_row(frame: &ratatui::buffer::Buffer, row: u16, width: u16) -> u16 {
    let line = row_text(frame, row, width);
    let start = line.find('a').expect("标题在");
    line[start..].chars().take_while(|c| *c != ' ').count() as u16
}

/// 那一行里**最后一段文字**的首列（时间列）。
fn last_text_start(frame: &ratatui::buffer::Buffer, row: u16, width: u16) -> u16 {
    let mut x = width;
    while x > 0 {
        x -= 1;
        if frame[(x, row)].symbol() != " " {
            let mut start = x;
            while start > 0 && frame[(start - 1, row)].symbol() != " " {
                start -= 1;
            }
            return start;
        }
    }
    0
}

/// 关闭标记是**红**的（`palette::CLOSE`）—— 与「错误」红撞值，但不混名。
#[test]
fn the_close_mark_is_red() {
    let mut state = two_hosted();
    open_sessions(&mut state, 120, 24);
    let frame = buffer(120, 24, &mut state);
    let (column, row) = find_leftmost_cell(&frame, 120, 24, "改一下左栏");
    hover(&mut state, column, row);
    let frame = buffer(120, 24, &mut state);

    assert_eq!(
        frame[(0, row)].fg,
        ratatui::style::Color::Red,
        "关闭标记是红的"
    );
}

/// 只读预览的那一行：标记是**同一个红**（它关得掉，所以给标记）。
#[test]
fn the_read_only_row_has_the_same_red_mark() {
    let mut state = fresh();
    load(
        &mut state,
        vec![row("s-ro", "只读那一个会话", "heng", SessionMark::Idle, 2)],
        1,
    );
    host_read_only(&mut state, "s-ro", &[("s-ro", true)]);
    open_sessions(&mut state, 120, 24);
    let frame = buffer(120, 24, &mut state);
    let (column, row) = find_leftmost_cell(&frame, 120, 24, "只读那一个会话");
    hover(&mut state, column, row);
    let frame = buffer(120, 24, &mut state);

    assert_eq!(
        frame[(0, row)].fg,
        ratatui::style::Color::Red,
        "只读行的标记与普通行同色"
    );
}

/// 焦点行靠 **ACCENT + 粗体**认 —— 与「在不在前台」是**正交**的两件事（票 29）。
#[test]
fn the_focused_row_is_accent_and_bold() {
    let mut state = two_hosted();
    open_sessions(&mut state, 120, 24);
    let frame = buffer(120, 24, &mut state);
    let (column, row) = find_leftmost_cell(&frame, 120, 24, "改一下左栏");
    click(&mut state, column, row);
    let frame = buffer(120, 24, &mut state);

    assert!(
        frame[(column, row)]
            .modifier
            .contains(ratatui::style::Modifier::BOLD),
        "焦点行的标题是粗体"
    );
    let time_at = find_leftmost_cell(&frame, 120, 24, "1 小时前").0;
    assert_eq!(
        frame[(time_at, row)].fg,
        ratatui::style::Color::LightMagenta,
        "焦点行的时间列是 ACCENT"
    );
}
