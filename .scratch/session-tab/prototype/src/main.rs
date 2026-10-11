//! THROWAWAY probe for ticket 06: the left sidebar's fifth tab, `会话`.
//!
//! Questions this probe answers with pictures (ticket 06 §「要画的帧」):
//!
//! 1. where the fifth tab sits (first vs last) and whether it fits at 28 columns
//!    without pushing `todo` off;
//! 2. the row layout (mark column / title / relative time), grouped vs flat;
//! 3. the four state marks side by side (foreground · waiting · active · absent);
//! 4. overflow: no-scroll + `＋N 场` against a scrolling variant;
//! 5. where `关闭会话` lives (top readout row / bottom row / row glyph);
//! 6. what the page looks like the instant you switch away and back;
//! 7. the page floor (a page with five rows left) and the empty state;
//! 8. the two things ticket 04 handed to ticket 06: the concurrency read-out's
//!    landing place, and how the refused-exit hint line's clickable entry reads.
//!
//! It is a probe, not product code: one file, no tests, no layers worth keeping.
//! Every frame in `frames/` is rendered by ratatui into a fixed-size `TestBackend`
//! buffer and dumped — nothing here is hand-drawn.
//!
//! Every existing Chinese string is copied verbatim from `src/render/wording.rs`
//! (line numbers in `mod w`); the ones ticket 02 / 04 / 06 **add** are marked
//! `NEW`, because they do not exist in wording.rs yet.
//!
//! Usage: `session-tab-probe <prototype-dir>` — writes `<dir>/frames/*.txt`.
//!
//! Known simplifications (not layout verdicts):
//! - the main column is a backdrop only; its shell rows differ from the real
//!   chrome by a row or two, and its transcript is blank. Nothing on the left
//!   sidebar depends on it.
//! - the identity block is drawn as the real compact mark (5 rows) or the text
//!   identity (1 row), but the 23-row seal-script mark is never drawn.
//! - colours are invisible in a dumped buffer; each frame's header lists the
//!   styles by name.

#![allow(dead_code)]

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Terminal;

// ---------------------------------------------------------------------------
// Palette: src/render/palette.rs:15–86 (interface domain only)
// ---------------------------------------------------------------------------

const PLAIN: Color = Color::Reset;
const MUTED: Color = Color::DarkGray;
const CHROME: Color = Color::Rgb(0x4a, 0x4a, 0x4a);
const ACCENT: Color = Color::LightMagenta;
const WARN: Color = Color::Yellow;
const BAD: Color = Color::Red;

// ---------------------------------------------------------------------------
// Wording: existing strings copied from src/render/wording.rs, new ones marked NEW
// ---------------------------------------------------------------------------

mod w {
    /// wording.rs:2250–2262
    pub const TAB_USAGE: &str = "调用量";
    pub const TAB_TODO: &str = "todo";
    pub const TAB_FILES: &str = "文件";
    pub const TAB_CHANGES: &str = "改动";
    pub const TAB_CONVERSATION: &str = "对话";
    pub const TAB_TRACE: &str = "轨迹";

    /// wording.rs:2510
    pub const TODO_EMPTY: &str = "还没有待办";

    /// NEW (ticket 06): the fifth tab's name.
    pub const TAB_SESSIONS: &str = "会话";

    /// NEW (ticket 02 §5): the page's only two sentences.
    pub const SESSIONS_EMPTY: &str = "还没有历史会话";
    pub const SESSIONS_FAILED: &str = "读不出会话列表";

    /// NEW (ticket 02 §2): the corrupt-stream tag.
    pub const BROKEN_TAG: &str = "损坏";

    /// NEW (ticket 06): the close entry, three candidate shapes.
    pub const CLOSE_WORD: &str = "关闭会话";
    pub const CLOSE_SHORT: &str = "关闭";
    pub const CLOSE_GLYPH: &str = "✕";

    /// NEW (ticket 02 §7): the overflow line's body.
    pub fn sessions_more(n: usize) -> String {
        format!("＋{n} 场")
    }

    /// NEW (ticket 04 §5): the concurrency read-out. `m` = sessions in the host
    /// table, `n` = of them running a turn.
    pub fn sessions_running(m: usize, n: usize) -> String {
        format!("{m} 场 · {n} 场在跑")
    }

    /// NEW (ticket 02 §6): the two rows that cannot be switched to.
    pub const GONE_HINT: &str = "工作区已不在，切不过去";
    pub const BROKEN_HINT: &str = "会话已损坏，切不过去";

    /// NEW (ticket 04 §7): the refused-exit hint line. The quoted title is the
    /// clickable entry (it points at the first running session).
    pub fn exit_refused(n: usize, title: &str) -> String {
        format!("还有 {n} 场在跑 · 切过去取消「{title}」")
    }

    /// NEW (ticket 02 §1): relative time, narrow tier (5 columns).
    pub fn age_narrow(mins: u64) -> String {
        if mins < 1 {
            "刚刚".to_owned()
        } else if mins < 60 {
            format!("{mins}分")
        } else if mins < 24 * 60 {
            format!("{}时", mins / 60)
        } else if mins < 48 * 60 {
            "昨天".to_owned()
        } else {
            format!("{:02}-{:02}", 8, 12)
        }
    }

    /// NEW (ticket 02 §1): relative time, wide tier — 11 columns, because
    /// `08-12 14:02` is 11 columns wide (ticket 02's table says 8; the probe
    /// measures 11, see README).
    pub fn age_wide(mins: u64, clock: (u64, u64)) -> String {
        if mins < 1 {
            "刚刚".to_owned()
        } else if mins < 60 {
            format!("{mins} 分钟前")
        } else if mins < 24 * 60 {
            format!("{} 小时前", mins / 60)
        } else if mins < 48 * 60 {
            format!("昨天 {:02}:{:02}", clock.0, clock.1)
        } else {
            format!("{:02}-{:02} {:02}:{:02}", 8, 12, clock.0, clock.1)
        }
    }

    /// wording.rs:2228–2236
    pub const LOGO_COMPACT: [&str; 5] = [
        "                                      ",
        "                héng                  ",
        "                 衡                   ",
        "                                      ",
        "                                      ",
    ];
}

// ---------------------------------------------------------------------------
// Width helpers (a deliberate approximation of src/render/width.rs)
// ---------------------------------------------------------------------------

fn char_w(c: char) -> usize {
    let cp = c as u32;
    let wide = (0x1100..=0x115F).contains(&cp)
        || (0x2E80..=0xA4CF).contains(&cp)
        || (0xAC00..=0xD7A3).contains(&cp)
        || (0xF900..=0xFAFF).contains(&cp)
        || (0xFE30..=0xFE6F).contains(&cp)
        || (0xFF00..=0xFF60).contains(&cp)
        || (0xFFE0..=0xFFE6).contains(&cp)
        || (0x1F300..=0x1F64F).contains(&cp)
        || (0x20000..=0x3FFFD).contains(&cp);
    if wide {
        2
    } else {
        1
    }
}

fn cols(s: &str) -> usize {
    s.chars().map(char_w).sum()
}

fn fit_cols(s: &str, width: usize) -> String {
    if cols(s) <= width {
        return s.to_owned();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let cw = char_w(c);
        if used + cw + 1 > width {
            break;
        }
        out.push(c);
        used += cw;
    }
    out.push('…');
    out
}

fn pad_right(s: &str, width: usize) -> String {
    let mut out = s.to_owned();
    let mut used = cols(s);
    while used < width {
        out.push(' ');
        used += 1;
    }
    out
}

fn pad_left(s: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = cols(s);
    while used < width {
        out.push(' ');
        used += 1;
    }
    out.push_str(s);
    out
}

// ---------------------------------------------------------------------------
// Facts: the session list (ticket 02's contract, already sorted)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Mark {
    Foreground,
    Waiting,
    Active,
    Idle,
}

struct Session {
    id: u32,
    group: usize,
    title: &'static str,
    mins: u64,
    clock: (u64, u64),
    mark: Mark,
    absent: bool,
    broken: bool,
}

struct Group {
    /// The directory name (or the last two path segments when two worktrees
    /// share a name).
    name: &'static str,
    /// Every cwd in this group is gone from disk: the header goes MUTED too.
    absent: bool,
}

fn groups() -> Vec<Group> {
    vec![
        Group {
            name: "heng",
            absent: false,
        },
        Group {
            name: "…/b/heng",
            absent: false,
        },
        Group {
            name: "fs-agent",
            absent: true,
        },
    ]
}

fn sessions() -> Vec<Session> {
    // Already in ticket 02 §3 order: the foreground's group pinned on top, then
    // groups by newest mtime, then rows by newest mtime, id as the tie-break.
    vec![
        Session { id: 1, group: 0, title: "左栏第五签：会话页的形态", mins: 0, clock: (14, 2), mark: Mark::Foreground, absent: false, broken: false },
        Session { id: 2, group: 0, title: "把外壳改成全高左栏", mins: 3, clock: (13, 59), mark: Mark::Active, absent: false, broken: false },
        Session { id: 3, group: 0, title: "会话宿主表与前台指针", mins: 12, clock: (13, 50), mark: Mark::Waiting, absent: false, broken: false },
        Session { id: 4, group: 0, title: "修复 render_layout 的帧断言", mins: 128, clock: (11, 54), mark: Mark::Idle, absent: false, broken: false },
        Session { id: 5, group: 0, title: "坏流样本", mins: 300, clock: (9, 2), mark: Mark::Idle, absent: false, broken: true },
        Session { id: 6, group: 0, title: "这是一条很长的会话标题，用来验证标题列到底怎么截断", mins: 1442, clock: (14, 2), mark: Mark::Idle, absent: false, broken: false },
        Session { id: 7, group: 0, title: "标题派生的回退链", mins: 1690, clock: (9, 30), mark: Mark::Idle, absent: false, broken: false },
        Session { id: 8, group: 0, title: "worktree 家族探测", mins: 2350, clock: (20, 0), mark: Mark::Idle, absent: false, broken: false },
        Session { id: 9, group: 0, title: "会话列表的排序兜底", mins: 2600, clock: (15, 0), mark: Mark::Idle, absent: false, broken: false },
        Session { id: 10, group: 0, title: "相对时间的列预算", mins: 2900, clock: (10, 0), mark: Mark::Idle, absent: false, broken: false },
        Session { id: 11, group: 0, title: "＋N 场的口径", mins: 4300, clock: (8, 0), mark: Mark::Idle, absent: false, broken: false },
        Session { id: 12, group: 0, title: "只读预览的降级落点", mins: 4600, clock: (9, 0), mark: Mark::Idle, absent: false, broken: false },
        Session { id: 13, group: 0, title: "后台事件分流", mins: 5000, clock: (11, 30), mark: Mark::Idle, absent: false, broken: false },
        Session { id: 14, group: 1, title: "sandbox 探测的缓存", mins: 4700, clock: (11, 0), mark: Mark::Idle, absent: false, broken: false },
        Session { id: 15, group: 1, title: "权限策略的默认值", mins: 5500, clock: (10, 0), mark: Mark::Idle, absent: false, broken: false },
        Session { id: 16, group: 2, title: "跨机器搬来的会话", mins: 15000, clock: (9, 0), mark: Mark::Idle, absent: true, broken: false },
    ]
}

// ---------------------------------------------------------------------------
// Geometry: src/render/layout.rs's ladder, one tier or two
// ---------------------------------------------------------------------------

/// src/render/layout.rs:86–131
const SIDEBAR_WIDE: u16 = 40;
const SIDEBAR_NARROW: u16 = 28;
const LOGO_WIDTH: u16 = 38;
const MARK_ROWS: u16 = 23;
const COMPACT_ROWS: u16 = 5;
const TEXT_ROWS: u16 = 1;
const TAB_ROWS: u16 = 3;
const MIN_PAGE_ROWS: u16 = 3;
const TOP_GAP: u16 = 1;

#[derive(Clone, Copy, PartialEq)]
enum Identity {
    Mark,
    Compact,
    Text,
    Hidden,
}

impl Identity {
    fn rows(self) -> u16 {
        match self {
            Identity::Mark => MARK_ROWS,
            Identity::Compact => COMPACT_ROWS,
            Identity::Text => TEXT_ROWS,
            Identity::Hidden => 0,
        }
    }
}

fn sidebar_tier(width: u16) -> Option<u16> {
    if width >= 120 {
        Some(SIDEBAR_WIDE)
    } else if width >= 80 {
        Some(SIDEBAR_NARROW)
    } else {
        None
    }
}

fn sidebar_content(tier: u16, content_rows: u16) -> (Identity, u16) {
    let mut kind = if tier >= LOGO_WIDTH {
        Identity::Mark
    } else {
        Identity::Text
    };
    loop {
        let floor = if kind == Identity::Mark { 5 } else { MIN_PAGE_ROWS };
        if kind.rows() + TAB_ROWS + floor <= content_rows {
            break;
        }
        kind = match kind {
            Identity::Mark => Identity::Compact,
            Identity::Compact => Identity::Text,
            Identity::Text => Identity::Hidden,
            Identity::Hidden => break,
        };
    }
    (kind, content_rows.saturating_sub(kind.rows() + TAB_ROWS))
}

// ---------------------------------------------------------------------------
// Cases
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Variant {
    /// The recommended shape: `会话` first on the tab bar.
    SessionFirst,
    /// The rejected counter-shape: `会话` last.
    SessionLast,
    /// Grouped rows with the group headers removed (flat).
    FlatRows,
    /// A concurrency read-out row at the top of the page, with `关闭` on its right.
    TopReadout,
    /// `关闭会话` on a row of its own at the bottom of the page.
    BottomClose,
    /// `✕` at the end of the focused row.
    RowClose,
    /// The rejected counter-shape: the page scrolls instead of saying `＋N 场`.
    Scroll,
    /// The instant after switching to another session.
    Attached,
    /// No history at all.
    Empty,
    /// The store cannot be read.
    Failed,
    /// A page with five rows left: what survives.
    SmallPage,
    /// The agreed small-page shape: headers degrade to flat, the read-out stays.
    SmallPageFinal,
    /// The refused-exit hint line, with its clickable entry.
    ExitRefused,
    /// Two worktrees that share a name.
    SameName,
}

struct Case {
    file: &'static str,
    w: u16,
    h: u16,
    variant: Variant,
    summary: &'static str,
    styles: &'static [&'static str],
    notes: &'static [&'static str],
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            file: "120x24-session-first.txt",
            w: 120,
            h: 24,
            variant: Variant::SessionFirst,
            summary: "宽档 40 / 页区 15 行 / 五签且 `会话` 第一（推荐形状）",
            styles: &[
                "页签：选中 ACCENT+BOLD，其余 MUTED，线与分隔 CHROME",
                "组头：PLAIN（改动页同档）",
                "行：标记列 1 · 标题 27 · 间隔 1 · 时间 11（右对齐）",
                "前台 ▍ ACCENT ／ 待答 ? WARN ／ 活跃 ● PLAIN ／ 空闲 空格",
                "损坏标签 BAD，排在时间之前（票 02 写的是时间之后，见 README）",
                "焦点行 ACCENT+BOLD，整行",
                "末行 ＋N 场 MUTED",
            ],
            notes: &[
                "组头 3 行 + 成员 12 行 = 15 行，正好是页区；还有 4 场没露面 → ＋4 场",
                "标题 27 列把长标题截到 …，时间列 11 列容得下 `08-12 14:02`",
            ],
        },
        Case {
            file: "80x24-session-first.txt",
            w: 80,
            h: 24,
            variant: Variant::SessionFirst,
            summary: "窄档 28 / 页区 19 行 / 五签全在，剩 2 列填线",
            styles: &[
                "页签：五签占 26 列（会话 4 + ┆ + 调用量 6 + … + 改动 4），余 2 列 ┄",
                "行：标记列 1 · 标题 21 · 间隔 1 · 时间 5（右对齐）",
                "时间窄写法：刚刚 / 3分 / 2时 / 昨天 / 08-12",
            ],
            notes: &[
                "`todo` 这一签没有被新签挤掉：26 ≤ 28，余 2 列",
                "窄档的时间列 5 列容不下 `08-12 14:02`，所以窄档只有短写法",
            ],
        },
        Case {
            file: "120x24-session-last.txt",
            w: 120,
            h: 24,
            variant: Variant::SessionLast,
            summary: "对照：同样五签，`会话` 放最后",
            styles: &["与第一张同，仅页签次序不同"],
            notes: &["被否决：切换会话是这一页最常做的一件事，末位要横扫整条页签条"],
        },
        Case {
            file: "120x24-flat-rows.txt",
            w: 120,
            h: 24,
            variant: Variant::FlatRows,
            summary: "对照：去掉组头，同一份数据平铺",
            styles: &["与第一张同，少了 3 行组头"],
            notes: &[
                "代价：看不出哪几场是同一棵检出，而票 02 §3 的排序正是按检出分组的",
                "收益：多 3 行会话（15 行里从 12 场到 15 场）",
            ],
        },
        Case {
            file: "80x24-flat-rows.txt",
            w: 80,
            h: 24,
            variant: Variant::FlatRows,
            summary: "对照：窄档平铺",
            styles: &["与窄档第一张同，少了组头"],
            notes: &["窄档 19 行页区里，3 行组头 = 15.8% 的页区"],
        },
        Case {
            file: "120x24-top-readout.txt",
            w: 120,
            h: 24,
            variant: Variant::TopReadout,
            summary: "关闭入口候选 A：页顶一行读数 `N 场 · M 场在跑`，右端 `关闭`",
            styles: &[
                "读数行 MUTED，`关闭` ACCENT（可点档）",
                "页区从第 2 行起，少 1 行",
            ],
            notes: &[
                "票 04 §5 的并发读数与票 04 §10 的关闭入口落在同一行",
                "代价：吃掉 1 行页区（15 → 14），窄档页区小时更贵",
            ],
        },
        Case {
            file: "120x24-bottom-close.txt",
            w: 120,
            h: 24,
            variant: Variant::BottomClose,
            summary: "关闭入口候选 B：页底一行 `关闭会话`",
            styles: &["`关闭会话` 右对齐 ACCENT"],
            notes: &["与 `todo` 页的 `详情` 按钮不同侧：那个在页顶，这个在页底"],
        },
        Case {
            file: "120x24-row-close.txt",
            w: 120,
            h: 24,
            variant: Variant::RowClose,
            summary: "关闭入口候选 C：焦点行尾 `✕`",
            styles: &["`✕` ACCENT，落在焦点行的时间之后"],
            notes: &[
                "代价：焦点行更挤，且 `✕` 与标题截断后的 `…` 在同一段里读起来像正文",
                "代价：误点一行 = 卸载一场会话，而命中层没有确认（票 07 定语义）",
            ],
        },
        Case {
            file: "120x24-scroll-variant.txt",
            w: 120,
            h: 24,
            variant: Variant::Scroll,
            summary: "对照：滚动变体（被否决）",
            styles: &["与第一张同，页区从焦点行那一组起，无 `＋N 场`"],
            notes: &[
                "被否决：左栏既有四页都没有滚动（docs/render.md:169、:346），票 02 §7 已定不滚动",
                "与之相对：末行 `＋N 场` 讲得清「还有多少」，而滚动条要占列",
            ],
        },
        Case {
            file: "120x24-attached.txt",
            w: 120,
            h: 24,
            variant: Variant::Attached,
            summary: "切到第 16 场之后：前台标记移动，它那一组钉到组序最顶",
            styles: &["与第一张同"],
            notes: &[
                "切走再切回：焦点行留在原处（按 id，不按下标），页签选择是进程级（票 04 §8）",
                "行序会变：mtime 降序 + 前台组钉顶，两次重扫之间行序可能漂移（票 02 §10）",
            ],
        },
        Case {
            file: "120x24-empty.txt",
            w: 120,
            h: 24,
            variant: Variant::Empty,
            summary: "空态：`还没有历史会话`；页签常驻",
            styles: &["整页一句话 MUTED"],
            notes: &["票 02 §5：页签不因内容空而消失，它是导航入口"],
        },
        Case {
            file: "120x24-failed.txt",
            w: 120,
            h: 24,
            variant: Variant::Failed,
            summary: "空态：store 读不出 → `读不出会话列表`",
            styles: &["整页一句话 MUTED"],
            notes: &["票 02 §5：整页文案只有这两句，`git` 不在 PATH 不产生文案"],
        },
        Case {
            file: "80x10-small-page.txt",
            w: 80,
            h: 10,
            variant: Variant::SmallPage,
            summary: "页区只有 5 行：1 行组头 + 3 场 + `＋13 场`",
            styles: &["与窄档第一张同"],
            notes: &[
                "这是票 02 §9 点名要回报的读数：窄档小页区里组头很贵",
                "候选降级：页区不足 N 行时不画组头（回到平铺），要不要收下由票 06 定",
            ],
        },
        Case {
            file: "80x11-small-page-final.txt",
            w: 80,
            h: 11,
            variant: Variant::SmallPageFinal,
            summary: "拍板形状的小页区：页区 6 行 → 组头降级成平铺，页顶读数与 `关闭` 保留",
            styles: &["与窄档第一张同；无组头；页顶读数行在"],
            notes: &[
                "三级阶梯：页区 ≥ 8 行则组头 + 读数行；6–7 行则丢组头保读数；< 6 行则读数也让位（那张就是 80x10 的平铺帧）",
            ],
        },
        Case {
            file: "80x10-small-page-no-headers.txt",
            w: 80,
            h: 10,
            variant: Variant::FlatRows,
            summary: "同样的 5 行页区，去掉组头：露出 4 场 + `＋12 场`",
            styles: &["与上面同，少了组头"],
            notes: &["与上一张成对，量的是组头在小页区里的价格"],
        },
        Case {
            file: "120x24-same-name.txt",
            w: 120,
            h: 24,
            variant: Variant::SameName,
            summary: "两个都叫 heng 的检出：组头写尾两段路径",
            styles: &["与第一张同"],
            notes: &["票 02 §4：不同路径可能同名，退化写 `…/a/heng` 与 `…/b/heng`"],
        },
        Case {
            file: "120x24-exit-refused.txt",
            w: 120,
            h: 24,
            variant: Variant::ExitRefused,
            summary: "退出被拒：提示行里的可点入口（票 04 §7 交给票 06 画）",
            styles: &[
                "提示行 MUTED，`切过去取消「…」` 那一段 ACCENT（可点档）",
                "主列 79 列提示行；会话页在左栏照常",
            ],
            notes: &[
                "票 04 §7：拒绝时给一个可点入口指向在跑集合里的第一场",
                "这一行只借这一帧画出形状；条目本身的措辞归票 04",
            ],
        },
    ]
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn put(frame: &mut ratatui::Frame, x: u16, y: u16, symbol: &str, style: Style) {
    if let Some(cell) = frame.buffer_mut().cell_mut((x, y)) {
        cell.set_symbol(symbol);
        cell.set_style(style);
    }
}

struct Layout {
    tier: u16,
    identity: Identity,
    page: Rect,
    tabs: Rect,
    main_x: u16,
}

fn layout(w: u16, h: u16) -> Layout {
    let tier = sidebar_tier(w).unwrap_or(SIDEBAR_NARROW);
    let content_rows = h.saturating_sub(TOP_GAP);
    let (identity, page_rows) = sidebar_content(tier, content_rows);
    let tabs_y = TOP_GAP + identity.rows() + 1;
    Layout {
        tier,
        identity,
        page: Rect::new(0, tabs_y + 2, tier, page_rows),
        tabs: Rect::new(0, tabs_y, tier, 1),
        main_x: tier + 1,
    }
}

fn draw_identity(frame: &mut ratatui::Frame, l: &Layout, h: u16) {
    match l.identity {
        Identity::Compact => {
            for (i, line) in w::LOGO_COMPACT.iter().enumerate() {
                let y = TOP_GAP + i as u16;
                if y >= h {
                    break;
                }
                // 拼音一行退一档、字形全亮（docs/render.md:121–128）。
                let style = Style::default().fg(if i == 1 { MUTED } else { ACCENT });
                put(frame, 0, y, line, style);
            }
        }
        Identity::Text => {
            let text = "衡 heng";
            let pad = (l.tier as usize).saturating_sub(cols(text)) / 2;
            put(frame, pad as u16, TOP_GAP, text, Style::default().fg(PLAIN));
        }
        _ => {}
    }
    // 左栏顶上那一行空行（layout.rs:100）。
    let _ = h;
}

fn draw_tab_bar(frame: &mut ratatui::Frame, l: &Layout, variant: Variant) {
    let labels: Vec<&str> = match variant {
        Variant::SessionLast => vec![
            w::TAB_USAGE,
            w::TAB_TODO,
            w::TAB_FILES,
            w::TAB_CHANGES,
            w::TAB_SESSIONS,
        ],
        _ => vec![
            w::TAB_SESSIONS,
            w::TAB_USAGE,
            w::TAB_TODO,
            w::TAB_FILES,
            w::TAB_CHANGES,
        ],
    };
    let rule = Style::default().fg(CHROME);
    let dim = Style::default().fg(MUTED);
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;
    for (i, label) in labels.iter().enumerate() {
        let selected = *label == w::TAB_SESSIONS;
        let style = if selected {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            dim
        };
        spans.push(Span::styled((*label).to_owned(), style));
        used += cols(label);
        if i + 1 < labels.len() {
            spans.push(Span::styled("┆", rule));
            used += 1;
        }
    }
    spans.push(Span::styled(
        "┄".repeat((l.tier as usize).saturating_sub(used)),
        rule,
    ));
    // 页签条上下两条线（tui.rs:8319–8326）。
    for y in [l.tabs.y.saturating_sub(1), l.tabs.y + 1] {
        put(
            frame,
            0,
            y,
            &"┄".repeat(l.tier as usize),
            Style::default().fg(CHROME),
        );
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), l.tabs);
}

fn mark_span(mark: Mark, muted: bool) -> Span<'static> {
    let (glyph, colour) = match mark {
        Mark::Foreground => ("▍", ACCENT),
        Mark::Waiting => ("?", WARN),
        Mark::Active => ("●", PLAIN),
        Mark::Idle => (" ", PLAIN),
    };
    Span::styled(
        glyph.to_owned(),
        Style::default().fg(if muted { MUTED } else { colour }),
    )
}

fn session_line(s: &Session, tier: u16, focused: bool, close_glyph: bool) -> Line<'static> {
    // Ticket 02 §1: mark 1 · title T · gap 1 · time Tw. The wide tier's time
    // column is 11 (not 8) because `08-12 14:02` is 11 columns wide.
    let wide = tier >= SIDEBAR_WIDE;
    let time_cols = if wide { 11 } else { 5 };
    let broken_cols = if s.broken { cols(w::BROKEN_TAG) + 1 } else { 0 };
    // The `✕` candidate eats two more columns off the title (glyph + space).
    let close_cols = if close_glyph { 2 } else { 0 };
    let title_cols = (tier as usize).saturating_sub(1 + 1 + time_cols + broken_cols + close_cols);
    let muted = s.absent;
    let row_style = if focused {
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else if muted {
        Style::default().fg(MUTED)
    } else {
        Style::default().fg(PLAIN)
    };
    let title = fit_cols(s.title, title_cols);
    let age = if wide {
        w::age_wide(s.mins, s.clock)
    } else {
        w::age_narrow(s.mins)
    };
    let mut spans = vec![mark_span(s.mark, muted)];
    spans.push(Span::styled(
        pad_right(&title, title_cols),
        row_style,
    ));
    spans.push(Span::styled(" ".to_owned(), row_style));
    if s.broken {
        // Ticket 02 puts the tag *after* the time; that pushes the row past the
        // time column, so the probe draws it before the time (see README).
        spans.push(Span::styled(
            w::BROKEN_TAG.to_owned(),
            Style::default().fg(if muted { MUTED } else { BAD }),
        ));
        spans.push(Span::styled(" ".to_owned(), row_style));
    }
    spans.push(Span::styled(pad_left(&age, time_cols), row_style));
    if close_glyph {
        spans.push(Span::styled(" ".to_owned(), row_style));
        spans.push(Span::styled(
            w::CLOSE_GLYPH.to_owned(),
            Style::default().fg(ACCENT),
        ));
    }
    Line::from(spans)
}

fn group_line(g: &Group, tier: u16) -> Line<'static> {
    let text = fit_cols(g.name, tier as usize);
    Line::from(Span::styled(
        text,
        Style::default().fg(if g.absent { MUTED } else { PLAIN }),
    ))
}

fn draw_page(frame: &mut ratatui::Frame, l: &Layout, case: &Case) {
    let page = l.page;
    if page.height == 0 {
        return;
    }
    let groups = {
        let mut g = groups();
        if case.variant == Variant::SameName {
            // Two checkouts that share a directory name: the header degrades to
            // the last two path segments (ticket 02 §4).
            g[0].name = "…/a/heng";
        }
        g
    };
    let mut all = sessions();

    // The `attached` frame: session 16 became the foreground, so its group is
    // pinned on top and its row leads that group.
    let focus_id = match case.variant {
        Variant::Attached => 15,
        Variant::Scroll => 13,
        _ => 1,
    };
    if case.variant == Variant::Attached {
        // Session 15 (in the `…/b/heng` group) became the foreground: its group
        // is pinned on top and its row leads that group.
        let mut moved: Vec<Session> = all
            .iter()
            .filter(|s| s.id == 15)
            .map(|s| Session { id: s.id, group: s.group, title: s.title, mins: s.mins, clock: s.clock, mark: Mark::Foreground, absent: s.absent, broken: s.broken })
            .collect();
        let rest: Vec<Session> = all
            .iter()
            .filter(|s| s.id != 15)
            .map(|s| Session { id: s.id, group: s.group, title: s.title, mins: s.mins, clock: s.clock, mark: if s.id == 1 { Mark::Idle } else { s.mark }, absent: s.absent, broken: s.broken })
            .collect();
        moved.extend(rest);
        all = moved;
    }
    if case.variant == Variant::Empty {
        all.clear();
    }
    if case.variant == Variant::Failed {
        let line = Line::from(Span::styled(
            w::SESSIONS_FAILED.to_owned(),
            Style::default().fg(MUTED),
        ));
        frame.render_widget(Paragraph::new(line), page);
        return;
    }
    if case.variant == Variant::Empty {
        let line = Line::from(Span::styled(
            w::SESSIONS_EMPTY.to_owned(),
            Style::default().fg(MUTED),
        ));
        frame.render_widget(Paragraph::new(line), page);
        return;
    }

    let flat = matches!(case.variant, Variant::FlatRows | Variant::SmallPageFinal);
    let top_readout = matches!(
        case.variant,
        Variant::TopReadout | Variant::SmallPageFinal
    );
    let bottom_close = case.variant == Variant::BottomClose;
    let scroll = case.variant == Variant::Scroll;

    // Build the drawn sequence: headers and rows, in list order.
    let mut items: Vec<(bool, usize, usize)> = Vec::new(); // (is_header, index, group)
    let mut group_of: Vec<Vec<usize>> = vec![Vec::new(); groups.len()];
    for (i, s) in all.iter().enumerate() {
        group_of[s.group].push(i);
    }
    // Groups in the order the rows already carry (ticket 02 §3).
    let mut seen: Vec<usize> = Vec::new();
    for s in &all {
        if !seen.contains(&s.group) {
            seen.push(s.group);
        }
    }
    for g in &seen {
        let members = &group_of[*g];
        if !flat {
            items.push((true, *g, *g));
        }
        for i in members {
            items.push((false, *i, *g));
        }
    }

    let mut y = page.y;
    let mut budget = page.height;
    if top_readout {
        let running = all.iter().filter(|s| s.mark == Mark::Active).count();
        let label = w::sessions_running(all.len(), running);
        let close = w::CLOSE_SHORT;
        let mut spans = vec![Span::styled(label.clone(), Style::default().fg(MUTED))];
        let used = cols(&label) + cols(close);
        spans.push(Span::styled(
            " ".repeat((l.tier as usize).saturating_sub(used)),
            Style::default().fg(MUTED),
        ));
        spans.push(Span::styled(close.to_owned(), Style::default().fg(ACCENT)));
        frame.render_widget(Paragraph::new(Line::from(spans)), Rect::new(0, y, l.tier, 1));
        y += 1;
        budget = budget.saturating_sub(1);
    }
    if bottom_close {
        budget = budget.saturating_sub(1);
    }

    // The scrolling variant has no `＋N 场`; it starts at the focused row's group.
    let (start, visible) = if scroll {
        let focus_pos = items
            .iter()
            .position(|(header, index, _)| !*header && all[*index].id == focus_id)
            .unwrap_or(0);
        let b = budget as usize;
        let start = focus_pos.saturating_sub(b / 2);
        (start, b)
    } else {
        (0usize, budget as usize)
    };

    let mut drawn = 0usize;
    for (n, (header, index, _)) in items.iter().enumerate().skip(start) {
        if drawn + 1 > visible {
            break;
        }
        // Reserve the last row for `＋N 场` when the list does not fit.
        if !scroll && items.len() > visible && drawn + 1 == visible {
            break;
        }
        let line = if *header {
            group_line(&groups[*index], l.tier)
        } else {
            let s = &all[*index];
            let close = case.variant == Variant::RowClose && s.id == focus_id;
            session_line(s, l.tier, s.id == focus_id, close)
        };
        frame.render_widget(Paragraph::new(line), Rect::new(0, y, l.tier, 1));
        y += 1;
        drawn += 1;
        let _ = n;
    }
    if !scroll && items.len() > drawn {
        // Ticket 02 §7: N counts the sessions that never appeared at all.
        let hidden_members = all.len()
            - items
                .iter()
                .take(drawn + start)
                .filter(|(header, _, _)| !*header)
                .count();
        let more = w::sessions_more(hidden_members);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                more,
                Style::default().fg(MUTED),
            ))),
            Rect::new(0, y, l.tier, 1),
        );
    }
    if bottom_close {
        let line = Line::from(vec![
            Span::styled(
                " ".repeat((l.tier as usize).saturating_sub(cols(w::CLOSE_WORD))),
                Style::default().fg(MUTED),
            ),
            Span::styled(w::CLOSE_WORD.to_owned(), Style::default().fg(ACCENT)),
        ]);
        frame.render_widget(Paragraph::new(line), Rect::new(0, page.y + page.height - 1, l.tier, 1));
    }
}

fn draw_main(frame: &mut ratatui::Frame, l: &Layout, case: &Case) {
    let w = case.w.saturating_sub(l.main_x);
    let x = l.main_x;
    let y0 = 0u16;
    let h = case.h;
    // 主列页签条（docs/render.md:102）
    let rule = Style::default().fg(CHROME);
    let labels = vec![(w::TAB_CONVERSATION, true), (w::TAB_TRACE, false)];
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;
    for (i, (label, selected)) in labels.iter().enumerate() {
        spans.push(Span::styled(
            (*label).to_owned(),
            if *selected {
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(MUTED)
            },
        ));
        used += cols(label);
        if i + 1 < labels.len() {
            spans.push(Span::styled("┆", rule));
            used += 1;
        }
    }
    spans.insert(
        0,
        Span::styled("┄ ".to_owned(), Style::default().fg(CHROME)),
    );
    used += 2;
    spans.push(Span::styled(
        "┄".repeat((w as usize).saturating_sub(used)),
        rule,
    ));
    frame.render_widget(Paragraph::new(Line::from(spans)), Rect::new(x, y0, w, 1));
    put(frame, x, y0 + 1, &"┄".repeat(w as usize), rule);

    // 状态行 / 输入区 / 提示行（近似真机：底 7 行归它们）
    let status_y = h.saturating_sub(7);
    let rule_y = h.saturating_sub(6);
    let input_y = h.saturating_sub(5);
    let input_rule_y = h.saturating_sub(2);
    let hint_style = Style::default().fg(MUTED);
    let hint = match case.variant {
        Variant::ExitRefused => {
            let head = "还有 2 场在跑 · ";
            let entry = "切过去取消「会话宿主表与前台指针」";
            Line::from(vec![
                Span::styled(head.to_owned(), hint_style),
                Span::styled(entry.to_owned(), Style::default().fg(ACCENT)),
            ])
        }
        _ => Line::from(Span::styled(
            "enter 发送 · ctrl-j 换行 · esc 取消",
            hint_style,
        )),
    };
    put(
        frame,
        x,
        status_y,
        &pad_right("模型 claude-sonnet-4-5 │ 模式 询问 │ 上下文 6%", w as usize),
        Style::default().fg(PLAIN),
    );
    put(frame, x, rule_y, &"┄".repeat(w as usize), rule);
    put(
        frame,
        x,
        input_y,
        &pad_right("> ", w as usize),
        Style::default().fg(PLAIN),
    );
    put(frame, x, input_rule_y, &"┄".repeat(w as usize), rule);
    frame.render_widget(Paragraph::new(hint), Rect::new(x, h - 1, w, 1));
}

fn render(case: &Case) -> String {
    let backend = TestBackend::new(case.w, case.h);
    let mut terminal = Terminal::new(backend).expect("TestBackend terminal");
    let l = layout(case.w, case.h);
    terminal
        .draw(|frame| {
            draw_identity(frame, &l, case.h);
            draw_tab_bar(frame, &l, case.variant);
            draw_page(frame, &l, case);
            // 分隔竖线（左栏与主列之间）
            for y in 0..case.h {
                put(frame, l.tier, y, "│", Style::default().fg(CHROME));
            }
            draw_main(frame, &l, case);
        })
        .expect("draw");
    let mut out = String::new();
    writeln!(out, "size: {}x{}", case.w, case.h).unwrap();
    writeln!(out, "case: {}", case.file).unwrap();
    writeln!(out, "what: {}", case.summary).unwrap();
    writeln!(out, "sidebar: 档 {} 列 / 身份 {} 行 / 页区 {} 行", l.tier, l.identity.rows(), l.page.height).unwrap();
    for s in case.styles {
        writeln!(out, "style: {s}").unwrap();
    }
    for n in case.notes {
        writeln!(out, "note: {n}").unwrap();
    }
    let body = format!("{}", terminal.backend());
    let mut ruler = String::from("    ");
    for x in 0..case.w {
        if x % 10 == 0 {
            ruler.push(char::from_digit(((x / 10) % 10) as u32, 10).unwrap());
        } else {
            ruler.push('.');
        }
    }
    writeln!(out, "{ruler}").unwrap();
    writeln!(out, "   |{}|", "-".repeat(case.w as usize)).unwrap();
    for (i, line) in body.lines().enumerate() {
        let line = line
            .split(" Hidden by multi-width symbols")
            .next()
            .unwrap_or(line);
        let inner = line
            .strip_prefix('"')
            .and_then(|l| l.strip_suffix('"'))
            .unwrap_or(line);
        writeln!(out, "{:>3}|{}|", i, pad_right(inner, case.w as usize)).unwrap();
    }
    writeln!(out, "   |{}|", "-".repeat(case.w as usize)).unwrap();
    out
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| ".".to_owned());
    let frames = Path::new(&dir).join("frames");
    fs::create_dir_all(&frames).expect("frames dir");
    for case in cases() {
        let text = render(&case);
        fs::write(frames.join(case.file), text).expect("write frame");
        println!("{}", case.file);
    }
}
