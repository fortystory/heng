//! 渲染器测试的共用辅助：造一份状态、画一帧、把屏幕读回来、合成指针事件。
//!
//! 帧级断言照 `tests/render_layout.rs` 的写法：`TestBackend` 上画一帧，把屏幕文本读回来。
//! 每个测试文件只用到其中一部分（例如平铺的用例不碰会话页的助手），所以整份模块放开
//! 「未用」告警 —— 否则每个文件都得抄一份才能消掉。

#![allow(dead_code)]

use std::time::{Duration, SystemTime};

use heng::config::{DiffViewerSettings, FileViewerSettings};
use heng::events::SessionId;
use heng::render::{ConsoleRequest, SessionFacts, TuiState, draw_frame, width::text_columns};
use heng::session::listing::{SessionListing, SessionMark, SessionRow};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

pub fn facts() -> SessionFacts {
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
pub fn fresh() -> TuiState {
    TuiState::new(facts(), std::path::PathBuf::from("/x/heng"), None)
}

pub fn row(id: &str, title: &str, group: &str, mark: SessionMark, minutes: u64) -> SessionRow {
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
pub fn load(state: &mut TuiState, rows: Vec<SessionRow>, total: usize) {
    state.request(ConsoleRequest::Notify(
        heng::render::ConsoleNotify::Sessions {
            listing: SessionListing {
                rows,
                total,
                unreadable: false,
            },
        },
    ));
}

/// 推一份宿主表事实：那几个会话装载着、谁是前台。
pub fn host(state: &mut TuiState, foreground: &str, sessions: &[&str]) {
    state.request(ConsoleRequest::Facts(heng::session::HostFacts {
        foreground: SessionId::new(foreground),
        sessions: sessions
            .iter()
            .map(|id| heng::session::HostedSession {
                id: SessionId::new(*id),
                state: heng::session::SessionState::Idle,
                read_only: false,
            })
            .collect(),
        waiting: 0,
        finished: 0,
    }));
}

/// 推一份宿主表事实，其中一个会话**正在跑**。
pub fn host_running(state: &mut TuiState, foreground: &str, sessions: &[&str], running: &str) {
    state.request(ConsoleRequest::Facts(heng::session::HostFacts {
        foreground: SessionId::new(foreground),
        sessions: sessions
            .iter()
            .map(|id| heng::session::HostedSession {
                id: SessionId::new(*id),
                state: if *id == running {
                    heng::session::SessionState::RunningTurn
                } else {
                    heng::session::SessionState::Idle
                },
                read_only: false,
            })
            .collect(),
        waiting: 0,
        finished: 0,
    }));
}

/// 推一份宿主表事实，其中一个会话是**只读预览**。
pub fn host_read_only(state: &mut TuiState, foreground: &str, sessions: &[(&str, bool)]) {
    state.request(ConsoleRequest::Facts(heng::session::HostFacts {
        foreground: SessionId::new(foreground),
        sessions: sessions
            .iter()
            .map(|(id, read_only)| heng::session::HostedSession {
                id: SessionId::new(*id),
                state: heng::session::SessionState::Idle,
                read_only: *read_only,
            })
            .collect(),
        waiting: 0,
        finished: 0,
    }));
}

/// 把渲染侧的**前台**切到某一个会话（就像循环层发来的那次切换）。
///
/// 宿主表事实只说「谁在表里」，前台指针是切换通知推的 —— 两件不同的事。
pub fn switch_front(state: &mut TuiState, id: &str) {
    let mut switched = facts();
    switched.session_id = id.to_owned();
    state.request(ConsoleRequest::Notify(
        heng::render::ConsoleNotify::SwitchSession {
            facts: switched,
            cwd: std::path::PathBuf::from("/x/heng"),
            events: Vec::new(),
        },
    ));
}

pub fn buffer(width: u16, height: u16, state: &mut TuiState) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("TestBackend");
    terminal
        .draw(|frame| draw_frame(frame, state))
        .expect("画一帧");
    terminal.backend().buffer().clone()
}

/// 一行的文本，按**显示列宽**前进（一个宽字素盖住两格）。
pub fn row_text(buffer: &Buffer, y: u16, width: u16) -> String {
    let mut out = String::new();
    let mut x = 0u16;
    while x < width {
        let symbol = buffer[(x, y)].symbol();
        out.push_str(symbol);
        x += text_columns(symbol).max(1) as u16;
    }
    out
}

pub fn screen(width: u16, height: u16, state: &mut TuiState) -> String {
    let frame = buffer(width, height, state);
    (0..height)
        .map(|y| row_text(&frame, y, width))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 某一**屏幕行**里那段文字的起始列（列按显示宽度算）。
///
/// 与 [`find_cell`] 的区别：它只在指定的那一行里找 —— 页签条与格标题行都在第 0 行，
/// 满屏找会撞上左栏会话页里的同名会话。
pub fn in_row(frame: &Buffer, row: u16, width: u16, needle: &str) -> (u16, u16) {
    let line = row_text(frame, row, width);
    let byte = line
        .find(needle)
        .unwrap_or_else(|| panic!("第 {row} 行没有 {needle}：{line}"));
    (text_columns(&line[..byte]) as u16, row)
}

/// 屏幕上那段文字的左端列与它所在的那一行。
pub fn find_cell(frame: &Buffer, width: u16, height: u16, needle: &str) -> (u16, u16) {
    find_cell_below(frame, width, height, needle, 0)
}

/// 与 [`find_cell`] 同一条路，只是从屏幕第 `from` 行起找 —— 顶部页签条也会写标题里的字。
pub fn find_cell_below(
    frame: &Buffer,
    width: u16,
    height: u16,
    needle: &str,
    from: u16,
) -> (u16, u16) {
    for y in from..height {
        let text = row_text(frame, y, width);
        if let Some(byte) = text.find(needle) {
            let column = text_columns(&text[..byte]) as u16;
            return (column, y);
        }
    }
    panic!("屏幕上找不到 {needle}");
}

/// 同一段文字**最靠左**的那一次出现。
///
/// 同一个会话名在两处都有：顶部页签条的标签与左栏会话页的那一行。找左栏那一处时用它 ——
/// [`find_cell`] 会先撞上页签条。
pub fn find_leftmost_cell(frame: &Buffer, width: u16, height: u16, needle: &str) -> (u16, u16) {
    let mut found: Option<(u16, u16)> = None;
    for y in 0..height {
        let text = row_text(frame, y, width);
        let Some(byte) = text.find(needle) else {
            continue;
        };
        let column = text_columns(&text[..byte]) as u16;
        match found {
            Some((left, _)) if left <= column => {}
            _ => found = Some((column, y)),
        }
    }
    found.unwrap_or_else(|| panic!("屏幕上找不到 {needle}"))
}

/// 点一下：按下与抬起。
pub fn click(state: &mut TuiState, column: u16, row: u16) {
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

/// 按下左键。
pub fn press(state: &mut TuiState, column: u16, row: u16) {
    state.mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    });
}

/// 拖到某处（按住左键移动）。
pub fn drag_to(state: &mut TuiState, column: u16, row: u16) {
    state.mouse(MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    });
}

/// 抬起左键。
pub fn release(state: &mut TuiState, column: u16, row: u16) {
    state.mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    });
}

/// 把指针挪到某处 —— 终端报的是「移动」，不是按下。
pub fn hover(state: &mut TuiState, column: u16, row: u16) {
    state.mouse(MouseEvent {
        kind: MouseEventKind::Moved,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    });
}

/// 点左栏页签条上的 `会话`，把这一页叫出来。
pub fn open_sessions(state: &mut TuiState, width: u16, height: u16) {
    let frame = buffer(width, height, state);
    let (column, row) = find_cell_below(
        &frame,
        width,
        height,
        heng::render::wording::TAB_SESSIONS,
        2,
    );
    click(state, column, row);
}
