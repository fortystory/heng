//! 两条键位：**全局 `Ctrl-X`**（关掉前台那一个会话）与**会话页内 `x`**（关掉焦点行那一个）。
//!
//! 票 [`09`](../../.scratch/session-close-button/issues/09-keyboard-close-shortcuts.md)。

mod common;

use common::{
    buffer, click, find_leftmost_cell, fresh, host, load, open_sessions, row, switch_front,
};
use heng::render::{FrontEndEvent, Key};
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

/// `Ctrl-X` 关掉**前台**那一个会话（票 09）—— 目标是前台时按票 06 的规则先切到相邻、再关。
#[test]
fn ctrl_x_closes_the_foreground_session() {
    let mut state = two_hosted();
    switch_front(&mut state, "s-b");

    state.key(Key::CtrlX);
    let events = state.take_events();

    assert_eq!(events.len(), 2, "两条请求：{events:?}");
    assert!(
        matches!(&events[0], FrontEndEvent::OpenSession { id } if id.as_str() == "s-a"),
        "先切到相邻的那一个：{events:?}"
    );
    assert!(
        matches!(&events[1], FrontEndEvent::CloseSession { id } if id.as_str() == "s-b"),
        "再关掉前台那一个：{events:?}"
    );
}

/// 会话页内 `x` 关掉**焦点行**那一个会话 —— 键盘路径作用于焦点行，不是「被点的那一行」（票 09）。
#[test]
fn x_closes_the_focused_row() {
    let mut state = two_hosted();
    open_sessions(&mut state, 120, 24);
    let frame = buffer(120, 24, &mut state);
    let (column, row) = find_leftmost_cell(&frame, 120, 24, "改一下左栏");
    click(&mut state, column, row);
    state.take_events();

    state.key(Key::Char('x'));
    let events = state.take_events();

    assert_eq!(events.len(), 1, "只发关闭：{events:?}");
    assert!(
        matches!(&events[0], FrontEndEvent::CloseSession { id } if id.as_str() == "s-a"),
        "关的是焦点行那一个：{events:?}"
    );
}

/// 空宿主时 `Ctrl-X`：不做事、不提示（票 09）。
#[test]
fn ctrl_x_does_nothing_in_an_empty_host() {
    let mut state = fresh();
    load(&mut state, vec![], 0);

    state.key(Key::CtrlX);

    assert!(
        state.take_events().is_empty(),
        "空宿主时 `Ctrl-X` 不发任何请求"
    );
}

/// 会话页里没有焦点行时 `x`：不做事、不提示（票 09）。
#[test]
fn x_does_nothing_without_a_focused_row() {
    let mut state = fresh();
    load(&mut state, vec![], 0);
    open_sessions(&mut state, 120, 24);

    state.key(Key::Char('x'));

    assert!(
        state.take_events().is_empty(),
        "没有焦点行时 `x` 不发任何请求"
    );
}

/// 输入区的 `Ctrl-W`（删一个词）不受影响（票 09）。
#[test]
fn ctrl_w_still_edits_the_input_line() {
    let mut state = two_hosted();
    state.key(Key::Char('h'));
    state.key(Key::Char('i'));
    state.key(Key::CtrlW);

    assert!(
        state.take_events().is_empty(),
        "`Ctrl-W` 是行编辑，不是关闭"
    );
}

/// `x` 不与既有的 `r`（改名）冲突：改名模式里键盘归编辑器，`x` 只是文本（票 09）。
#[test]
fn x_is_text_while_renaming() {
    let mut state = two_hosted();
    open_sessions(&mut state, 120, 24);
    let frame = buffer(120, 24, &mut state);
    let (column, row) = find_leftmost_cell(&frame, 120, 24, "改一下左栏");
    click(&mut state, column, row);
    state.take_events();

    // `r` 进改名模式（键盘归编辑器），此时 `x` 只是名字里的一个字符。
    state.key(Key::Char('r'));
    state.key(Key::Char('x'));

    assert!(state.take_events().is_empty(), "改名模式里 `x` 不关会话");
}

/// 键位提示里写着那两条（票 09 的验收五）。
#[test]
fn the_hint_line_names_both_shortcuts() {
    // `ctrl-x 关闭` 收在提示表末位（位置就是优先级），窄档从末位丢起 —— 要宽屏才看得见。
    assert!(
        heng::render::wording::status_line(false, 200, false).contains("ctrl-x 关闭"),
        "输入区那套提示行"
    );
    // 会话页持有键盘时，提示说的是**这一页**的键位。
    assert!(
        heng::render::wording::sessions_status_line_with(None, false, 200, false)
            .contains("x 关闭"),
        "会话页那套提示行"
    );
}
