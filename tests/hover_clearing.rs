//! 黏住的标记会消失（票 `10`）：指针移出终端窗口、或敲了任意一个键，屏幕上那个 `✖` 就不该
//! 一直挂着。
//!
//! 票 [`10`](../../.scratch/session-close-button/issues/10-stale-cross-clearing.md)。

mod common;

use common::{
    buffer, drag_to, find_leftmost_cell, fresh, host, hover, load, open_sessions, press, release,
    row,
};
use heng::render::Key;
use heng::session::listing::SessionMark;

/// 两个会话都在宿主表里。
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

/// 悬到会话页的某一行上，让那个 `✖` 出来。
fn hovering_a_row(state: &mut heng::render::TuiState) -> u16 {
    open_sessions(state, 120, 24);
    let frame = buffer(120, 24, state);
    let (column, row) = find_leftmost_cell(&frame, 120, 24, "改一下左栏");
    hover(state, column, row);
    row
}

/// 终端的焦点变化事件到达 → 悬浮目标清掉（票 10）。
#[test]
fn a_focus_event_clears_the_hover() {
    let mut state = two_hosted();
    let row = hovering_a_row(&mut state);
    assert!(state.hovered_session().is_some(), "悬着时有悬浮目标");
    let _ = buffer(120, 24, &mut state);

    state.focus_changed();

    assert!(state.hovered_session().is_none(), "焦点变了，悬浮目标清掉");
    // 重画之后那一行不再有标记。
    let frame = buffer(120, 24, &mut state);
    assert!(
        !common::row_text(&frame, row, 40).contains('✖'),
        "标记消失了：{}",
        common::row_text(&frame, row, 40)
    );
}

/// 任意键盘输入 → 悬浮目标清掉（票 10）。
#[test]
fn a_keystroke_clears_the_hover() {
    let mut state = two_hosted();
    let row = hovering_a_row(&mut state);
    assert!(state.hovered_session().is_some(), "悬着时有悬浮目标");

    state.key(Key::Char('h'));

    assert!(state.hovered_session().is_none(), "敲了键，悬浮目标清掉");
    let frame = buffer(120, 24, &mut state);
    assert!(
        !common::row_text(&frame, row, 40).contains('✖'),
        "标记消失了：{}",
        common::row_text(&frame, row, 40)
    );
}

/// 拖选**不**把那个 `✖` 选进剪贴板（票 10）—— 标记是画上去的，不在文本层里。
#[test]
fn dragging_does_not_copy_the_close_mark() {
    let mut state = two_hosted();
    let row = hovering_a_row(&mut state);
    let _ = buffer(120, 24, &mut state);

    // 从标记那一列拖到行尾。
    press(&mut state, 0, row);
    drag_to(&mut state, 20, row);
    release(&mut state, 20, row);

    let clip = state.take_clipboard();
    assert!(
        clip.as_deref().is_none_or(|text| !text.contains('✖')),
        "剪贴板里没有那个标记：{clip:?}"
    );
}
