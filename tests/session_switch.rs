//! 切换会话：投影整块换、轻状态换入换出、`seq > S` 补齐
//! （`.scratch/session-tab/spec.md` §6、票 19）。

use heng::config::{DiffViewerSettings, FileViewerSettings};
use heng::events::{Event, EventPayload, Role, SessionId, SpeakerId};
use heng::render::{ConsoleRequest, Key, RenderEvent, SessionFacts, TuiState, draw_frame, wording};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;

fn facts(session_id: &str) -> SessionFacts {
    SessionFacts {
        switchable: true,
        session_id: session_id.to_owned(),
        session_dir: format!("~/code/heng/{session_id}"),
        model: "fake-model".to_owned(),
        context_window: 200_000,
        mode: heng::permissions::Mode::Ask,
        budget_limit: None,
        number_style: heng::render::wording::NumberStyle::Cn,
        file_viewer: FileViewerSettings::default(),
        diff_viewer: DiffViewerSettings::default(),
        tool_schemas: Default::default(),
        speaker_order: vec!["kimi".to_owned()],
    }
}

fn fresh() -> TuiState {
    TuiState::new(facts("s-a"), std::path::PathBuf::from("/x/heng"), None)
}

fn said(seq: u64, text: &str) -> Event {
    Event::new(
        seq,
        SpeakerId::Debater("kimi".into()),
        EventPayload::MessageCompleted {
            role: Role::User,
            text: text.to_owned(),
            reasoning: None,
            first_token_ms: None,
        },
    )
}

fn buffer(width: u16, height: u16, state: &mut TuiState) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("TestBackend");
    terminal
        .draw(|frame| draw_frame(frame, state))
        .expect("画一帧");
    terminal.backend().buffer().clone()
}

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

/// 换到 `session_id` 那个会话，并把它的快照铺完。
fn switch(state: &mut TuiState, session_id: &str, events: Vec<Event>) {
    state.request(ConsoleRequest::SwitchSession {
        facts: facts(session_id),
        events,
    });
    while state.replay_pending() {
        state.replay_batch();
    }
}

#[test]
fn switching_replaces_the_whole_projection() {
    let mut state = fresh();
    switch(&mut state, "s-a", vec![said(1, "一号会话说的话")]);
    let text = screen(120, 24, &mut state);
    assert!(text.contains("一号会话说的话"), "{text}");

    switch(&mut state, "s-b", vec![said(1, "二号会话说的话")]);
    let text = screen(120, 24, &mut state);
    assert!(text.contains("二号会话说的话"), "切过去看到的是它：{text}");
    assert!(
        !text.contains("一号会话说的话"),
        "上一个会话的内容一点都不留：{text}"
    );
}

#[test]
fn switching_does_not_insert_the_history_divider() {
    // 切过去看到的是一整个会话，不是「历史 + 本段」。
    let mut state = fresh();
    switch(&mut state, "s-b", vec![said(1, "只有一段")]);
    let text = screen(120, 24, &mut state);
    assert!(
        !text.contains(wording::history_divider()),
        "切换不插接缝：{text}"
    );
}

#[test]
fn the_draft_comes_back_with_its_session() {
    let mut state = fresh();
    switch(&mut state, "s-a", vec![said(1, "甲")]);
    // 在 A 里打一半字。
    state.key(Key::Char('甲'));
    state.key(Key::Char('稿'));

    switch(&mut state, "s-b", vec![said(1, "乙")]);
    let text = screen(120, 24, &mut state);
    assert!(!text.contains("甲稿"), "B 的草稿是空的：{text}");

    // 切回 A：草稿原样还在。
    switch(&mut state, "s-a", vec![said(1, "甲")]);
    let text = screen(120, 24, &mut state);
    assert!(text.contains("甲稿"), "切回来草稿还在：{text}");
}

#[test]
fn a_live_event_of_the_new_session_lands_after_the_snapshot() {
    // 切换期间到达的实时事件（属于**新**那个会话）在收尾时补上，而快照里已有的不画第二遍。
    let mut state = fresh();
    state.request(ConsoleRequest::SwitchSession {
        facts: facts("s-b"),
        events: vec![said(1, "快照里的")],
    });
    state.live_event(RenderEvent::Logged {
        session: SessionId::new("s-b"),
        event: said(1, "快照里的"),
    });
    state.live_event(RenderEvent::Logged {
        session: SessionId::new("s-b"),
        event: said(2, "切换期间新到的"),
    });
    while state.replay_pending() {
        state.replay_batch();
    }

    let text = screen(120, 24, &mut state);
    assert_eq!(
        text.matches("快照里的").count(),
        1,
        "快照已经持有的那条只画一次：{text}"
    );
    assert!(text.contains("切换期间新到的"), "新的那条补上了：{text}");
}

#[test]
fn the_way_out_follows_the_session_that_is_in_front() {
    // 状态行末尾那一段说的是**眼前这个会话**的键盘（`.scratch/session-tab/spec.md` §1、票 23）：
    // 跑着的时候 `Ctrl-C` 取消、`Ctrl-D` 退出，空闲的时候只剩 `Ctrl-D`。
    //
    // 它曾经只读一个进程级的忙闲开关，而那个开关在切换会话之后没人重算 —— 于是切到后台正跑着的
    // 那个会话，状态行按空闲说话，而那时 `Ctrl-C` 明明可用（2026-10-12 维护者报告）。
    let mut state = fresh();
    let idle = screen(120, 24, &mut state);
    assert!(
        idle.contains(wording::EXIT_HINT_IDLE) && !idle.contains("ctrl-c"),
        "空闲那一档只宣传退出：{idle}"
    );

    // 后台那个会话正在跑回合。
    state.request(ConsoleRequest::SessionState {
        id: SessionId::new("s-b"),
        state: heng::session::host::SessionState::RunningTurn,
    });
    let still_idle = screen(120, 24, &mut state);
    assert!(
        still_idle.contains(wording::EXIT_HINT_IDLE),
        "后台那个会话在跑，不改眼前这个会话的提示：{still_idle}"
    );

    // 切到它：出口换成忙碌那一档，两个键各说各的（空闲那一档是它的后缀，所以按整段比而不是
    // 「不含」—— `ctrl-d 退出` 在忙碌那一档里也出现）。
    switch(&mut state, "s-b", vec![said(1, "乙")]);
    let busy = screen(120, 24, &mut state);
    let busy_row = busy
        .lines()
        .find(|row| row.contains("ctrl-d 退出"))
        .expect("提示行在屏幕上");
    assert!(
        busy_row.contains(wording::EXIT_HINT_BUSY),
        "切过去就是忙碌那一档：{busy_row}"
    );

    // 再切回空闲那个会话：提示跟着切回去。
    switch(&mut state, "s-a", vec![said(1, "甲")]);
    let back = screen(120, 24, &mut state);
    assert!(
        back.contains(wording::EXIT_HINT_IDLE) && !back.contains("ctrl-c"),
        "切回来又是空闲那一档：{back}"
    );
}

#[test]
fn a_live_event_of_the_old_session_does_not_leak_into_the_new_one() {
    // 上一个会话攒下的实时事件不带过去（`live_buffer` 在切换时清掉）。
    let mut state = fresh();
    state.request(ConsoleRequest::SwitchSession {
        facts: facts("s-b"),
        events: vec![said(1, "乙")],
    });
    state.live_event(RenderEvent::Notice {
        session: SessionId::new("s-a"),
        at: chrono::Utc::now(),
        message: "上一个会话的一句话".to_owned(),
    });
    while state.replay_pending() {
        state.replay_batch();
    }
    let text = screen(120, 24, &mut state);
    assert!(
        !text.contains("上一个会话的一句话"),
        "上一个会话的不泄过来：{text}"
    );
}
