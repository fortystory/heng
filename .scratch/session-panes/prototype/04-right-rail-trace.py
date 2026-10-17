#!/usr/bin/env python3
"""一次性原型：右栏 40 / 28 列里的轨迹长什么样。

票：`.scratch/session-panes/issues/04-prototype-right-rail-trace.md`
跑法：`python3 .scratch/session-panes/prototype/04-right-rail-trace.py > .scratch/session-panes/prototype/04-right-rail-trace.txt`

这是 **throwaway**：宽度算式照 `src/render/layout.rs` 与 `src/render/timeline.rs` 抄
（`SIDEBAR_WIDE = 40`、`SIDEBAR_NARROW = 28`、`SIDEBAR_WIDE_FROM = 120`、
`SIDEBAR_NARROW_FROM = 80`、`TRAILING_COLUMNS = 2`、`STAMP_COLUMNS = 9`、
`timeline::LABEL_COLUMNS = 5`、`BandKind::of_axis` 的阈值 44 / 32 / 9、`MIN_LEDGER_ROWS = 3`），
行内容照 `trace-ledger` 落地后的形态（组头并进虚线、二级头、行尾 `in=… out=…`、`▸` 折叠）排。

**右栏今天不存在**：它的宽度档按 [图](../map.md) 冻结项 6「同左栏制」当**候选**，帧里写明。
中文按两列算；脚本对每一帧的**内容行**做显示列自检（`("注", …)` 那些是说明，不参与自检）。
"""

from __future__ import annotations

import unicodedata

# --- 从 layout.rs / timeline.rs 抄来的常量（改了那边，这边要跟着改） --------

SIDEBAR_WIDE = 40
SIDEBAR_NARROW = 28
SIDEBAR_WIDE_FROM = 120
SIDEBAR_NARROW_FROM = 80
TRAILING_COLUMNS = 2  # 滚动条与回合条那两列
STAMP_COLUMNS = 9  # `HH:MM:SS ` 九列
LABEL_COLUMNS = 5  # 泳道标签五列
FULL_TICKS_FROM = 44
SPARSE_TICKS_FROM = 32
COMPACT_BAND_FROM = 9

TRACE_W = 79  # 今天：120 列终端带左栏时的主列宽（120 − 40 − 1）
RAIL_WIDE = SIDEBAR_WIDE  # 候选：右栏宽档
RAIL_NARROW = SIDEBAR_NARROW

CELLS = {
    "full": (STAMP_COLUMNS, lambda s: s),  # `HH:MM:SS `
    "short": (6, lambda s: s[0:5] if s else ""),  # `HH:MM `
    "none": (0, lambda s: ""),  # 不画时刻
}

# 一行可以是内容行（str，受自检）或说明行（("注", …)，不受自检）
Line = str | tuple[str, str]


def note(text: str) -> tuple[str, str]:
    return ("注", text)


# --- 显示宽度小工具 ---------------------------------------------------------


def w(text: str) -> int:
    return sum(2 if unicodedata.east_asian_width(ch) in "WF" else 1 for ch in text)


def fit(text: str, width: int) -> str:
    """超宽就截断并留一列给 `…`；不补白。"""
    if width <= 0:
        return ""
    if w(text) <= width:
        return text
    out, used = "", 0
    for ch in text:
        cw = 2 if unicodedata.east_asian_width(ch) in "WF" else 1
        if used + cw > width - 1:
            break
        out += ch
        used += cw
    return out + "…"


def pad(text: str, width: int) -> str:
    return text + " " * max(0, width - w(text))


def joined_head(text: str, width: int) -> str:
    """组头并进单位之间那条虚线（今天 79 列里的形态）。

    虚线至少留得下一列才值得并进去；连组头文字都放不下时退成裸文字。帧里能看到 40 档只剩
    四列虚线、28 档只剩一列 —— 「组头并进那条线」在右栏里本来就快没了。
    """
    body = f"┄┄ {text} "
    if w(body) >= width:
        return fit(text, width)
    return body + "┄" * max(0, width - w(body))


def row(stamp: str, body: str, tail: str, width: int, cell: str) -> str:
    """一行轨迹：行首那一格 + 正文 + 行尾用量。

    用量的取舍是**显式**的：正文至少还剩 12 列时才带用量，否则当场丢掉它（照
    `trace-ledger` 票 04 的口径：溢出先丢耗时、保用量；这里再窄一档就两样都保不住）。
    """
    cell_w, cell_txt = CELLS[cell]
    avail = max(0, width - cell_w)
    tail_w = w(tail) + 1 if tail else 0
    if tail_w and avail - tail_w < 12:
        tail, tail_w = "", 0
    body_w = avail - tail_w
    out = pad(fit(cell_txt(stamp), cell_w), cell_w) + fit(body, body_w)
    if tail:
        out += " " * max(0, width - tail_w - w(out)) + " " + tail
    return out


# --- 一段样例会话（照 trace-ledger 的素材重排，只用来量宽度） ----------------

CONVO: list[tuple[str, ...]] = [
    ("head", "回合 1 · 49 s · bash×2 read×2 edit×1"),
    ("row", "09:12:05", "[kimi] 第 1 次迭代 · 7 s", ""),
    ("row", "09:12:05", "[kimi] … 正在思考", ""),
    ("row", "09:12:11", "[kimi] ▸ ✓ 思考完成", ""),
    ("row", "09:12:11", "[kimi] ▸ 调用 read_file src/render/tui.rs", "in=18234 out=312"),
    ("row", "09:12:12", "[kimi] ▸ 调用 grep TRUNCATED_MARKER", "in=18310 out=204"),
    ("row", "09:12:27", "[kimi] ▸ 调用 bash 运行 cargo test -q", "in=19502 out=1880"),
    ("row", "09:12:29", "[kimi] ▸ 调用 edit_file src/render/tui.rs 失败", "in=21460 out=143"),
    ("row", "09:12:52", "[kimi] ▸ 头尾都留住了，省掉多少也写清楚了", "in=24130 out=402"),
    ("row", "09:12:52", "[kimi] ▸ 回合结束：完成", ""),
]


def render(width: int, cell: str, keep_tail: bool = True) -> list[Line]:
    out: list[Line] = []
    for item in CONVO:
        if item[0] == "head":
            out.append(joined_head(item[1], width))
        else:
            _, stamp, body, tail = item
            out.append(row(stamp, body, tail if keep_tail else "", width, cell))
    return out


def axis_width(rail: int) -> int:
    return max(0, rail - TRAILING_COLUMNS - (STAMP_COLUMNS + LABEL_COLUMNS))


def band_kind(axis: int) -> str:
    if axis >= FULL_TICKS_FROM:
        return "Full（3 行，五格刻度）"
    if axis >= SPARSE_TICKS_FROM:
        return "Sparse（3 行，两端两点）"
    if axis >= COMPACT_BAND_FROM:
        return "Compact（2 行，一行三态）"
    return "Empty（不画）"


def panel_columns(total: int, left: int | None, right: int | None) -> tuple[int, int]:
    """(占掉的列, 主列剩下列)：左右各连同它那一列分隔线。"""
    used = (left + 1 if left else 0) + (right + 1 if right else 0)
    return used, max(0, total - used)


def screen_table(total: int) -> list[Line]:
    wide = total >= SIDEBAR_WIDE_FROM
    tier = SIDEBAR_WIDE if wide else SIDEBAR_NARROW
    rows: list[Line] = []
    for left, right, label in (
        (tier, tier, "两侧都开"),
        (tier, None, "只开左栏"),
        (None, tier, "只开右栏"),
        (None, None, "两侧都关"),
    ):
        used, main = panel_columns(total, left, right)
        rows.append(
            f"{total} 列 · {label}：左栏 {left or 0} + 1 ÷ 主列 {main} + 1 ÷ 右栏 {right or 0}"
            f"（占 {used}）"
        )
    return rows


# --- 帧 --------------------------------------------------------------------

FRAMES: list[tuple[str, str, int, list[Line]]] = []


def frame(title: str, why: str, width: int, lines: list[Line]) -> None:
    FRAMES.append((title, why, width, lines))


frame(
    "帧 1 · 基线：今天主列里的轨迹（内容宽 79）",
    "120 列终端带左栏时主列 79 列。行首九列时刻、组头并进那条虚线、二级头、行尾用量，"
    "全部完整 —— 这是下面每一帧的对照。",
    TRACE_W,
    render(TRACE_W, "full"),
)

frame(
    "帧 2a · 右栏 40 列，保住九列时刻 + 行尾用量",
    "扣掉九列时刻只剩 31 列，行尾用量再吃掉 16 列 —— 正文只剩 15 列，工具行读成一个 `调用…`，"
    "连工具名都报不全。这是右栏里最没法读的一档，而它恰恰是「什么都想保住」的那一档。",
    RAIL_WIDE,
    render(RAIL_WIDE, "full"),
)

frame(
    "帧 2b · 右栏 40 列，时刻收成 `HH:MM`（6 列）+ 行尾用量",
    "把时刻那一格从 9 列收到 6 列（秒丢掉），正文多拿 3 列。秒对轨迹的读法有用吗 ——"
    "下面是答案的原料。",
    RAIL_WIDE,
    render(RAIL_WIDE, "short"),
)

frame(
    "帧 2c · 右栏 40 列，不画时刻 + 行尾用量",
    "正文拿到 24 列（40 减掉用量那 16 列），比帧 2a 的 15 列好一档，而帧 1 的正文是 54 列。"
    "代价：这一页再也说不出「什么时候」。",
    RAIL_WIDE,
    render(RAIL_WIDE, "none"),
)

frame(
    "帧 2d · 右栏 40 列，保住时刻、放弃行尾用量",
    "与帧 2a 同一个宽度，换一种取舍：正文从 15 列涨到 31 列，工具名与路径都回来了 ——"
    "代价是用量一个字都不在右栏里出现，这一页再也说不出「花了多少」。",
    RAIL_WIDE,
    render(RAIL_WIDE, "full", keep_tail=False),
)

frame(
    "帧 3a · 右栏 28 列（herdr 常态），保住九列时刻 + 行尾用量",
    "内容只剩 19 列：`[kimi] ▸ 调用 re…`。行尾用量在这一档已经挂不上（19 − 22 < 12），"
    "于是它与描述一起被截 —— 28 档不是「40 档的缩小版」，它会换一种读法。",
    RAIL_NARROW,
    render(RAIL_NARROW, "full"),
)

frame(
    "帧 3b · 右栏 28 列，时刻收成 `HH:MM`",
    "内容 22 列。比 3a 多三个字，仍是「只认得出工具名」的一档。",
    RAIL_NARROW,
    render(RAIL_NARROW, "short"),
)

frame(
    "帧 3c · 右栏 28 列，不画时刻、不带用量",
    "内容拿满 28 列。这是 28 档唯一能读出「哪次调用、成没成」的排法 —— 代价是时刻与用量"
    "两样都不在这一页。",
    RAIL_NARROW,
    render(RAIL_NARROW, "none", keep_tail=False),
)

frame(
    "帧 4 · 时间轴横带：同左栏制下永远到不了 Full / Sparse",
    "轴宽 = 右栏宽 − 2（滚动条与回合条）− 9（时刻列）− 5（泳道标签）。三档阈值是 44 / 32 / 9。",
    60,
    [
        note(f"右栏 40 列 → 轴 {axis_width(RAIL_WIDE)} 列 → {band_kind(axis_width(RAIL_WIDE))}"),
        note(f"右栏 28 列 → 轴 {axis_width(RAIL_NARROW)} 列 → {band_kind(axis_width(RAIL_NARROW))}"),
        note(f"对照 · 今天主列 79 列 → 轴 {axis_width(TRACE_W)} 列 → {band_kind(axis_width(TRACE_W))}"),
        note(""),
        note("两档都落在 Compact（2 行、一行三态）；40 档离 Sparse 还差 8 列。"),
        note("降级省下的行还给账本（Full 3 行 → Compact 2 行，省 1 行）。"),
        note(""),
        note("高度那一档另算（layout::band_kind）：账本还要留得下 MIN_LEDGER_ROWS = 3 行，"),
        note("否则整条横带关掉 —— 右栏高度不足时同样适用。"),
    ],
)

frame(
    "帧 5 · 右栏 28 列里的组头、选中与折叠",
    "纯文本帧用行首 `▌` 示意选中（真机上它是整行 ACCENT + BOLD，不画反显），`▸` 是今天"
    "折叠态唯一允许用它的地方。",
    RAIL_NARROW,
    [
        joined_head("回合 1 · 49 s · bash×2", RAIL_NARROW),
        row("09:12:11", "▌[kimi] ▸ 调用 read_file src/render/tui.rs", "", RAIL_NARROW, "full"),
        row("09:12:12", "[kimi] ▸ 调用 grep TRUNCATED_MARKER", "", RAIL_NARROW, "full"),
        row("09:12:27", "[kimi] ▸ 调用 bash 运行 cargo test -q", "", RAIL_NARROW, "full"),
        "",
        note("折叠态（一整段连续工具调用收成一行）："),
        row("09:12:27", "▸ 连续工具调用 × 4 · 8.2 s", "", RAIL_NARROW, "full"),
        "",
        note("组头被截时的两种写法（交给 grilling 收口）："),
        joined_head("回合 1 · 49 s", RAIL_NARROW),
        fit("┄┄ 回合 1 " + "┄" * 20, RAIL_NARROW),
    ],
)

frame(
    "帧 6 · 整屏对照：herdr 里 120 列的终端（heng 只见 94 列）",
    "冻结项 6 说「右栏与左栏并存、窄屏右栏先让位」。94 列落 28 档。这一帧把这条张力摆到"
    "眼睛前，让位阈值归票 05 / 票 06 拍。右栏宽度档是同左栏制的**候选**。",
    94,
    [
        note("外层 120 列终端 → herdr 客户端扣 26 列侧栏 → heng 面板 94 列（research/03 第 4 条）"),
        note(""),
        *screen_table(94),
        note(""),
        note("同一算式在没有 herdr 的 120 列（40 档）与 80 列（28 档边界）："),
        *screen_table(120),
        *screen_table(80),
    ],
)

frame(
    "帧 7 · 右栏 40 列里点一行：详情覆盖层不受右栏约束",
    "只报事实，不画几何。",
    40,
    [
        note("详情覆盖层按**整屏**算宽度（layout 的 detail_width 对着屏幕量，不缩进 pane），"),
        note("所以右栏 40 列里点得开、正文按整屏排。"),
        note(""),
        note("关闭时还原给打开它的那一页（今天：轨迹页）—— 搬到右栏之后「那一页」是谁，"),
        note("以及平铺时该不该缩进 pane，都归票 06。"),
    ],
)


# --- 输出与自检 -------------------------------------------------------------

problems: list[str] = []
print("=" * 78)
print("右栏 40 / 28 列里的轨迹 —— 形态帧")
print("票：.scratch/session-panes/issues/04-prototype-right-rail-trace.md")
print("=" * 78)

for index, (title, why, width, lines) in enumerate(FRAMES, start=1):
    print()
    print(f"── 帧 {index} ─ {title} {'─' * max(0, 60 - w(title))}")
    print(f"   目标宽度 {width} 列")
    for text in why.splitlines():
        print(f"   {text}")
    print()
    for line in lines:
        if isinstance(line, tuple):
            print(f"   ({line[1]})")
            continue
        mark = ""
        if w(line) > width:
            mark = f"  ⚠ 溢出 {w(line) - width} 列"
            problems.append(f"帧 {index}：{w(line)} > {width} —— {line!r}")
        print(f"   |{line}{mark}")
    print()

print("=" * 78)
if problems:
    print(f"自检：{len(problems)} 行溢出")
    for problem in problems:
        print(f"  - {problem}")
else:
    print("自检：每一帧的每一条内容行都 ≤ 目标宽度")
print("=" * 78)
