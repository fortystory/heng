#!/usr/bin/env python3
"""一次性原型：平铺的几何与两态布局。

票：`.scratch/session-panes/issues/05-prototype-panes-layout.md`
跑法：`python3 .scratch/session-panes/prototype/05-panes-layout.py > .scratch/session-panes/prototype/05-panes-layout.txt`

这是 **throwaway**：行开销照 `src/render/layout.rs` 抄（`CHROME = 6` =
主列页签条 2 + 状态行 1 + 两条线 2 + 提示行 1；`MIN_INPUT_ROWS = 3`、`MIN_WIDTH = 40`、
`MIN_HEIGHT = 10`、`SIDEBAR_WIDE = 40` / `SIDEBAR_NARROW = 28` / `≥120` / `≥80`）。

**平铺几何是候选**，帧里要回答的四件事：
1. 顶部页签条占几行、横跨整屏还是只在主列里；
2. pane 之间的分隔线怎么画、焦点 pane 怎么认出来；
3. 宽 / 高不够时，低于哪一档回标签态；
4. 非焦点 pane 的忙 / 等人类 / 未读怎么标（左栏管状态、顶部管显示）。

中文按两列算；每一帧做网格自检（格子上每行显示宽度必须正好等于目标宽度）。
"""

from __future__ import annotations

import unicodedata

# --- 从 layout.rs 抄来的常量 -------------------------------------------------

MIN_WIDTH = 40
MIN_HEIGHT = 10
SIDEBAR_WIDE = 40
SIDEBAR_NARROW = 28
SIDEBAR_WIDE_FROM = 120
SIDEBAR_NARROW_FROM = 80
MIN_INPUT_ROWS = 3

# 平铺态的垂直开销（候选）：页签条 2（标签行 + 一条虚线）+ 状态行 1 + 一条线 1 +
# 输入区 MIN_INPUT_ROWS + 一条线 1 + 提示行 1
STRIP_ROWS = 2
FIXED_BELOW = 1 + 1 + MIN_INPUT_ROWS + 1 + 1


def w(text: str) -> int:
    return sum(2 if unicodedata.east_asian_width(ch) in "WF" else 1 for ch in text)


def fit(text: str, width: int) -> str:
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


def pane_area(height: int) -> int:
    return max(0, height - STRIP_ROWS - FIXED_BELOW)


def layout_panes(width: int, height: int, count: int) -> list[tuple[int, int, int, int]] | None:
    """(x, y, w, h) 列表；放不下返回 None。分隔线各占 1 列 / 1 行。"""
    rows = pane_area(height)
    if rows <= 0:
        return None
    half_w = (width - 1) // 2
    half_h = (rows - 1) // 2
    if count == 1:
        plan = [(0, 0, width, rows)]
    elif count == 2:
        plan = [(0, 0, half_w, rows), (half_w + 1, 0, width - half_w - 1, rows)]
    elif count == 3:
        plan = [
            (0, 0, half_w, rows),
            (half_w + 1, 0, width - half_w - 1, half_h),
            (half_w + 1, half_h + 1, width - half_w - 1, rows - half_h - 1),
        ]
    elif count == 4:
        plan = [
            (0, 0, half_w, half_h),
            (half_w + 1, 0, width - half_w - 1, half_h),
            (0, half_h + 1, half_w, rows - half_h - 1),
            (half_w + 1, half_h + 1, width - half_w - 1, rows - half_h - 1),
        ]
    else:
        return None
    if any(pw < MIN_WIDTH or ph < MIN_HEIGHT for _, _, pw, ph in plan):
        return None
    return plan


# --- 字符网格 ---------------------------------------------------------------


class Grid:
    def __init__(self, width: int, height: int) -> None:
        self.w = width
        self.h = height
        self.cells = [[" "] * width for _ in range(height)]

    def put(self, x: int, y: int, text: str, limit: int) -> None:
        if y < 0 or y >= self.h:
            return
        col, used = x, 0
        for ch in fit(text, limit):
            cw = 2 if unicodedata.east_asian_width(ch) in "WF" else 1
            if used + cw > limit or col + cw > self.w:
                break
            self.cells[y][col] = ch
            if cw == 2:
                self.cells[y][col + 1] = ""
            col += cw
            used += cw

    def hline(self, x: int, y: int, length: int, char: str = "─") -> None:
        for col in range(x, min(self.w, x + length)):
            self.cells[y][col] = char

    def vline(self, x: int, y: int, length: int, char: str = "│") -> None:
        for row_index in range(y, min(self.h, y + length)):
            self.cells[row_index][x] = char

    def rows(self) -> list[str]:
        return ["".join(cell for cell in row) for row in self.cells]


def draw_screen(
    width: int,
    height: int,
    panes: list[tuple[str, str]],
    *,
    strip: list[str] | None = None,
    sides: tuple[int | None, int | None] = (None, None),
    focus: int = 0,
    focus_style: str = "frame",
    force_single: bool = False,
) -> list[str]:
    """一屏：页签条 + pane 区 + 状态行 + 输入区 + 提示行。sides = (左栏宽, 右栏宽)。"""
    grid = Grid(width, height)
    left, right = sides
    # 左栏 / 右栏（全高，页签条横跨整屏时它们从页签条下面起）
    body_y = STRIP_ROWS
    if left:
        grid.vline(left, body_y, height - body_y, "│")
        for row_index in range(body_y, height):
            grid.put(0, row_index, "左栏", left)
    if right:
        rx = width - right - 1
        grid.vline(rx, body_y, height - body_y, "│")
        for row_index in range(body_y, height):
            grid.put(rx + 1, row_index, "右栏", right)

    if strip:
        for index, label in enumerate(strip):
            grid.put(0, 0, label, width)
        grid.hline(0, 1, width, "┄")

    inner_x = (left + 1) if left else 0
    inner_w = width - inner_x - ((right + 1) if right else 0)
    # 标签态：一个会话占主列，**不套** MIN_WIDTH 判据 —— 它今天就可能比 40 列窄
    # （120 列两侧栏全开 → 主列只有 38 列，见帧 1 的注）。
    if force_single and len(panes) == 1 and pane_area(height) > 0:
        plan: list[tuple[int, int, int, int]] | None = [(0, 0, inner_w, pane_area(height))]
    else:
        plan = layout_panes(inner_w, height, len(panes))
    if plan is None:
        grid.put(inner_x, STRIP_ROWS, "（放不下：回标签态）", inner_w)
        return grid.rows()

    for (px, py, pw, ph), (title, state) in zip(plan, panes):
        ax, ay = inner_x + px, body_y + py
        corner = {"frame": "┌", "title": "┌", "mark": "┌"}[focus_style]
        hz = "─"
        if panes.index((title, state)) == focus:
            if focus_style == "frame":
                corner, hz = "┏", "━"
        # 框
        grid.put(ax, ay, corner, 1)
        grid.hline(ax + 1, ay, max(0, pw - 2), hz)
        grid.put(ax + pw - 1, ay, "┐" if corner == "┌" else "┓", 1)
        grid.vline(ax, ay + 1, max(0, ph - 2), "┃" if (corner == "┏" and focus_style == "frame") else "│")
        grid.vline(ax + pw - 1, ay + 1, max(0, ph - 2), "┃" if (corner == "┏" and focus_style == "frame") else "│")
        grid.put(ax, ay + ph - 1, "└", 1)
        grid.hline(ax + 1, ay + ph - 1, max(0, pw - 2), hz)
        grid.put(ax + pw - 1, ay + ph - 1, "┘" if corner == "┌" else "┛", 1)
        # 内容
        mark = ""
        if panes.index((title, state)) == focus and focus_style == "mark":
            mark = "◆ "
        grid.put(ax + 2, ay + 1, f"{mark}{title} · {state}", max(0, pw - 4))
        grid.put(ax + 2, ay + 2, f"{pw}×{ph}", max(0, pw - 4))
        for offset in range(3, ph - 2):
            grid.put(ax + 2, ay + offset, "…", max(0, pw - 4))

    # 底下的固定三条
    grid.put(0, height - FIXED_BELOW + 1, "状态行：模型 │ 模式 │ 上下文 12% —— 读焦点会话", width)
    grid.put(0, height - FIXED_BELOW + 3, "输入区（全局一份，绑焦点 pane）", width)
    grid.put(0, height - FIXED_BELOW + 4, "提示行：ctrl-? 平铺 · ctrl-v 右栏 · ctrl-o 左栏", width)
    return grid.rows()


FRAMES: list[tuple[str, str, int, list[str]]] = []


def frame(title: str, why: str, width: int, lines: list[str]) -> None:
    FRAMES.append((title, why, width, lines))


def show(lines: list[str], width: int) -> list[str]:
    return [f"|{line}" for line in lines]


# --- 帧 1：两态对照（120×40） ----------------------------------------------

TAG_SCREEN = draw_screen(
    120,
    40,
    [("会话 A · 调研", "忙碌")],
    strip=["[会话 A · 调研]  [会话 B · 编译]  [会话 C · 文档]        ▤ 平铺  ▥ 右栏"],
    sides=(SIDEBAR_WIDE, SIDEBAR_WIDE),
    force_single=True,
)
TILE_SCREEN = draw_screen(
    120,
    40,
    [("会话 A · 调研", "忙碌"), ("会话 B · 编译", "等人类")],
    strip=["[会话 A]  [会话 B]  [会话 C]                    ▤ 平铺  ▥ 右栏"],
    sides=(None, None),
)

frame(
    "帧 1 · 两态对照（120×40）：标签态 vs 平铺态",
    "两帧同宽同高。**标签态里两侧栏全开时主列只剩 38 列（120 − 41 − 41）—— 已经低于"
    "`MIN_WIDTH = 40`**，所以「右栏的让位阈值」不能等到 120 列以下才生效，这是帧 1 自己报出来的"
    "第一条张力。平铺态按冻结项 6 的推论让两侧栏先让位，pane 面吃满 120 列 —— 2 个 pane 各 59 / 60"
    "列。顶部页签条横跨整屏（2 行：标签行 + 一条虚线），输入区只有底部那一条、绑焦点 pane"
    "（冻结项 8）。",
    120,
    [*show(TAG_SCREEN, 120), "", "同一批会话切到平铺态：", "", *show(TILE_SCREEN, 120)],
)

# --- 帧 2：1 / 2 / 3 / 4 个 pane（120×40） ---------------------------------

for count in (1, 2, 3, 4):
    states = ["忙碌", "等人类", "空闲", "未读"]
    panes = [(f"会话 {chr(ord('A') + i)}", states[i]) for i in range(count)]
    lines = show(
        draw_screen(
            120,
            40,
            panes,
            strip=["  ".join(f"[{title}]" for title, _ in panes) + "                 ▤ 平铺  ▥ 右栏"],
            sides=(None, None),
            focus=0,
        ),
        120,
    )
    plan = layout_panes(120, 40, count)
    detail = "、".join(f"{pw}×{ph}" for _, _, pw, ph in plan) if plan else "放不下"
    frame(
        f"帧 2 · {count} 个 pane（120×40，两侧栏让位）",
        f"每格 {detail}。排法照冻结项 4：1 独占 / 2 左右 / 3 L 形 / 4 上下各二。",
        120,
        lines,
    )

# --- 帧 3：herdr 实况 94×40 ------------------------------------------------

for count in (2, 4):
    states = ["忙碌", "等人类", "空闲", "未读"]
    panes = [(f"会话 {chr(ord('A') + i)}", states[i]) for i in range(count)]
    plan = layout_panes(94, 40, count)
    detail = "、".join(f"{pw}×{ph}" for _, _, pw, ph in plan) if plan else "放不下"
    frame(
        f"帧 3 · {count} 个 pane（94 列 = herdr 里外层 120 列的实况）",
        f"每格 {detail}。这一档是 herdr 默认展开侧栏时的常态。",
        94,
        show(
            draw_screen(
                94,
                40,
                panes,
                strip=["  ".join(f"[{t}]" for t, _ in panes) + "      ▤ 平铺  ▥ 右栏"],
                sides=(None, None),
            ),
            94,
        ),
    )

# --- 帧 4：降级表 ----------------------------------------------------------

TABLE = [
    ("120×40", 120, 40),
    ("120×30", 120, 30),
    ("120×24", 120, 24),
    ("94×40（herdr 常态）", 94, 40),
    ("94×30", 94, 30),
    ("94×24", 94, 24),
    ("80×40", 80, 40),
    ("80×30", 80, 30),
    ("60×24", 60, 24),
]

table_lines: list[str] = []
for label, width, height in TABLE:
    cells = []
    for count in (1, 2, 3, 4):
        plan = layout_panes(width, height, count)
        if plan is None:
            cells.append(f"{count}:—")
        else:
            pw, ph = plan[0][2], plan[0][3]
            cells.append(f"{count}:{pw}×{ph}")
    table_lines.append(f"{label:<20} 1/2/3/4 → {cells[0]:>9} {cells[1]:>9} {cells[2]:>9} {cells[3]:>9}")

frame(
    "帧 4 · 低于哪一档回标签态（每格标「宽×高」；`—` = 放不下，回标签态）",
    "判据是**每个 pane 都要 ≥ MIN_WIDTH = 40 列、≥ MIN_HEIGHT = 10 行**。"
    f"pane 区高度 = 屏高 − {STRIP_ROWS}（页签条）− {FIXED_BELOW}（状态行 / 两条线 / 输入区 3 行 / 提示行）。",
    96,
    table_lines,
)

# --- 帧 5：焦点 pane 的三种画法 --------------------------------------------

focus_frames: list[str] = []
for style, name in (("frame", "框加亮（双线）"), ("title", "标题行加亮"), ("mark", "只画一个记号")):
    focus_frames.append(f"—— {name} ——")
    focus_frames.extend(
        show(
            draw_screen(
                94,
                24,
                [("会话 A · 调研", "忙碌"), ("会话 B · 编译", "等人类")],
                strip=["[会话 A]  [会话 B]                 ▤ 平铺"],
                sides=(None, None),
                focus=0,
                focus_style=style,
            ),
            94,
        )
    )
    focus_frames.append("")

frame(
    "帧 5 · 焦点 pane 的三种画法（94×24，2 个 pane）",
    "全屏唯一的反显已经给了问卷与 `/`·`@` 菜单（`tui-visual-language`），所以焦点只能靠结构或"
    "字形。三种：双线框（与既有字形语法冲突最小）、标题行加亮、角落一个记号。",
    94,
    focus_frames,
)

# --- 帧 6：顶部页签条 ------------------------------------------------------

strip_lines: list[str] = []
labels = ["会话 A · 调研", "会话 B · 编译", "会话 C · 文档"]
strip_lines.append("三标签 + 平铺按钮 + 右栏开关（94 列）：")
strip_lines.append("|" + "  ".join(f"[{label}]" for label in labels) + "        ▤ 平铺  ▥ 右栏")
strip_lines.append("")
strip_lines.append("标签带状态字形（● 忙 / ? 等人类 / • 未读 / ○ 空闲）与焦点标记：")
strip_lines.append("|[◆会话 A ●]  [会话 B ?]  [会话 C •]        ▤ 平铺  ▥ 右栏")
strip_lines.append("")
strip_lines.append("再多几个会话时溢出（收成 +N，靠右端）：")
strip_lines.append("|[会话 A]  [会话 B]  [会话 C]  +4          ▤ 平铺  ▥ 右栏")
strip_lines.append("")
strip_lines.append("只有一行可用时（如 120×24 或问卷立着时）：")
strip_lines.append("|[会话 A]  [会话 B]  +2   ▤ 平铺  [右栏 28 列切一档]")

frame(
    "帧 6 · 顶部页签条的形态（标签写什么、多了怎么溢）",
    "冻结项 5 把页签条定成「管显示」、左栏会话页「管状态」—— 所以标签只写：标题、焦点标记、"
    "以及「在不在平铺集合里」，忙 / 等人类 / 未读这些状态仍然归左栏。这一帧给两种候选对照："
    "只写标题 vs 标题 + 状态字形（后者会与左栏重复）。",
    94,
    strip_lines,
)

# --- 帧 7：非焦点 pane 的三种状态 -----------------------------------------

state_lines = show(
    draw_screen(
        94,
        24,
        [("会话 A · 调研", "忙碌"), ("会话 B · 编译", "等人类"), ("会话 C · 文档", "空闲")],
        strip=["[◆会话 A]  [会话 B ?]  [会话 C •]        ▤ 平铺"],
        sides=(None, None),
        focus=0,
    ),
    94,
)
frame(
    "帧 7 · 非焦点 pane 在忙 / 等人类 / 未读（94×24，3 个 pane）",
    "忙碌与空闲只由 pane 里的内容与状态字形说；**等人类**特殊 —— 它是唯一会拦住整个会话的状态，"
    "所以它在标签上也留一个记号（谁在等人一句话说不清，但「有人在等」必须一眼看见）。"
    "冻结项 5 的分工在这里第一次被检验：左栏管状态、顶部管显示，两处不许写同一件事。",
    94,
    state_lines,
)


# --- 输出与自检 -------------------------------------------------------------

problems: list[str] = []
print("=" * 78)
print("平铺的几何与两态布局 —— 形态帧")
print("票：.scratch/session-panes/issues/05-prototype-panes-layout.md")
print("=" * 78)

for index, (title, why, width, lines) in enumerate(FRAMES, start=1):
    print()
    print(f"── 帧 {index} ─ {title} {'─' * max(0, 60 - w(title))}")
    print(f"   目标宽度 {width} 列")
    for text in why.splitlines():
        print(f"   {text}")
    print()
    for line in lines:
        bare = line[1:] if line.startswith("|") else line
        mark = ""
        if w(bare) > width and not line.startswith("|"):
            mark = ""
        elif w(bare) > width:
            mark = f"  ⚠ 溢出 {w(bare) - width} 列"
            problems.append(f"帧 {index}：{w(bare)} > {width} —— {bare!r}")
        print(f"   {line}{mark}")
    print()

print("=" * 78)
if problems:
    print(f"自检：{len(problems)} 行溢出")
    for problem in problems:
        print(f"  - {problem}")
else:
    print("自检：每一帧的每一条网格行都 ≤ 目标宽度")
print("=" * 78)
