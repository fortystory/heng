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
