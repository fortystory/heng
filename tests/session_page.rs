//! 左栏第一签「会话」：页签、行格式、标记、`＋N 个会话`、降级阶梯、空态与页内键位
//! （`.scratch/session-tab/spec.md` §3）。
//!
//! 帧级断言照 `tests/render_layout.rs` 的写法：`TestBackend` 上画一帧，把屏幕文本读回来。

mod common;

use common::{
    buffer, click, facts, find_cell, find_leftmost_cell, fresh, load, open_sessions, row, row_text,
    screen,
};
use heng::events::SessionId;
use heng::render::{ConsoleRequest, FrontEndEvent, Key, RenderEvent, TuiState, wording};
use heng::session::listing::{SessionListing, SessionMark};
use ratatui::buffer::Buffer;

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
    // 页顶读数与右端那个入口（票 05：`关闭` 退场，关闭改成每行一个标记）。
    assert!(text.contains("已装载 2 个"), "页顶左端是读数：{text}");
    assert!(text.contains(wording::SESSION_NEW), "右端有新建：{text}");
    assert!(!text.contains("关闭"), "页顶没有关闭入口了：{text}");
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
    let mut front = row("s-front", "当前这个会话", "heng", SessionMark::Idle, 1);
    front.foreground = true;
    load(
        &mut state,
        vec![
            front,
            row(
                "s-wait",
                "等我答的一个会话",
                "heng",
                SessionMark::Awaiting,
                3,
            ),
            row("s-run", "在跑的一个会话", "heng", SessionMark::Running, 5),
        ],
        3,
    );
    open_sessions(&mut state, 120, 24);
    let text = screen(120, 24, &mut state);

    assert!(!text.contains('▍'), "前台那个字形退场了（票 05）：{text}");
    assert!(text.contains('?'), "待答那一行带 `?`：{text}");
    assert!(text.contains('●'), "活跃那一行带 `●`：{text}");
    // 页顶读数的 `M` 只数在跑的。
    assert!(
        text.contains("已装载 3 个 · 1 个在跑"),
        "读数只数在跑的那几个：{text}"
    );
}

#[test]
fn the_foreground_marker_follows_the_switch_not_the_listing_snapshot() {
    let mut state = fresh();
    // 启动那一份 listing 说第一行是前台 —— 它只在启动扫过一次，之后不再重来。
    let mut snapshot = row("01J8ZQ4K7M", "启动那个会话", "heng", SessionMark::Idle, 5);
    snapshot.foreground = true;
    load(
        &mut state,
        vec![
            snapshot,
            row(
                "s-other",
                "刚切过去的一个会话",
                "heng",
                SessionMark::Idle,
                1,
            ),
        ],
        2,
    );

    // 切到第二个会话：列表快照不动，换的是渲染侧那个前台（`SwitchSession`）。
    let mut switched = facts();
    switched.session_id = "s-other".to_owned();
    state.request(ConsoleRequest::Notify(
        heng::render::ConsoleNotify::SwitchSession {
            facts: switched,
            cwd: std::path::PathBuf::from("/x/heng"),
            events: Vec::new(),
        },
    ));

    open_sessions(&mut state, 120, 24);
    let frame = buffer(120, 24, &mut state);
    // `▍` 已退场（票 05）—— 前台由**标题色**说：切换后的那一个是 `LIVE`（绿），启动
    // 快照那一行只是「已装载」（`HOSTED`，蓝）。跟的是切换，不停在启动那一份 listing 上。
    let (column, row) = find_leftmost_cell(&frame, 120, 24, "刚切过去的一个会话");
    assert_eq!(
        fg_of(&frame, column, row),
        ratatui::style::Color::LightGreen,
        "切过去的那个会话是前台的"
    );
    let (column, row) = find_leftmost_cell(&frame, 120, 24, "启动那个会话");
    assert_eq!(
        fg_of(&frame, column, row),
        ratatui::style::Color::DarkGray,
        "启动快照那一行不是前台了（它只在盘上，是 `MUTED`）"
    );
    for y in 0..24 {
        let line = row_text(&frame, y, 120);
        if line.contains("启动那个会话") {
            assert!(!line.contains('▍'), "旧前台不再带 `▍`：{line}");
        }
    }
}

#[test]
fn the_list_scrolls_instead_of_counting_what_never_showed() {
    // 会话页有了视口：装不下的那几条靠滚轮够到，「＋N 个会话」那句退场
    // （`.scratch/wheel-routing/spec.md` §4、票 04）。
    let mut state = fresh();
    let rows = (0..30)
        .map(|index| {
            row(
                &format!("s-{index:02}"),
                &format!("会话 {index:02}"),
                "heng",
                SessionMark::Idle,
                index + 1,
            )
        })
        .collect();
    load(&mut state, rows, 30);
    open_sessions(&mut state, 120, 24);
    let text = screen(120, 24, &mut state);
    assert!(!text.contains('＋'), "不再有溢出读数：{text}");
    assert!(text.contains("会话 00"), "顶上第一条在：{text}");
    assert!(!text.contains("会话 20"), "装不下的那条不在屏幕上：{text}");
}

#[test]
fn groups_are_whole_rows_and_the_ladder_drops_them() {
    let mut state = fresh();
    load(
        &mut state,
        vec![
            row("s-a", "第一组的一个会话", "alpha", SessionMark::Idle, 1),
            row("s-b", "第二组的一个会话", "beta", SessionMark::Idle, 2),
        ],
        2,
    );
    // 页区 ≥8 行：组头与页顶读数都在。
    open_sessions(&mut state, 120, 40);
    let tall = screen(120, 40, &mut state);
    assert!(tall.contains("alpha"), "组头是检出目录名：{tall}");
    assert!(tall.contains("beta"), "另一组也在：{tall}");
    assert!(tall.contains("已装载 2 个"), "读数行在：{tall}");

    // 页区 6–7 行：丢组头、保读数行。
    let mut short = fresh();
    short.request(ConsoleRequest::Notify(
        heng::render::ConsoleNotify::Sessions {
            listing: SessionListing {
                rows: vec![
                    row("s-a", "第一组的一个会话", "alpha", SessionMark::Idle, 1),
                    row("s-b", "第二组的一个会话", "beta", SessionMark::Idle, 2),
                ],
                total: 2,
                unreadable: false,
            },
        },
    ));
    open_sessions(&mut short, 120, 24);
    let low = screen(120, 14, &mut short);
    assert!(
        low.contains("第一组的一个会话") && low.contains("第二组的一个会话"),
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
    state.request(ConsoleRequest::Notify(
        heng::render::ConsoleNotify::Sessions {
            listing: SessionListing {
                rows: Vec::new(),
                total: 0,
                unreadable: true,
            },
        },
    ));
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
        vec![row("s-a", "一个会话", "heng", SessionMark::Idle, 1)],
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
    assert!(!text.contains("关闭"), "`关闭` 让位（它已经退场）：{text}");
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
            row("s-a", "甲组的一个会话", "alpha", SessionMark::Idle, 2),
            row("s-b", "乙组的一个会话", "beta", SessionMark::Idle, 1),
        ],
        2,
    );
    filter_with(&mut state, "甲组");
    let text = screen(120, 24, &mut state);
    assert!(text.contains("alpha"), "命中行的组头还在：{text}");
    assert!(!text.contains("beta"), "空组的组头不画：{text}");
    assert!(!text.contains("乙组的一个会话"), "{text}");
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
    let mut front = row("s-old", "旧的那个会话", "alpha", SessionMark::Idle, 30);
    front.foreground = true;
    load(
        &mut state,
        vec![
            front,
            row("s-new", "新的那个会话", "beta", SessionMark::Idle, 1),
        ],
        2,
    );
    open_sessions(&mut state, 120, 24);
    let plain = screen(120, 24, &mut state);
    let old_at = plain.lines().position(|line| line.contains("旧的那个会话"));
    let new_at = plain.lines().position(|line| line.contains("新的那个会话"));
    assert!(old_at < new_at, "没过滤时前台那一组钉在顶上：{plain}");

    filter_with(&mut state, "会话");
    let filtered = screen(120, 24, &mut state);
    let old_at = filtered
        .lines()
        .position(|line| line.contains("旧的那个会话"))
        .expect("命中");
    let new_at = filtered
        .lines()
        .position(|line| line.contains("新的那个会话"))
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
            row("s-c", "别的一个会话", "gamma", SessionMark::Idle, 3),
        ],
        3,
    );
    filter_with(&mut state, "命中");
    let text = screen(120, 24, &mut state);
    assert!(text.contains(&wording::hit_count(2)), "命中两句：{text}");
    assert!(!text.contains("别的一个会话"), "{text}");
}

/// 某一行的标题是什么颜色：找到含那个标题的那一行，取它第一个字的颜色。
fn title_colour(state: &mut TuiState, title: &str) -> ratatui::style::Color {
    let frame = buffer(120, 24, state);
    let first = title.chars().next().expect("标题非空").to_string();
    // 顶部页签条（票 15）也写标题，但那是另一种颜色语义；要找的是左栏列表行。
    for y in 2..24u16 {
        let text = row_text(&frame, y, 120);
        // 页顶那一行是读数（票 05 起页顶没有 `关闭` 了）—— 要读的是**列表行**那一档色。
        if !text.contains(title) || text.contains("已装载") {
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
    let mut front = row("s-front", "当前这个会话", "heng", SessionMark::Idle, 1);
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
    let foreground = state.front().clone();
    state.request(ConsoleRequest::Facts(heng::session::HostFacts {
        foreground,
        sessions: vec![
            heng::session::HostedSession {
                id: SessionId::new("s-front"),
                state: heng::session::host::SessionState::Idle,
                read_only: false,
            },
            heng::session::HostedSession {
                id: SessionId::new("s-hosted"),
                state: heng::session::host::SessionState::Idle,
                read_only: false,
            },
        ],
        waiting: 0,
        finished: 0,
    }));
    open_sessions(&mut state, 120, 24);

    let live = title_colour(&mut state, "当前这个会话");
    let hosted = title_colour(&mut state, "已装载空闲");
    let cold = title_colour(&mut state, "只在盘上");
    assert_ne!(live, hosted, "前台与已装载撞了颜色");
    assert_ne!(hosted, cold, "已装载与只在盘上撞了颜色");
    assert_ne!(live, cold, "前台与只在盘上撞了颜色");
    assert_eq!(live, heng::render::palette::LIVE, "前台那一行");
    assert_eq!(hosted, heng::render::palette::HOSTED, "已装载那一行");
    assert_eq!(cold, heng::render::palette::MUTED, "只在盘上那一行");

    // 关掉一个会话之后它退回「只在盘上」那一档 —— 盘上那份还在，只是这个进程不再装着它。
    let foreground = state.front().clone();
    state.request(ConsoleRequest::Facts(heng::session::HostFacts {
        foreground: foreground.clone(),
        sessions: vec![heng::session::HostedSession {
            id: SessionId::new("s-front"),
            state: heng::session::host::SessionState::Idle,
            read_only: false,
        }],
        waiting: 0,
        finished: 0,
    }));
    open_sessions(&mut state, 120, 24);
    assert_eq!(
        title_colour(&mut state, "已装载空闲"),
        heng::render::palette::MUTED,
        "卸下来之后就是只在盘上了"
    );
}

#[test]
fn the_head_line_keeps_both_entries_in_the_narrow_sidebar() {
    let mut state = fresh();
    load(
        &mut state,
        vec![row("s-a", "一个会话", "heng", SessionMark::Idle, 1)],
        1,
    );
    // 窄档：整屏 80 列 → 左栏 28 列（`layout::SIDEBAR_NARROW_FROM`）。读数里那句「已装载」
    // 放不下，它会挤掉右端的两个入口（票 09 的宽度复核）。
    open_sessions(&mut state, 80, 24);
    let text = screen(80, 24, &mut state);

    assert!(
        text.contains(wording::SESSION_NEW),
        "窄档下 `新建` 仍在：{text}"
    );
    assert!(
        !text.contains("关闭"),
        "窄档下页顶也没有 `关闭`（票 05）：{text}"
    );
    assert!(
        !text.contains("已装载"),
        "窄档只报数，不写「已装载」：{text}"
    );
}

/// 票 32：单击只落焦点（切前台改由 `Enter` 与双击承担）。
#[test]
fn a_single_click_only_moves_the_focus() {
    let mut state = fresh();
    load(
        &mut state,
        vec![
            row("s-a", "甲会话", "heng", SessionMark::Idle, 1),
            row("s-b", "乙会话", "heng", SessionMark::Idle, 2),
        ],
        2,
    );
    open_sessions(&mut state, 120, 24);
    let frame = buffer(120, 24, &mut state);
    let (column, line) = find_cell(&frame, 120, 24, "乙会话");
    click(&mut state, column, line);

    assert_eq!(state.take_events(), Vec::new(), "单击不发切换请求");
    // 焦点确实落在那一行：`Enter` 切的就是它。
    state.key(Key::Enter);
    assert_eq!(
        state.take_events(),
        vec![FrontEndEvent::OpenSession {
            id: SessionId::new("s-b")
        }],
        "`Enter` 切到焦点行"
    );
}

/// 票 32：同一行的第二次点击（时间窗内）才切前台。
#[test]
fn a_double_click_on_the_same_row_switches() {
    let mut state = fresh();
    load(
        &mut state,
        vec![
            row("s-a", "甲会话", "heng", SessionMark::Idle, 1),
            row("s-b", "乙会话", "heng", SessionMark::Idle, 2),
        ],
        2,
    );
    open_sessions(&mut state, 120, 24);
    let frame = buffer(120, 24, &mut state);
    let (column, line) = find_cell(&frame, 120, 24, "乙会话");

    click(&mut state, column, line);
    assert_eq!(state.take_events(), Vec::new(), "第一下只落焦点");
    click(&mut state, column, line);
    assert_eq!(
        state.take_events(),
        vec![FrontEndEvent::OpenSession {
            id: SessionId::new("s-b")
        }],
        "第二下切过去"
    );
}

/// 票 05：窄档页顶只剩 `新建` —— 那个 `关闭` 入口退场了。
#[test]
fn the_narrow_head_line_keeps_the_new_entry() {
    let mut state = fresh();
    let mut front = row("01J8ZQ4K7M", "前台这一个", "heng", SessionMark::Idle, 1);
    front.foreground = true;
    load(
        &mut state,
        vec![
            front,
            row("s-b", "另一个会话", "heng", SessionMark::Idle, 2),
        ],
        2,
    );
    open_sessions(&mut state, 80, 24);
    let frame = buffer(80, 24, &mut state);
    let (column, line) = find_cell(&frame, 80, 24, "另一个会话");
    click(&mut state, column, line);

    let text = screen(80, 24, &mut state);
    assert!(
        text.contains(wording::SESSION_NEW),
        "窄档下 `新建` 仍在：{text}"
    );
    assert!(!text.contains("关闭"), "窄档下页顶也没有 `关闭`：{text}");
}

/// 某一格的前景色。
fn fg_of(frame: &Buffer, column: u16, row: u16) -> ratatui::style::Color {
    frame[(column, row)].fg
}

/// 票 32：前台被拒时提示行给一段可点的，点它把前台切到相邻的那一个。
#[test]
fn a_refused_close_offers_a_clickable_switch() {
    let mut state = fresh();
    let mut front = row("01J8ZQ4K7M", "前台这一个", "heng", SessionMark::Idle, 1);
    front.foreground = true;
    load(
        &mut state,
        vec![
            front,
            row("s-b", "另一个会话", "heng", SessionMark::Idle, 2),
        ],
        2,
    );
    open_sessions(&mut state, 120, 24);
    // 循环判出「要关的这一个正在前台」，把目标告诉渲染器；相邻是谁由它自己算。
    state.request(ConsoleRequest::Notify(
        heng::render::ConsoleNotify::CloseBlocked {
            target: SessionId::new("01J8ZQ4K7M"),
        },
    ));

    let frame = buffer(120, 24, &mut state);
    let (column, line) = find_cell(&frame, 120, 24, "切到「另一个会话」");
    click(&mut state, column, line);
    assert_eq!(
        state.take_events(),
        vec![FrontEndEvent::OpenSession {
            id: SessionId::new("s-b")
        }],
        "点那段可点的 = 把前台切到相邻的那一个"
    );
}

// ---------------------------------------------------------------------------
// 只读预览是**每会话**的（票 25）
// ---------------------------------------------------------------------------

/// 只读预览里会话页的改名被挡住并说一句。
#[test]
fn a_read_only_session_blocks_the_rename() {
    let mut state = fresh();
    let foreground = state.front().clone();
    state.request(ConsoleRequest::Facts(
        heng::session::HostFacts::foreground_read_only(foreground, true),
    ));
    open_sessions(&mut state, 120, 24);
    state.key(Key::Char('r'));
    let rows = screen(120, 24, &mut state);
    assert!(
        rows.contains(&wording::read_only_session()),
        "只读会话里改名被拒：{rows}"
    );
}

/// 普通会话里同一个键照旧进改名。
#[test]
fn a_normal_session_still_enters_the_rename() {
    let mut state = fresh();
    open_sessions(&mut state, 120, 24);
    state.key(Key::Char('r'));
    let rows = screen(120, 24, &mut state);
    assert!(
        !rows.contains(&wording::read_only_session()),
        "普通会话不被只读挡：{rows}"
    );
}
