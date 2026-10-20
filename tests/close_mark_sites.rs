//! 另外两处落点的**关闭标记**：顶部页签条的标签上、平铺态的格标题行上。
//!
//! 票 [`07`](../../.scratch/session-close-button/issues/07-close-button-in-tab-strip.md)、
//! [`08`](../../.scratch/session-close-button/issues/08-close-button-in-tile-header.md)；
//! 形状与列账在[形态票](../../.scratch/session-close-button/issues/02-prototype-close-button-shapes.md)。

mod common;

use common::{buffer, click, fresh, host, hover, in_row, load, row, row_text, switch_front};
use heng::render::{FrontEndEvent, Key};
use heng::session::listing::SessionMark;
use ratatui::style::{Color, Modifier};

/// 三个会话都在宿主表里：页签条上有三个标签。
fn three_hosted() -> heng::render::TuiState {
    let mut state = fresh();
    load(
        &mut state,
        vec![
            row("s-b", "左栏会话页", "heng", SessionMark::Idle, 2),
            row("s-a", "改一下左栏", "heng", SessionMark::Idle, 90),
            row("s-c", "丙会话", "heng", SessionMark::Idle, 120),
        ],
        3,
    );
    host(&mut state, "s-b", &["s-b", "s-a", "s-c"]);
    state
}

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

/// 指针悬在顶部页签条的某个标签上 → 那个标签的标记位出现红色 `✖`（票 07）。
#[test]
fn the_hovered_top_tab_shows_a_close_mark() {
    let mut state = three_hosted();
    let frame = buffer(120, 24, &mut state);
    let (column, _) = in_row(&frame, 0, 120, "改一下左栏");
    // 标签形如 `[ 标题]`：标记位在标题前两列（`[` + 标记位 + 空格）。
    let mark_at = column - 2;
    assert_eq!(frame[(mark_at, 0)].symbol(), " ", "悬浮之前没有标记");

    hover(&mut state, column, 0);
    let frame = buffer(120, 24, &mut state);

    assert_eq!(frame[(mark_at, 0)].symbol(), "✖", "标记位是 ✖");
    assert_eq!(frame[(mark_at, 0)].fg, Color::Red, "标记是红的");
    let (after, _) = in_row(&frame, 0, 120, "改一下左栏");
    assert_eq!(after, column, "标题没有挪位置");
}

/// 标签**宽度不变** → 后面的标签起点与不悬浮时一字不差（零抖动，票 07）。
#[test]
fn the_top_tabs_do_not_shift_when_hovered() {
    let mut state = three_hosted();
    let frame = buffer(120, 24, &mut state);
    let (before, _) = in_row(&frame, 0, 120, "丙会话");

    let (column, _) = in_row(&frame, 0, 120, "改一下左栏");
    hover(&mut state, column, 0);
    let frame = buffer(120, 24, &mut state);

    let (after, _) = in_row(&frame, 0, 120, "丙会话");
    assert_eq!(after, before, "悬浮不抖：后面的标签起点一个像素都不动");
}

/// 点页签条上的标记 → 关掉**那一个**会话（不是焦点标签那一个，票 07）。
#[test]
fn clicking_the_top_tab_mark_closes_that_session() {
    let mut state = three_hosted();
    switch_front(&mut state, "s-b");
    let frame = buffer(120, 24, &mut state);
    let (column, _) = in_row(&frame, 0, 120, "改一下左栏");
    let mark_at = column - 2;
    hover(&mut state, column, 0);
    let _ = buffer(120, 24, &mut state);

    click(&mut state, mark_at, 0);
    let events = state.take_events();

    assert_eq!(events.len(), 1, "只发关闭：{events:?}");
    assert!(
        matches!(&events[0], FrontEndEvent::CloseSession { id } if id.as_str() == "s-a"),
        "关的是被悬着的那个标签：{events:?}"
    );
}

/// 点**前台**那个标签的标记 → 按票 06 的规则**先切到相邻、再关**（票 07 + 06）。
#[test]
fn clicking_the_front_tab_mark_switches_first() {
    let mut state = three_hosted();
    switch_front(&mut state, "s-b");
    let frame = buffer(120, 24, &mut state);
    let (column, _) = in_row(&frame, 0, 120, "左栏会话页");
    let mark_at = column - 2;
    hover(&mut state, column, 0);
    let _ = buffer(120, 24, &mut state);

    click(&mut state, mark_at, 0);
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

/// 焦点标记 `◆` 退场 —— 哪一签是前台只靠 **ACCENT + 粗体**认（票 07）。
#[test]
fn the_front_tab_is_accent_and_bold() {
    let mut state = three_hosted();
    switch_front(&mut state, "s-b");
    let frame = buffer(120, 24, &mut state);

    assert!(
        !row_text(&frame, 0, 120).contains('◆'),
        "焦点那个字形退场了：{}",
        row_text(&frame, 0, 120)
    );
    let (column, _) = in_row(&frame, 0, 120, "左栏会话页");
    assert_eq!(
        frame[(column, 0)].fg,
        Color::LightMagenta,
        "前台那签是 ACCENT"
    );
    assert!(
        frame[(column, 0)].modifier.contains(Modifier::BOLD),
        "前台那签是粗体"
    );
}

/// 标签多到收成 `+N` 的那一档同样成立 —— `+N` 仍不可点（票 07）。
#[test]
fn the_more_slot_stays_unclickable() {
    let mut state = fresh();
    let rows: Vec<heng::session::listing::SessionRow> = (0..6)
        .map(|index| {
            row(
                &format!("s-{index}"),
                &format!("第几个会话 {index}"),
                "heng",
                SessionMark::Idle,
                index,
            )
        })
        .collect();
    load(&mut state, rows, 6);
    let hosted: Vec<&str> = (0..6)
        .map(|index| Box::leak(format!("s-{index}").into_boxed_str()) as &str)
        .collect();
    host(&mut state, "s-0", &hosted);
    let frame = buffer(120, 24, &mut state);
    let text = row_text(&frame, 0, 120);
    assert!(text.contains('+'), "标签收起来了：{text}");

    // 点 `+N` 那一段：什么也不发生。
    let (column, _) = in_row(&frame, 0, 120, "+");
    click(&mut state, column, 0);
    assert!(state.take_events().is_empty(), "`+N` 不可点");
}

/// 指针悬在平铺态某一格的标题行上 → 那一格的标记位出现红色 `✖`（票 08）。
#[test]
fn the_hovered_tile_shows_a_close_mark() {
    let mut state = two_hosted();
    switch_front(&mut state, "s-b");
    // 先有一帧（平铺按这一帧的尺寸算放不放得下），再进平铺。
    let _ = buffer(120, 40, &mut state);
    state.key(Key::CtrlR);
    let frame = buffer(120, 40, &mut state);
    let (column, _) = in_row(&frame, 0, 120, "改一下左栏");
    // 格标题行：`  标题`（标记位 + 空格 + 标题），标题在格内第 4 列。
    let mark_at = column - 2;
    assert_eq!(frame[(mark_at, 0)].symbol(), " ", "悬浮之前没有标记");

    hover(&mut state, column, 0);
    let frame = buffer(120, 40, &mut state);

    assert_eq!(frame[(mark_at, 0)].symbol(), "✖", "标记位是 ✖");
    assert_eq!(frame[(mark_at, 0)].fg, Color::Red, "标记是红的");
    let (after, _) = in_row(&frame, 0, 120, "改一下左栏");
    assert_eq!(after, column, "标题没有挪位置");
}

/// 点格标题行上的标记 → 关掉**那一个**会话（票 08）。
#[test]
fn clicking_the_tile_mark_closes_that_session() {
    let mut state = two_hosted();
    switch_front(&mut state, "s-b");
    let _ = buffer(120, 40, &mut state);
    state.key(Key::CtrlR);
    let frame = buffer(120, 40, &mut state);
    let (column, _) = in_row(&frame, 0, 120, "改一下左栏");
    let mark_at = column - 2;
    hover(&mut state, column, 0);
    let _ = buffer(120, 40, &mut state);

    click(&mut state, mark_at, 0);
    let events = state.take_events();

    assert_eq!(events.len(), 1, "只发关闭：{events:?}");
    assert!(
        matches!(&events[0], FrontEndEvent::CloseSession { id } if id.as_str() == "s-a"),
        "关的是被悬着的那个格：{events:?}"
    );
}

/// 焦点标记 `◆` / `·` 退场 —— 焦点格靠 **ACCENT + 粗体**认（票 08）。
#[test]
fn the_focus_tile_is_accent_and_bold() {
    let mut state = two_hosted();
    switch_front(&mut state, "s-b");
    let _ = buffer(120, 40, &mut state);
    state.key(Key::CtrlR);
    let frame = buffer(120, 40, &mut state);
    let text = row_text(&frame, 0, 120);

    assert!(!text.contains('◆'), "焦点那个字形退场了：{text}");
    assert!(!text.contains('·'), "其余那枚也退场了：{text}");
    let (column, _) = in_row(&frame, 0, 120, "左栏会话页");
    assert_eq!(
        frame[(column, 0)].fg,
        Color::LightMagenta,
        "焦点格是 ACCENT"
    );
    assert!(
        frame[(column, 0)].modifier.contains(Modifier::BOLD),
        "焦点格是粗体"
    );
}

/// 关掉焦点格 → 焦点格落到相邻格（票 08 + 06）。
#[test]
fn the_focus_tile_lands_on_the_neighbour() {
    let mut state = two_hosted();
    switch_front(&mut state, "s-b");
    let _ = buffer(120, 40, &mut state);
    state.key(Key::CtrlR);
    let _ = buffer(120, 40, &mut state);

    // 循环层执行「先切到相邻」：前台换成 s-a。
    switch_front(&mut state, "s-a");
    let frame = buffer(120, 40, &mut state);

    let (column, _) = in_row(&frame, 0, 120, "改一下左栏");
    assert_eq!(
        frame[(column, 0)].fg,
        Color::LightMagenta,
        "焦点格落到相邻格（新的前台那一个）"
    );
}
