//! 指针的移动事件：渲染器知道指针停在**哪一条会话**上（悬浮目标），并且**只在目标变化时**
//! 才请求重画（`.scratch/session-close-button/spec.md` 的「B 悬浮目标与指针事件」、
//! 票 [`04`](../.scratch/session-close-button/issues/04-pointer-move-events.md)）。
//!
//! 帧级断言照 `tests/render_layout.rs` 与 `tests/session_page.rs` 的写法：`TestBackend` 上画一帧，
//! 把屏幕文本读回来。

mod common;

use common::{buffer, find_cell, fresh, host, hover, load, open_sessions, row};
use heng::render::Key;
use heng::session::listing::SessionMark;
use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

/// 指针落在会话页的某一行上时，悬浮目标就是那一行的会话。
#[test]
fn the_pointer_finds_the_session_row_under_it() {
    let mut state = fresh();
    load(
        &mut state,
        vec![
            row("s-b", "左栏会话页", "heng", SessionMark::Idle, 2),
            row("s-a", "改一下左栏", "heng", SessionMark::Idle, 90),
        ],
        2,
    );
    open_sessions(&mut state, 120, 24);
    let frame = buffer(120, 24, &mut state);
    let (column, row) = find_cell(&frame, 120, 24, "改一下左栏");

    hover(&mut state, column, row);

    assert_eq!(
        state.hovered_session().map(|id| id.as_str()),
        Some("s-a"),
        "指针停在第二行上，悬浮目标就是第二行那个会话"
    );
}

/// 指针落在顶部页签条的一个标签上时，悬浮目标就是那个标签的会话。
#[test]
fn the_pointer_finds_the_session_under_a_top_tab() {
    let mut state = fresh();
    load(
        &mut state,
        vec![
            row("s-1", "甲会话", "heng", SessionMark::Idle, 2),
            row("s-2", "乙会话", "heng", SessionMark::Idle, 90),
        ],
        2,
    );
    host(&mut state, "s-1", &["s-1", "s-2"]);
    let frame = buffer(120, 24, &mut state);
    let (column, row) = find_cell(&frame, 120, 24, "乙会话");
    assert_eq!(row, 0, "页签条的标签在第 0 行：{column},{row}");

    hover(&mut state, column, row);

    assert_eq!(
        state.hovered_session().map(|id| id.as_str()),
        Some("s-2"),
        "指针停在第二个标签上，悬浮目标就是那一个会话"
    );
}

/// 指针落在平铺态某一格的标题上时，悬浮目标就是那一格的会话。
#[test]
fn the_pointer_finds_the_session_under_a_tile() {
    let mut state = fresh();
    load(
        &mut state,
        vec![
            row("s-1", "甲会话", "heng", SessionMark::Idle, 2),
            row("s-2", "乙会话", "heng", SessionMark::Idle, 90),
        ],
        2,
    );
    host(&mut state, "s-1", &["s-1", "s-2"]);
    state.key(Key::CtrlR);
    let frame = buffer(120, 40, &mut state);
    let (column, row) = find_cell(&frame, 120, 40, "乙会话");

    hover(&mut state, column, row);

    assert_eq!(
        state.hovered_session().map(|id| id.as_str()),
        Some("s-2"),
        "指针停在第二格上，悬浮目标就是那一格的会话"
    );
}

/// 平铺态里只有**格标题行**算：格里的正文不是会话名，悬浮它没有会话可指。
#[test]
fn the_body_of_a_tile_is_not_a_session_name() {
    let mut state = fresh();
    load(
        &mut state,
        vec![
            row("s-1", "甲会话", "heng", SessionMark::Idle, 2),
            row("s-2", "乙会话", "heng", SessionMark::Idle, 90),
        ],
        2,
    );
    host(&mut state, "s-1", &["s-1", "s-2"]);
    state.key(Key::CtrlR);
    let frame = buffer(120, 40, &mut state);
    let (column, row) = find_cell(&frame, 120, 40, "乙会话");

    // 那一格的正文（标题行的下一行）：整格可点，但那里没有会话名。
    hover(&mut state, column, row + 1);

    assert_eq!(
        state.hovered_session(),
        None,
        "格里的正文不是会话名，没有悬浮目标"
    );
}

/// 列表滚动之后，悬浮目标跟着**这一帧**的命中表重算 —— 指针没动，底下那条会话换了。
#[test]
fn the_hovered_session_follows_the_frame_it_was_drawn_in() {
    let mut state = fresh();
    let rows: Vec<heng::session::listing::SessionRow> = (0..30)
        .map(|index| {
            row(
                &format!("s-{index:02}"),
                &format!("会话 {index:02}"),
                "heng",
                SessionMark::Idle,
                index,
            )
        })
        .collect();
    load(&mut state, rows, 30);
    open_sessions(&mut state, 120, 24);
    let frame = buffer(120, 24, &mut state);
    let (column, row) = find_cell(&frame, 120, 24, "会话 01");
    hover(&mut state, column, row);
    assert_eq!(
        state.hovered_session().map(|id| id.as_str()),
        Some("s-01"),
        "悬浮在第一行上"
    );

    // 列表往下滚一格：指针没动，但同一屏位置上换了一个会话。
    state.mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    });
    let _ = buffer(120, 24, &mut state);
    state.refresh_hover();

    assert_eq!(
        state.hovered_session().map(|id| id.as_str()),
        Some("s-02"),
        "滚动之后悬浮目标跟着这一帧的命中表走"
    );
}

/// 同一个会话里挪动指针**不**请求重画 —— 终端一次横扫能送出上百个移动事件。
#[test]
fn moving_within_one_session_does_not_ask_for_a_redraw() {
    let mut state = fresh();
    load(
        &mut state,
        vec![
            row("s-b", "左栏会话页", "heng", SessionMark::Idle, 2),
            row("s-a", "改一下左栏", "heng", SessionMark::Idle, 90),
        ],
        2,
    );
    open_sessions(&mut state, 120, 24);
    let frame = buffer(120, 24, &mut state);
    let (column, row) = find_cell(&frame, 120, 24, "改一下左栏");

    hover(&mut state, column, row);
    assert!(state.is_dirty(), "换了一个会话 → 要重画");

    state.mark_clean();
    hover(&mut state, column + 1, row);
    assert!(!state.is_dirty(), "同一个会话里挪一格 → 不重画");

    state.mark_clean();
    let (other, other_row) = find_cell(&frame, 120, 24, "左栏会话页");
    hover(&mut state, other, other_row);
    assert!(state.is_dirty(), "换到另一个会话 → 又要重画");
}

/// 指针不在任何会话上时，没有悬浮目标。
#[test]
fn the_pointer_has_no_session_when_it_is_not_over_one() {
    let mut state = fresh();
    load(
        &mut state,
        vec![row("s-b", "左栏会话页", "heng", SessionMark::Idle, 2)],
        1,
    );
    open_sessions(&mut state, 120, 24);
    let _ = buffer(120, 24, &mut state);

    // 输入区在屏幕底下：那里没有会话名。
    hover(&mut state, 5, 23);

    assert_eq!(
        state.hovered_session(),
        None,
        "指针不在会话上时没有悬浮目标"
    );
}

/// 按住左键拖动仍然是拖选，**不**改变悬浮目标。
#[test]
fn dragging_does_not_change_the_hovered_session() {
    let mut state = fresh();
    load(
        &mut state,
        vec![
            row("s-b", "左栏会话页", "heng", SessionMark::Idle, 2),
            row("s-a", "改一下左栏", "heng", SessionMark::Idle, 90),
        ],
        2,
    );
    open_sessions(&mut state, 120, 24);
    let frame = buffer(120, 24, &mut state);
    let (column, row) = find_cell(&frame, 120, 24, "改一下左栏");
    hover(&mut state, column, row);

    state.mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    });
    state.mouse(MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: column + 4,
        row,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(
        state.hovered_session().map(|id| id.as_str()),
        Some("s-a"),
        "拖动不改变悬浮目标"
    );
}
