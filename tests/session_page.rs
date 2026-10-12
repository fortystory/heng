//! 左栏第一签「会话」：页签、行格式、标记、`＋N 场`、降级阶梯、空态与页内键位
//! （`.scratch/session-tab/spec.md` §3）。
//!
//! 帧级断言照 `tests/render_layout.rs` 的写法：`TestBackend` 上画一帧，把屏幕文本读回来。

use std::time::{Duration, SystemTime};

use heng::config::{DiffViewerSettings, FileViewerSettings};
use heng::events::SessionId;
use heng::render::{
    ConsoleRequest, FrontEndEvent, Key, RenderEvent, SessionFacts, TuiState, draw_frame, wording,
};
use heng::session::listing::{SessionListing, SessionMark, SessionRow};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

fn facts() -> SessionFacts {
    SessionFacts {
        switchable: true,
        session_id: "01J8ZQ4K7M".to_owned(),
        session_dir: "~/code/fortystory/heng".to_owned(),
        model: "claude-sonnet-4-5".to_owned(),
        context_window: 200_000,
        mode: heng::permissions::Mode::Ask,
        budget_limit: None,
        number_style: heng::render::wording::NumberStyle::Cn,
        file_viewer: FileViewerSettings::default(),
        diff_viewer: DiffViewerSettings::default(),
        tool_schemas: Default::default(),
        speaker_order: Vec::new(),
    }
}

/// 每个用例都从干净的一份开始。
fn fresh() -> TuiState {
    TuiState::new(facts(), std::path::PathBuf::from("/x/heng"), None)
}

fn row(id: &str, title: &str, group: &str, mark: SessionMark, minutes: u64) -> SessionRow {
    SessionRow {
        id: SessionId::new(id),
        cwd: Some(format!("/x/{group}")),
        title: title.to_owned(),
        group_key: group.to_owned(),
        group: group.to_owned(),
        written: Some(SystemTime::now() - Duration::from_secs(minutes * 60)),
        broken: false,
        missing_cwd: false,
        foreground: false,
        mark,
    }
}

/// 把一份列表推进去，就像组装方做的那样。
fn load(state: &mut TuiState, rows: Vec<SessionRow>, total: usize) {
    state.request(ConsoleRequest::Sessions {
        listing: SessionListing {
            rows,
            total,
            unreadable: false,
        },
    });
}

fn buffer(width: u16, height: u16, state: &mut TuiState) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("TestBackend");
    terminal
        .draw(|frame| draw_frame(frame, state))
        .expect("画一帧");
    terminal.backend().buffer().clone()
}

/// 一行的文本，按**显示列宽**前进（一个宽字素盖住两格）。
fn row_text(buffer: &Buffer, y: u16, width: u16) -> String {
    let mut out = String::new();
    let mut x = 0u16;
    while x < width {
        let symbol = buffer[(x, y)].symbol();
        out.push_str(symbol);
        x += heng::render::width::text_columns(symbol).max(1) as u16;
    }
    out
}

fn screen(width: u16, height: u16, state: &mut TuiState) -> String {
    let frame = buffer(width, height, state);
    (0..height)
        .map(|y| row_text(&frame, y, width))
        .collect::<Vec<_>>()
        .join("\n")
}

fn find_cell(frame: &Buffer, width: u16, height: u16, needle: &str) -> (u16, u16) {
    for y in 0..height {
        let text = row_text(frame, y, width);
        if let Some(byte) = text.find(needle) {
            let column = heng::render::width::text_columns(&text[..byte]) as u16;
            return (column, y);
        }
    }
    panic!("屏幕上找不到 {needle}");
}

fn click(state: &mut TuiState, column: u16, row: u16) {
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ] {
        state.mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        });
    }
}

/// 点左栏页签条上的 `会话`，把这一页叫出来。
fn open_sessions(state: &mut TuiState, width: u16, height: u16) {
    let frame = buffer(width, height, state);
    let (column, row) = find_cell(&frame, width, height, wording::TAB_SESSIONS);
    click(state, column, row);
}

#[test]
fn the_session_tab_comes_first_and_lists_the_history() {
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
    let text = screen(120, 24, &mut state);

    assert!(text.contains("左栏会话页"), "列表画出来了：{text}");
    // 相对时间：宽档是 `N 分钟前` / `N 小时前`。
    assert!(text.contains("2 分钟前"), "宽档的时间措辞：{text}");
    assert!(text.contains("1 小时前"), "90 分钟就是 1 小时前：{text}");
    // 页顶读数与右端两个入口。
    assert!(text.contains("2 场"), "页顶左端是读数：{text}");
    assert!(text.contains(wording::SESSION_NEW), "右端有新建：{text}");
    assert!(text.contains(wording::SESSION_CLOSE), "右端有关闭：{text}");
}

#[test]
fn the_tab_bar_puts_the_session_label_before_the_others() {
    let mut state = fresh();
    let text = screen(120, 24, &mut state);
    let sessions = text.find(wording::TAB_SESSIONS).expect("页签在");
    let usage = text.find(wording::TAB_USAGE).expect("调用量那一签在");
    assert!(sessions < usage, "会话排在最前：{text}");
}

#[test]
fn the_rows_carry_their_marks() {
    let mut state = fresh();
    let mut front = row("s-front", "当前这一场", "heng", SessionMark::Idle, 1);
    front.foreground = true;
    load(
        &mut state,
        vec![
            front,
            row("s-wait", "等我答的一场", "heng", SessionMark::Awaiting, 3),
            row("s-run", "在跑的一场", "heng", SessionMark::Running, 5),
        ],
        3,
    );
    open_sessions(&mut state, 120, 24);
    let text = screen(120, 24, &mut state);

    assert!(text.contains('▍'), "前台那一行带 `▍`：{text}");
    assert!(text.contains('?'), "待答那一行带 `?`：{text}");
    assert!(text.contains('●'), "活跃那一行带 `●`：{text}");
    // 页顶读数的 `M` 只数在跑的。
    assert!(
        text.contains("3 场 · 1 场在跑"),
        "读数只数在跑的那几场：{text}"
    );
}

#[test]
fn the_foreground_marker_follows_the_switch_not_the_listing_snapshot() {
    let mut state = fresh();
    // 启动那一份 listing 说第一行是前台 —— 它只在启动扫过一次，之后不再重来。
    let mut snapshot = row("01J8ZQ4K7M", "启动那一场", "heng", SessionMark::Idle, 5);
    snapshot.foreground = true;
    load(
        &mut state,
        vec![
            snapshot,
            row("s-other", "刚切过去的一场", "heng", SessionMark::Idle, 1),
        ],
        2,
    );

    // 切到第二场：列表快照不动，换的是渲染侧那个前台（`SwitchSession`）。
    let mut switched = facts();
    switched.session_id = "s-other".to_owned();
    state.request(ConsoleRequest::SwitchSession {
        facts: switched,
        events: Vec::new(),
    });

    open_sessions(&mut state, 120, 24);
    let frame = buffer(120, 24, &mut state);
    let (_, marked) = find_cell(&frame, 120, 24, "▍");
    assert!(
        row_text(&frame, marked, 120).contains("刚切过去的一场"),
        "`▍` 跟着切换走，不停在启动那一份 listing 上"
    );
    for y in 0..24 {
        let line = row_text(&frame, y, 120);
        if line.contains("启动那一场") {
            assert!(!line.contains('▍'), "旧前台不再带 `▍`：{line}");
        }
    }
}

#[test]
fn the_more_line_counts_what_never_showed() {
    let mut state = fresh();
    load(
        &mut state,
        vec![row("s-a", "只露一面的那一场", "heng", SessionMark::Idle, 1)],
        6,
    );
    open_sessions(&mut state, 120, 24);
    let text = screen(120, 24, &mut state);
    assert!(
        text.contains(&wording::session_more(5)),
        "末行说还有 5 场没露面：{text}"
    );
}

#[test]
fn groups_are_whole_rows_and_the_ladder_drops_them() {
    let mut state = fresh();
    load(
        &mut state,
        vec![
            row("s-a", "第一组的一场", "alpha", SessionMark::Idle, 1),
            row("s-b", "第二组的一场", "beta", SessionMark::Idle, 2),
        ],
        2,
    );
    // 页区 ≥8 行：组头与页顶读数都在。
    open_sessions(&mut state, 120, 40);
    let tall = screen(120, 40, &mut state);
    assert!(tall.contains("alpha"), "组头是检出目录名：{tall}");
    assert!(tall.contains("beta"), "另一组也在：{tall}");
    assert!(tall.contains("2 场"), "读数行在：{tall}");

    // 页区 6–7 行：丢组头、保读数行。
    let mut short = fresh();
    short.request(ConsoleRequest::Sessions {
        listing: SessionListing {
            rows: vec![
                row("s-a", "第一组的一场", "alpha", SessionMark::Idle, 1),
                row("s-b", "第二组的一场", "beta", SessionMark::Idle, 2),
            ],
            total: 2,
            unreadable: false,
        },
    });
    open_sessions(&mut short, 120, 24);
    let low = screen(120, 14, &mut short);
    assert!(
        low.contains("第一组的一场") && low.contains("第二组的一场"),
        "丢了组头之后两行都还在：{low}"
    );
    assert!(!low.contains("alpha"), "页区不够 8 行时组头让位：{low}");
}

#[test]
fn the_page_says_which_empty_it_is() {
    let mut state = fresh();
    open_sessions(&mut state, 120, 24);
    let text = screen(120, 24, &mut state);
    assert!(
        text.contains(wording::SESSIONS_EMPTY),
        "store 空的时候说「还没有历史会话」：{text}"
    );

    let mut state = fresh();
    state.request(ConsoleRequest::Sessions {
        listing: SessionListing {
            rows: Vec::new(),
            total: 0,
            unreadable: true,
        },
    });
    open_sessions(&mut state, 120, 24);
    let text = screen(120, 24, &mut state);
    assert!(
        text.contains(wording::SESSIONS_UNREADABLE),
        "读不出是另一句话：{text}"
    );
}

#[test]
fn r_renames_and_enter_reports_the_new_title() {
    let mut state = fresh();
    load(
        &mut state,
        vec![row("s-a", "旧名字", "heng", SessionMark::Idle, 1)],
        1,
    );
    open_sessions(&mut state, 120, 24);
    // 单击一行 = 交键盘 + 落焦点（票 17 的点击语义）。
    let frame = buffer(120, 24, &mut state);
    let (column, row) = find_cell(&frame, 120, 24, "旧名字");
    click(&mut state, column, row);
    state.key(Key::Char('r'));
    // 改名模式里编辑器接着这一行：先清掉旧名字再打新的。
    for _ in 0..6 {
        state.key(Key::Backspace);
    }
    for ch in "新名字".chars() {
        state.key(Key::Char(ch));
    }
    state.key(Key::Enter);

    let reported: Vec<FrontEndEvent> = state
        .take_events()
        .into_iter()
        .filter(|event| matches!(event, FrontEndEvent::SessionTitled { .. }))
        .collect();
    assert_eq!(reported.len(), 1, "改完名上报一条：{reported:?}");
    match &reported[0] {
        FrontEndEvent::SessionTitled { id, title } => {
            assert_eq!(id.as_str(), "s-a");
            assert_eq!(title, "新名字");
        }
        other => panic!("想要一条改名，拿到 {other:?}"),
    }
}

#[test]
fn a_live_event_from_another_session_still_stays_out() {
    // 会话页与前台过滤互不打扰：这一条只是确认注入列表之后渲染通道没被牵连。
    let mut state = fresh();
    load(
        &mut state,
        vec![row("s-a", "一场会话", "heng", SessionMark::Idle, 1)],
        1,
    );
    state.live_event(RenderEvent::identity(
        SessionId::new("01J8ZQ4K7M"),
        "身份".to_owned(),
    ));
    let text = screen(120, 24, &mut state);
    assert!(text.contains(wording::TAB_SESSIONS), "{text}");
}

// ---------------------------------------------------------------------------
// 票 24 — 列表过滤
// ---------------------------------------------------------------------------

/// 进会话页、交键盘、按 `/` 进过滤，再打一串字符（实时重筛）。
fn filter_with(state: &mut TuiState, query: &str) {
    open_sessions(state, 120, 24);
    state.key(Key::Char('/'));
    for ch in query.chars() {
        state.key(Key::Char(ch));
    }
}

#[test]
fn slash_filters_the_list_live_and_the_head_line_becomes_the_filter_line() {
    let mut state = fresh();
    load(
        &mut state,
        vec![
            row("s-a", "左栏会话页", "alpha", SessionMark::Idle, 1),
            row("s-b", "改一下左栏", "beta", SessionMark::Idle, 2),
        ],
        2,
    );
    filter_with(&mut state, "beta");
    let text = screen(120, 24, &mut state);

    assert!(text.contains("改一下左栏"), "命中的那一行还在：{text}");
    assert!(!text.contains("左栏会话页"), "没命中的那一行走了：{text}");
    assert!(
        text.contains(&wording::hit_count(1)),
        "过滤行右端是命中读数：{text}"
    );
    assert!(
        !text.contains(wording::SESSION_NEW),
        "过滤行整行占掉页顶，`新建` 让位：{text}"
    );
    assert!(
        !text.contains(wording::SESSION_CLOSE),
        "`关闭` 也让位：{text}"
    );
    // 过滤行左端是 `会话` + 查询串。
    assert!(text.contains(wording::TAB_SESSIONS), "{text}");
}

#[test]
fn the_criteria_are_title_and_group_case_insensitive_with_and() {
    let mut state = fresh();
    load(
        &mut state,
        vec![
            row("s-a", "左栏会话页", "alpha", SessionMark::Idle, 1),
            row("s-b", "改一下左栏", "alpha", SessionMark::Idle, 2),
        ],
        2,
    );
    // 大写 + 两个词：组头（alpha）与标题（会话）各中一个，两条各判一次。
    filter_with(&mut state, "ALPHA 会话");
    let text = screen(120, 24, &mut state);
    assert!(text.contains("左栏会话页"), "两个词都中的那一行在：{text}");
    assert!(!text.contains("改一下左栏"), "只中一个词的不在：{text}");
    assert!(text.contains(&wording::hit_count(1)), "{text}");
}

#[test]
fn an_empty_result_says_it_twice() {
    let mut state = fresh();
    load(
        &mut state,
        vec![row("s-a", "左栏会话页", "alpha", SessionMark::Idle, 1)],
        1,
    );
    filter_with(&mut state, "没有这个东西");
    let text = screen(120, 24, &mut state);
    assert!(
        text.contains(&wording::hit_count(0)),
        "过滤行写 `0 个命中`：{text}"
    );
    assert!(
        text.contains(wording::SESSIONS_NO_HIT),
        "页区写 `没有命中的会话`：{text}"
    );
}

#[test]
fn groups_survive_the_filter_and_empty_ones_are_not_drawn() {
    let mut state = fresh();
    load(
        &mut state,
        vec![
            row("s-a", "甲组的一场", "alpha", SessionMark::Idle, 2),
            row("s-b", "乙组的一场", "beta", SessionMark::Idle, 1),
        ],
        2,
    );
    filter_with(&mut state, "甲组");
    let text = screen(120, 24, &mut state);
    assert!(text.contains("alpha"), "命中行的组头还在：{text}");
    assert!(!text.contains("beta"), "空组的组头不画：{text}");
    assert!(!text.contains("乙组的一场"), "{text}");
}

#[test]
fn enter_accepts_the_filter_and_escape_unwinds_two_layers() {
    let mut state = fresh();
    load(
        &mut state,
        vec![
            row("s-a", "左栏会话页", "alpha", SessionMark::Idle, 1),
            row("s-b", "改一下左栏", "beta", SessionMark::Idle, 2),
        ],
        2,
    );
    filter_with(&mut state, "beta");
    // `Enter`：接受查询、离开输入态 —— 过滤仍然生效。
    state.key(Key::Enter);
    let text = screen(120, 24, &mut state);
    assert!(!text.contains("左栏会话页"), "过滤还生效：{text}");
    assert!(text.contains("改一下左栏"), "{text}");

    // 已生效但没在输入：这一下 `Esc` 清过滤，键盘**还留在页里**。
    state.key(Key::Esc);
    let text = screen(120, 24, &mut state);
    assert!(text.contains("左栏会话页"), "清过滤之后两条都回来：{text}");
    assert!(text.contains(wording::TAB_SESSIONS), "页还在：{text}");

    // 再一下才把键盘还给输入区：那之后打字落进草稿。
    state.key(Key::Esc);
    state.key(Key::Char('x'));
    let text = screen(120, 24, &mut state);
    assert!(text.contains('x'), "键盘回去了，草稿收下了那个字符：{text}");
}

#[test]
fn the_filter_gives_up_the_pinned_top() {
    // 未过滤：前台所在的组钉组顶。过滤生效之后钉顶让位，排序回到 mtime 降序
    // （决策票 10 §3）。
    let mut state = fresh();
    let mut front = row("s-old", "旧的那一场", "alpha", SessionMark::Idle, 30);
    front.foreground = true;
    load(
        &mut state,
        vec![
            front,
            row("s-new", "新的那一场", "beta", SessionMark::Idle, 1),
        ],
        2,
    );
    open_sessions(&mut state, 120, 24);
    let plain = screen(120, 24, &mut state);
    let old_at = plain.lines().position(|line| line.contains("旧的那一场"));
    let new_at = plain.lines().position(|line| line.contains("新的那一场"));
    assert!(old_at < new_at, "没过滤时前台那一组钉在顶上：{plain}");

    filter_with(&mut state, "一场");
    let filtered = screen(120, 24, &mut state);
    let old_at = filtered
        .lines()
        .position(|line| line.contains("旧的那一场"))
        .expect("命中");
    let new_at = filtered
        .lines()
        .position(|line| line.contains("新的那一场"))
        .expect("命中");
    assert!(
        new_at < old_at,
        "过滤态下按 mtime 降序，钉顶让位：{filtered}"
    );
}

#[test]
fn the_more_line_still_counts_what_this_page_holds() {
    let mut state = fresh();
    load(
        &mut state,
        vec![
            row("s-a", "命中甲", "alpha", SessionMark::Idle, 1),
            row("s-b", "命中乙", "beta", SessionMark::Idle, 2),
            row("s-c", "别的一场", "gamma", SessionMark::Idle, 3),
        ],
        3,
    );
    filter_with(&mut state, "命中");
    let text = screen(120, 24, &mut state);
    assert!(text.contains(&wording::hit_count(2)), "命中两句：{text}");
    assert!(!text.contains("别的一场"), "{text}");
}

/// 某一行的标题是什么颜色：找到含那个标题的那一行，取它第一个字的颜色。
fn title_colour(state: &mut TuiState, title: &str) -> ratatui::style::Color {
    let frame = buffer(120, 24, state);
    let first = title.chars().next().expect("标题非空").to_string();
    for y in 0..24u16 {
        let text = row_text(&frame, y, 120);
        if !text.contains(title) {
            continue;
        }
        for x in 0..120u16 {
            if frame[(x, y)].symbol() == first {
                return frame[(x, y)].fg;
            }
        }
    }
    panic!("{title} 不在屏幕上");
}

#[test]
fn the_title_colour_says_where_a_session_is() {
    // 三档必须一眼分得开（票 29）：**当前前台**、**已装载**（切过去零延迟、`关闭`对它可用）、
    // **只在盘上**（切过去要先装载）。此前「已装载但空闲」与「只在盘上」共用一个空标记，于是
    // 读的人判断不了 `关闭` 对哪一行可用（2026-10-12 维护者报告）。
    let mut state = fresh();
    let mut front = row("s-front", "当前这一场", "heng", SessionMark::Idle, 1);
    front.foreground = true;
    load(
        &mut state,
        vec![
            front,
            row("s-hosted", "已装载空闲", "heng", SessionMark::Idle, 2),
            row("s-cold", "只在盘上", "heng", SessionMark::Idle, 3),
        ],
        3,
    );
    state.request(ConsoleRequest::Hosted {
        ids: vec![SessionId::new("s-front"), SessionId::new("s-hosted")],
    });
    open_sessions(&mut state, 120, 24);

    let live = title_colour(&mut state, "当前这一场");
    let hosted = title_colour(&mut state, "已装载空闲");
    let cold = title_colour(&mut state, "只在盘上");
    assert_ne!(live, hosted, "前台与已装载撞了颜色");
    assert_ne!(hosted, cold, "已装载与只在盘上撞了颜色");
    assert_ne!(live, cold, "前台与只在盘上撞了颜色");
    assert_eq!(live, heng::render::palette::LIVE, "前台那一行");
    assert_eq!(hosted, heng::render::palette::HOSTED, "已装载那一行");
    assert_eq!(cold, heng::render::palette::MUTED, "只在盘上那一行");

    // 关掉一场之后它退回「只在盘上」那一档 —— 盘上那份还在，只是这个进程不再装着它。
    state.request(ConsoleRequest::SessionClosed {
        id: SessionId::new("s-hosted"),
        empty: false,
    });
    open_sessions(&mut state, 120, 24);
    assert_eq!(
        title_colour(&mut state, "已装载空闲"),
        heng::render::palette::MUTED,
        "卸下来之后就是只在盘上了"
    );
}
