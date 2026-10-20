#!/usr/bin/env python3
"""会话标题上的关闭按钮：三处落点的形态帧（2026-10-20）。

跑法：`python3 gen_frames.py`（在这个目录里跑）。产出同目录下 `01..07` 七个 `.txt`。

这个脚本干两件事：

1. **复刻 heng 的排版算术** —— `tile_cells`（layout.rs:882）、`top_tab_entries`
   （tui.rs:4838）、`fit_title`（tui.rs:10140）、`session_line`（tui.rs:10057）。
   帧里的列位置、命中矩形、截断宽度全是算出来的，不是手摆的。
2. **显示列自检** —— 每帧每行的显示列宽必须 ≤ 帧宽；每个 `X` 必须落在算出来的那一列上；
   每个预算必须与源码里的常数一致。不符就 `assert` 报错退出。

## 字形替身

帧里的 **`X` = 红色的 ✖（U+2716）**。用 ASCII 替身是为了**列账不依赖读者终端的字体设置**：
`✖` 的 East Asian Width 是 **N（Neutral）**——恒 1 列；但同一族里 `×`（U+00D7）是
**A（Ambiguous）**，在 CJK 字体 / 宽字符的终端里会变 2 列。替身让每个 `.txt` 在任何终端里
都对齐。

## 复刻的源码事实（一手，帧按这些算）

- 平铺格：标题画在 `rect.x + 1`（tui.rs:11440），标题先按 `rect.width - 4` 截断（tui.rs:11430）。
- 平铺格几何：`left = (w-1)/2`，`right = w - (left+1)`，格间空 1 列（layout.rs:882-913）。
- 顶部页签条：标签从 `area.x + 1` 起、每个后空 1 列；右端 `▤ 平铺` / `▥ 右栏`；
  标签可用宽 `room = tiling_x - (area.x + 2)`；标题截断 24 列（TOP_TAB_TITLE_MAX）；
  放不下从尾部收成 `+N`（tui.rs:9618-9646、4838-4861）。
- 左栏会话页：行 = 标记 1 + 标题 27/21 + 间隔 1 + 时间 11/5（tui.rs:9868-9869）。
"""

import unicodedata
from pathlib import Path

HERE = Path(__file__).resolve().parent

# --- 显示列宽 ---------------------------------------------------------------

def cw(ch: str) -> int:
    """一个字符占几个终端列。宽字符（W/F）两列，其余一列。"""
    return 2 if unicodedata.east_asian_width(ch) in ("W", "F") else 1


def dw(text: str) -> int:
    """一段文字占几个终端列。"""
    return sum(cw(c) for c in text)


def truncate_columns(text: str, width: int) -> str:
    """width.rs::truncate_columns"""
    out, used = "", 0
    for ch in text:
        step = cw(ch)
        if used + step > width:
            break
        out += ch
        used += step
    return out


def rjust_cols(text: str, width: int) -> str:
    """按**显示列**右对齐（str.rjust 按字符数，CJK 上会算错）。"""
    return " " * max(0, width - dw(text)) + text


def c1(x: int) -> int:
    """0 基的列号 → 人读的 1 基列号。帧里说的「第 N 列」一律是 1 基。"""
    return x + 1


def fit_title(title: str, width: int) -> str:
    """tui.rs::fit_title —— 超宽时尾一个 `…`。"""
    if dw(title) <= width:
        return title
    return truncate_columns(title, width - 1) + "…"


# --- 帧的网格 ---------------------------------------------------------------

class Grid:
    """一块屏幕。宽字符占两列，第二格放占位符，于是「列」与「字符」不会错开。"""

    def __init__(self, w: int, h: int):
        self.w = w
        self.h = h
        self.cells = [[" "] * w for _ in range(h)]

    def put(self, x: int, y: int, text: str) -> None:
        if not (0 <= y < self.h):
            return
        col = x
        for ch in text:
            step = cw(ch)
            if 0 <= col < self.w:
                if step == 2 and col + 1 >= self.w:
                    # 宽字符放不下整格 —— 留一格空白，不越界（真机也是这样被裁的）
                    self.cells[y][col] = " "
                else:
                    self.cells[y][col] = ch
                    if step == 2:
                        self.cells[y][col + 1] = ""
            col += step

    def rule(self, y: int, x0: int, x1: int) -> None:
        """paint_rule —— `┄`，从 x0 到 x1（不含 x1）。"""
        self.put(x0, y, "┄" * max(0, x1 - x0))

    def at(self, x: int, y: int) -> str:
        if 0 <= y < self.h and 0 <= x < self.w:
            return self.cells[y][x]
        return " "

    def line(self, y: int) -> str:
        return "".join(self.cells[y]).rstrip()

    def dump(self) -> list:
        return [self.line(y) for y in range(self.h)]


def ruler(w: int) -> list:
    """两行列尺：上行每 10 列给十位，下行给个位。"""
    tens = "".join(str((i // 10) % 10) if i % 10 == 0 else " " for i in range(w))
    ones = "".join(str(i % 10) for i in range(w))
    return [tens, ones]


# --- 复刻 heng 的版面算术 ---------------------------------------------------

MIN_TILE_WIDTH = 32
MIN_TILE_HEIGHT = 10
TOP_TAB_TITLE_MAX = 24
SIDEBAR_WIDE = 40
SIDEBAR_NARROW = 28
SIDEBAR_WIDE_FROM = 120
SIDEBAR_NARROW_FROM = 80
RAIL_WIDE = 60
RAIL_NARROW = 42
RAIL_WIDE_FROM = 142
RAIL_HIDDEN_BELOW = 80
MIN_MAIN_WIDTH = 40
CHROME = 4
MIN_INPUT_ROWS = 3
SIDEBAR_TOP_GAP = 1
TAB_ROWS = 3
TAB_RULE_ROWS = 1


def tile_cells(pool_x: int, pool_y: int, pool_w: int, pool_h: int, count: int) -> list:
    """layout.rs::tile_cells —— 返回**绝对**坐标的 [(x, y, w, h)]，顺序即视觉顺序。"""
    left = (pool_w - 1) // 2
    right = pool_w - (left + 1)
    top = (pool_h - 1) // 2
    bottom = pool_h - (top + 1)
    if count == 1:
        cells = [(pool_x, pool_y, pool_w, pool_h)]
    elif count == 2:
        cells = [(pool_x, pool_y, left, pool_h),
                 (pool_x + left + 1, pool_y, right, pool_h)]
    elif count == 3:
        cells = [(pool_x, pool_y, left, pool_h),
                 (pool_x + left + 1, pool_y, right, top),
                 (pool_x + left + 1, pool_y + top + 1, right, bottom)]
    else:
        cells = [(pool_x, pool_y, left, top),
                 (pool_x + left + 1, pool_y, right, top),
                 (pool_x, pool_y + top + 1, left, bottom),
                 (pool_x + left + 1, pool_y + top + 1, right, bottom)]
    for x, y, w, h in cells:
        assert w >= MIN_TILE_WIDTH and h >= MIN_TILE_HEIGHT, (
            f"格 {w}×{h} 低于下限 {MIN_TILE_WIDTH}×{MIN_TILE_HEIGHT}：平铺会退回标签态")
    return cells


def main_width(screen_w: int, sidebar: int | None, rail: int | None) -> int:
    """layout.rs::main_width —— 侧栏连同各自分隔线那一列一起扣。"""
    return screen_w - (0 if sidebar is None else sidebar + 1) - (0 if rail is None else rail + 1)


def right_rail_for(width: int, sidebar: int | None) -> int | None:
    """layout.rs::right_rail_for"""
    if width < RAIL_HIDDEN_BELOW:
        return None
    tier = RAIL_WIDE if width >= RAIL_WIDE_FROM else RAIL_NARROW
    main_room = width - (0 if sidebar is None else sidebar + 1)
    return tier if main_room >= tier + 1 + MIN_MAIN_WIDTH else None


def laid_out(width: int, sidebar_wanted: bool, right_wanted: bool):
    """layout.rs::laid_out —— 右栏优先，左栏为它降一档。"""
    sidebar = None
    if sidebar_wanted:
        if width >= SIDEBAR_WIDE_FROM:
            sidebar = SIDEBAR_WIDE
        elif width >= SIDEBAR_NARROW_FROM:
            sidebar = SIDEBAR_NARROW
    if (right_wanted and sidebar == SIDEBAR_WIDE
            and right_rail_for(width, sidebar) is None
            and right_rail_for(width, SIDEBAR_NARROW) is not None):
        sidebar = SIDEBAR_NARROW
    rail = right_rail_for(width, sidebar) if right_wanted else None
    return sidebar, rail


def top_tabs_area(main_x: int, main_w: int, screen_h: int):
    """顶部页签条：区域 + 行数（屏高 < 20 收成 1 行）+ 标签可用宽。"""
    rows = 1 if screen_h < 20 else 2
    area_x, area_w = main_x, main_w
    right = area_x + area_w
    rail_x = right - (dw("▥ 右栏") + 1)
    tiling_x = rail_x - (dw("▤ 平铺") + 2)
    room = tiling_x - (area_x + 2)
    return {"x": area_x, "w": area_w, "rows": rows, "rail_x": rail_x,
            "tiling_x": tiling_x, "room": room, "right": right}


def top_tab_entries(titles: list, focused: int, hover: int, area: dict, version: str):
    """tui.rs::top_tab_entries —— 返回 (标签列表, 藏起来的个数, `+N` 的列)。

    version = "A"：焦点标记 `◆` 保留，悬浮时标签里**多**出 `✖` → 标签宽 +2。
    version = "B"：焦点标记退场，悬浮时 `✖` **顶替**它那一列 → 标签宽不变。
    """
    entries = []
    used = 0
    x = area["x"] + 1
    for index, raw in enumerate(titles):
        title = fit_title(raw, TOP_TAB_TITLE_MAX)
        if version == "A":
            mark = "◆" if index == focused else " "
            body = f"[{mark} "
            if index == hover:
                body += "X "
        else:
            body = "["
            if index == hover:
                body += "X "
            else:
                body += "  "
        text = body + title + "]"
        width = dw(text)
        if used + width + 1 > area["room"]:
            break
        used += width + 1
        entries.append({"x": x, "text": text, "width": width, "index": index})
        x += width + 1
    hidden = len(titles) - len(entries)
    more_x = x
    if hidden > 0:
        more_w = dw(f"+{hidden}")
        if used + more_w > area["room"] and entries:
            entries.pop()
            hidden += 1
            more_x = entries[-1]["x"] + entries[-1]["width"] + 1
        more_text = f"+{hidden}"
    else:
        more_text = ""
        more_w = 0
    return entries, hidden, more_x, more_text, more_w


def session_row(mark: str, title: str, time_text: str, tier: int,
                hovered: bool, variant: str, mark_shown: bool = True):
    """会话页的一行 —— 照 tui.rs::session_line 的列账。

    variant:
      "front"  名字前：`✖` **顶替标记列**（1 列）+ 一个空格 → 标题起点第 2 列、预算 −1
      "back"   名字后：`✖` 贴在标题尾 → 标题起点不变、预算 −1
      "keep"   标记留着：`✖` 插在标记与名字之间 → 标题起点右移 2 列、预算 −2
      "none"   不悬浮（现状）
    """
    wide = tier >= 40
    title_w, time_w = (27, 11) if wide else (21, 5)
    if not mark_shown and mark == "▍":
        # 版 B：前台标记 `▍` 退场（前台由标题的 LIVE 绿认）；`●`/`?` 留着 ——
        # 它们说的是「这一行在跑 / 等人类」，全局读数不替它们说。
        mark = " "
    if not hovered:
        head = mark if mark_shown else " "
        title_budget = title_w
        x_title = 1
        x_x = None
    elif variant == "front":
        head = "X "
        title_budget = title_w - 1
        x_title = 2
        x_x = 0
    elif variant == "back":
        head = mark if mark_shown else " "
        title_budget = title_w - 1
        x_title = 1
        x_x = None
    elif variant == "keep":
        head = (mark if mark_shown else " ") + "X "
        title_budget = title_w - 2
        x_title = 3
        x_x = 1
    else:
        raise ValueError(variant)
    shown = fit_title(title, title_budget)
    # 标题段：占 x_title 起 title_budget 列，不足补空格（与 Rust 的 title_pad 同义）
    title_seg = shown + " " * (title_budget - dw(shown))
    pieces = [head]
    if variant == "back" and hovered:
        # `✖` 贴在标题尾：标题段之后那 1 列
        x_x = x_title + title_budget
        pieces.append(title_seg)
        pieces.append("X")
    else:
        pieces.append(title_seg)
    body = "".join(pieces)
    body = body + " " + rjust_cols(time_text, time_w)
    return {
        "text": body,
        "x_x": x_x,
        "x_title": x_title,
        "title_budget": title_budget,
        "title": shown,
        "time": time_text,
        "time_w": time_w,
    }


# --- 场景：屏幕 + 注释 ------------------------------------------------------

class Scene:
    def __init__(self, w: int, h: int, note: str):
        self.g = Grid(w, h)
        self.w = w
        self.h = h
        self.note = note
        self.lines = []
        self.checks = []

    def add(self, text: str = "") -> None:
        self.lines.append(text)

    def check(self, ok: bool, what: str) -> None:
        self.checks.append((ok, what))
        assert ok, what

    def render(self) -> list:
        out = list(self.lines)
        out.append("列尺（上行十位 / 下行个位）：")
        out.extend(ruler(self.w))
        out.append("")
        out.extend(self.g.dump())
        out.append("")
        return out


def caret(scene: Scene, y: int, x: int, width: int, label: str) -> None:
    """在帧下面画一行 `^`，指着命中矩形的那几列。"""
    assert scene.g.at(x, y) == "X", f"第 {y} 行第 {x} 列不是 X（是 {scene.g.at(x,y)!r}）"
    assert x + width <= scene.w
    scene.add("    " + " " * x + "^" * width + "  " + label)
    scene.add("")


# --- 帧 01：平铺态两格 · 120×40 ---------------------------------------------

TILE_TITLES = [
    "调研终端鼠标移动事件的真实行为：谁会报、报多少",
    "把状态行那段读数再压一压，顺便把会话页三档标题色也对齐",
]


def draw_tiles(scene: Scene, cells, pool_x, pool_y, pool_w, pool_h, hover_index,
               focus_index, titles, main_x, main_w, screen_h, rail=None):
    """画平铺态的格：分隔线 + 标题行 + 几行正文。"""
    # 分隔线（tui.rs:11533）
    if len(cells) >= 2:
        x = cells[1][0] - 1
        for y in range(pool_y, pool_y + pool_h):
            scene.g.put(x, y, "┆")
    if len(cells) >= 3:
        y = cells[2][1] - 1
        start = pool_x if len(cells) >= 4 else cells[1][0]
        scene.g.rule(y, start, pool_x + pool_w)
    for index, (x, y, w, h) in enumerate(cells):
        title = fit_title(titles[index], w - 4)
        hovered = index == hover_index
        mark = "X" if hovered else " "        # `◆`/`·` 退场，✖ 顶替那一列
        scene.g.put(x + 1, y, f"{mark} {title}")
        # 正文几行
        body = [
            "  用户",
            "        这个按钮到底长什么样",
            "  衡",
            "  悬浮才出现，点它就关掉那一会话。",
        ]
        for offset, line in enumerate(body):
            # 真机把窗格里的文字裁在格内 —— 这里也裁，列账才对得上
            scene.g.put(x + 1, y + 2 + offset, truncate_columns(line, w - 1))
    return {}


def frame_tiles_120x40() -> list:
    out = []
    screen_w, screen_h = 120, 40
    sidebar, rail = laid_out(screen_w, False, True)
    main_x = 0 if sidebar is None else sidebar + 1
    main_w = main_width(screen_w, sidebar, rail)
    pool_h = screen_h - CHROME - MIN_INPUT_ROWS
    cells = tile_cells(main_x, 0, main_w, pool_h, 2)
    out.append("=" * 100)
    out.append("帧 01 · 平铺态（Ctrl-R）· 两格 · 120×40")
    out.append("=" * 100)
    out.append(f"版面：左栏收起（Ctrl-O 关）· 右栏开（{rail} 档）· 主列 = {main_w} 列"
               f"（120 − {rail} − 1 分隔）")
    out.append(f"平铺池 = 主列 {main_w}×{pool_h}（转录 {pool_h} 行 = 40 − {CHROME} 外壳 − {MIN_INPUT_ROWS} 输入）")
    out.append(f"格几何：left = ({main_w}−1)/2 = {cells[0][2]}，right = {main_w} − {cells[0][2]+1} = {cells[1][2]}"
               f"；格间空 1 列（第 {cells[1][0]-1} 列画 `┆`）")
    out.append(f"标题行：字形落在**格的第 2 列**（0 基 rect.x+1），标题从**格的第 4 列**起"
               f"（0 基 rect.x+3），预算 = 格宽 − 4 = {cells[0][2]-4}")
    out.append("本帧的 `X` = 红色的 ✖（U+2716，East Asian Width = N，恒 1 列）")
    out.append("")

    frames = [
        (0, "指针悬浮在第 1 格的标题行（格 1 出 `X`，格 2 没有）"),
        (1, "指针悬浮在第 2 格的标题行"),
        (None, "指针不在任何一格的标题行上（都不悬浮）"),
    ]
    for number, (hover, state) in enumerate(frames, start=1):
        scene = Scene(screen_w, screen_h, state)
        scene.add(f"── 帧 {number} / {len(frames)} · {state}")
        scene.add("")
        draw_tiles(scene, cells, 0, 0, main_w, pool_h, hover, 0, TILE_TITLES, main_x, main_w, screen_h)
        # 右栏（42 档）
        rail_x = main_x + main_w + 1
        scene.g.rule(0, rail_x, rail_x + rail)
        scene.g.put(rail_x, 1, "轨迹┆流程" + "┄" * (rail - dw("轨迹┆流程")))
        scene.g.rule(2, rail_x, rail_x + rail)
        for i, line in enumerate([
            "  回合 1 · 49 s",
            "   bash ✓ cargo test",
            "   read ✓ tui.rs",
            "   edit ✓ layout.rs",
            "  回合 2 · 12 s",
            "   bash ✓ cargo fmt",
        ]):
            scene.g.put(rail_x, 3 + i, line)
        # 底部三行（状态行 / 规则 / 输入 / 规则 / 提示行）
        status_y = pool_h
        scene.g.put(main_x, status_y,
                    "deepseek-v4 · high ┆ workspace ┆ 上下文 12% ┆ 🌕 就绪   另有 1 个在跑")
        scene.g.rule(status_y + 1, main_x, main_x + main_w)
        scene.g.put(main_x, status_y + 2, "把状态行那段读数再压一压")
        scene.g.rule(status_y + 4, main_x, main_x + main_w)
        scene.g.put(main_x, status_y + 5,
                    "enter 发送 · esc 取消 · shift+tab 模式 · ctrl-v 右栏 · ctrl-r 平铺 · ctrl-o 左栏")
        for row in scene.g.dump():
            scene.check(dw(row) <= screen_w, f"帧 {number} 有一行超宽：{dw(row)} > {screen_w}")
        out.extend(scene.render())
        out.append(f"命中矩形（帧 {number}）")
        for index, (x, y, w, h) in enumerate(cells):
            if hover is not None and index == hover:
                scene2 = Scene(screen_w, screen_h, "")
                scene2.g = scene.g
                caret(scene2, y, x + 1, 1, f"格 {index+1} 的 `X`：第 {c1(x+1)} 列（1 列宽）★红色")
                out.extend(scene2.lines)
            out.append(f"  格 {index+1} · 标题：第 {c1(x+3)} 列起，预算 {w-4} 列"
                       + ("（本帧被截断，尾 `…`）" if dw(fit_title(TILE_TITLES[index], w-4)) == w-4 else ""))
        out.append("  命中判定：整条标题行都算悬浮（frozen 9），`X` 那一列才是点击的落点")
        out.append("")
    return out


# --- 帧 02：平铺态窄档与多格 -----------------------------------------------

def frame_tiles_94x40() -> list:
    out = []
    screen_w, screen_h = 94, 40
    sidebar, rail = laid_out(screen_w, True, False)
    main_x = sidebar + 1
    main_w = main_width(screen_w, sidebar, rail)
    pool_h = screen_h - CHROME - MIN_INPUT_ROWS
    cells2 = tile_cells(main_x, 0, main_w, pool_h, 2)
    out.append("=" * 100)
    out.append("帧 02 · 平铺态 · 94×40（herdr 常态宽度）")
    out.append("=" * 100)
    out.append(f"版面：左栏开（{sidebar} 档，94 < 120）· 右栏关（右栏在 94 列上放不下："
               f"主列只剩 {main_w} < {RAIL_NARROW}+1+{MIN_MAIN_WIDTH}）")
    out.append(f"主列 = 94 − {sidebar} − 1 = {main_w} 列；平铺池 = {main_w}×{pool_h}")
    out.append(f"两格：left = ({main_w}−1)/2 = {cells2[0][2]}，right = {cells2[1][2]}"
               f"（格宽下限 {MIN_TILE_WIDTH}，本帧正好卡在下限上）")
    out.append(f"标题预算 = 格宽 − 4 = {cells2[0][2]-4}；`X` 落在格的第 2 列（0 基 rect.x+1），"
               f"标题从格的第 4 列起（0 基 rect.x+3）")
    out.append("")
    out.append("注：94 列若把右栏也打开，平铺池只剩 51 列 → 每格 25 列 < 32 → 平铺放不下，"
               "自动退回标签态（layout.rs:877-913）。")
    out.append("")

    def scene_for(cells, hover_index, state, titles):
        scene = Scene(screen_w, screen_h, state)
        scene.add(f"── {state}")
        scene.add("")
        draw_tiles(scene, cells, main_x, 0, main_w, pool_h, hover_index, 0, titles,
                   main_x, main_w, screen_h)
        # 左栏（28 档：文字身份 1 行 + 页签条 3 行 + 页区）
        scene.g.put(0, 1, "heng 0.1.0")
        scene.g.rule(2, 0, sidebar + 1)
        left_tabs = "会话┆调用量┆todo┆文件┆改动"
        scene.g.put(0, 3, left_tabs + "┄" * max(0, sidebar - dw(left_tabs)))
        scene.g.rule(4, 0, sidebar + 1)
        narrow_head = "3 个 · 2 个在跑"
        new_at = sidebar - dw("新建")
        scene.g.put(0, 5, narrow_head + " " * (new_at - dw(narrow_head)) + "新建")
        for i, (mark, title, time_text, _note) in enumerate([
            ("▍", "把状态行那段读数再压一压", "3分", ""),
            ("●", "调研终端鼠标移动事件的真实行为", "刚刚", ""),
            (" ", "编译与测试", "2时", ""),
        ]):
            row = session_row(mark, title, time_text, sidebar, False, "front")
            scene.g.put(0, 6 + i, row["text"])
        status_y = pool_h
        scene.g.put(main_x, status_y,
                    "deepseek-v4 · high ┆ workspace ┆ 上下文 12% ┆ 🌕 就绪   另有 1 个在跑")
        scene.g.rule(status_y + 1, main_x, main_x + main_w)
        scene.g.put(main_x, status_y + 2, "这个按钮到底长什么样")
        scene.g.rule(status_y + 4, main_x, main_x + main_w)
        scene.g.put(main_x, status_y + 5,
                    "enter 发送 · esc 取消 · ctrl-v 右栏 · ctrl-r 平铺 · ctrl-o 左栏")
        for row in scene.g.dump():
            scene.check(dw(row) <= screen_w, f"有一行超宽：{dw(row)} > {screen_w}")
        return scene

    # 帧 1：两格，悬浮第 1 格
    scene = scene_for(cells2, 0, "帧 1 / 3 · 两格 · 悬浮第 1 格（格宽 32，`X` 与标题各占多少）",
                      TILE_TITLES)
    out.extend(scene.render())
    out.append("命中矩形（帧 1）")
    for index, (x, y, w, h) in enumerate(cells2):
        if index == 0:
            probe = Scene(screen_w, screen_h, "")
            probe.g = scene.g
            caret(probe, y, x + 1, 1, f"格 1 的 `X`：第 {c1(x+1)} 列（1 列宽）★红色")
            out.extend(probe.lines)
        out.append(f"  格 {index+1} · 悬浮时 `X` 落在第 {c1(x+1)} 列 · 标题：第 {c1(x+3)} 列起，"
                   f"预算 {w-4} 列")
    out.append(f"  格宽 {cells2[0][2]} 的列账：`X` 1 列 + 空格 1 列 + 标题 {cells2[0][2]-4} 列 = "
               f"{2 + cells2[0][2]-4} 列，格内右侧还余 {cells2[0][2] - (2 + cells2[0][2]-4) - 1} 列")
    out.append("")

    # 帧 2：三格 L 形
    cells3 = tile_cells(main_x, 0, main_w, pool_h, 3)
    scene = scene_for(cells3, 1, "帧 2 / 3 · 三格 L 形 · 悬浮右上那一格", TILE_TITLES + ["周会纪要"])
    out.extend(scene.render())
    out.append("命中矩形（帧 2）")
    for index, (x, y, w, h) in enumerate(cells3):
        if index == 1:
            probe = Scene(screen_w, screen_h, "")
            probe.g = scene.g
            caret(probe, y, x + 1, 1, f"格 2 的 `X`：第 {c1(x+1)} 列（1 列宽）★红色")
            out.extend(probe.lines)
        out.append(f"  格 {index+1} · 第 {y+1} 行 · 悬浮时 `X` 落在第 {c1(x+1)} 列 · "
                   f"标题：第 {c1(x+3)} 列起，预算 {w-4} 列")
    out.append("")

    # 帧 3：四格 2×2（120×40）
    screen_w3 = 120
    sidebar3, rail3 = laid_out(screen_w3, False, True)
    main_w3 = main_width(screen_w3, sidebar3, rail3)
    pool_h3 = screen_h - CHROME - MIN_INPUT_ROWS
    cells4 = tile_cells(0, 0, main_w3, pool_h3, 4)
    scene = Scene(screen_w3, screen_h, "")
    scene.add("── 帧 3 / 3 · 四格 2×2 · 120×40（同一份版面，主列 77 列 → 每格 38 列）· 悬浮第 3 格")
    scene.add("")
    draw_tiles(scene, cells4, 0, 0, main_w3, pool_h3, 2, 0,
               TILE_TITLES + ["周会纪要与下一步的分工", "跑一遍全量测试"], 0, main_w3, screen_h)
    rail_x = main_w3 + 1
    scene.g.rule(0, rail_x, rail_x + rail3)
    scene.g.put(rail_x, 1, "轨迹┆流程" + "┄" * (rail3 - dw("轨迹┆流程")))
    scene.g.rule(2, rail_x, rail_x + rail3)
    for i, line in enumerate(["  回合 1 · 49 s", "   bash ✓ cargo test", "   read ✓ tui.rs"]):
        scene.g.put(rail_x, 3 + i, line)
    status_y = pool_h3
    scene.g.put(0, status_y, "deepseek-v4 · high ┆ workspace ┆ 上下文 12% ┆ 🌕 就绪   另有 1 个在跑")
    scene.g.rule(status_y + 1, 0, main_w3)
    scene.g.put(0, status_y + 2, "这个按钮到底长什么样")
    scene.g.rule(status_y + 4, 0, main_w3)
    scene.g.put(0, status_y + 5, "enter 发送 · esc 取消 · ctrl-v 右栏 · ctrl-r 平铺 · ctrl-o 左栏")
    for row in scene.g.dump():
        scene.check(dw(row) <= screen_w3, f"帧 3 有一行超宽：{dw(row)}")
    out.extend(scene.render())
    out.append("命中矩形（帧 3）")
    for index, (x, y, w, h) in enumerate(cells4):
        if index == 2:
            probe = Scene(screen_w3, screen_h, "")
            probe.g = scene.g
            caret(probe, y, x + 1, 1, f"格 3 的 `X`：第 {c1(x+1)} 列（1 列宽）★红色")
            out.extend(probe.lines)
        out.append(f"  格 {index+1} · 第 {y+1} 行 · 悬浮时 `X` 落在第 {c1(x+1)} 列 · "
                   f"标题：第 {c1(x+3)} 列起，预算 {w-4} 列")
    out.append("")
    return out


# --- 帧 03：顶部页签条 -----------------------------------------------------

TAB_TITLES_3 = ["读数再压一压", "调研鼠标移动事件", "编译与测试"]
TAB_TITLES_8 = [
    "把状态行那段读数再压一压",
    "调研终端鼠标移动事件的真实行为",
    "编译与测试",
    "文档与记录",
    "迁移到新仓库",
    "审查这批改动",
    "周会纪要",
    "跑一遍全量测试",
]


def draw_tab_strip(scene: Scene, area: dict, entries, more_x, more_text, rail=True):
    for entry in entries:
        scene.g.put(entry["x"], 0, entry["text"])
    if more_text:
        scene.g.put(more_x, 0, more_text)
    if rail:
        scene.g.put(area["tiling_x"], 0, "▤ 平铺")
        scene.g.put(area["rail_x"], 0, "▥ 右栏")
    if area["rows"] >= 2:
        scene.g.rule(1, area["x"], area["right"])


def frame_tab_strip() -> list:
    out = []
    screen_w, screen_h = 120, 24
    sidebar, rail = laid_out(screen_w, False, False)
    main_x = 0 if sidebar is None else sidebar + 1
    main_w = main_width(screen_w, sidebar, rail)
    area = top_tabs_area(main_x, main_w, screen_h)
    out.append("=" * 100)
    out.append("帧 03 · 标签态（不平铺）· 顶部页签条 · 120×24")
    out.append("=" * 100)
    out.append(f"版面：左栏收起 · 右栏关 → 主列 = {main_w} 列；页签条区域 = 主列全宽，{area['rows']} 行"
               f"（屏高 {screen_h} ≥ 20 → 第 0 行标签 + 第 1 行 `┄`）")
    out.append(f"右端：▥ 右栏 在第 {c1(area['rail_x'])} 列，▤ 平铺 在第 {c1(area['tiling_x'])} 列；"
               f"标签可用宽 room = {area['tiling_x']} − ({area['x']} + 2) = {area['room']} 列")
    out.append(f"标签从第 {c1(area['x']+1)} 列起、每个后空 1 列；标题截断 {TOP_TAB_TITLE_MAX} 列")
    out.append("")

    def scene_for(titles, focused, hover, version, state):
        scene = Scene(screen_w, screen_h, state)
        scene.add(f"── {state}")
        scene.add("")
        entries, hidden, more_x, more_text, more_w = top_tab_entries(
            titles, focused, hover, area, version)
        draw_tab_strip(scene, area, entries, more_x, more_text)
        # 主列其余部分
        pool_h = screen_h - area["rows"] - CHROME - MIN_INPUT_ROWS
        for i, line in enumerate([
            "用户",
            "      终端鼠标移动事件到底会不会报",
            "衡",
            "多数终端实现了 ?1003h；herdr 默认自己抓鼠标，不放行。",
            "",
            "所以 `✖` 只在悬浮时出现 —— 不做常显的降级。",
        ]):
            scene.g.put(0, area["rows"] + 1 + i, line)
        status_y = screen_h - 7
        scene.g.put(0, status_y, "deepseek-v4 · high ┆ workspace ┆ 上下文 12% ┆ 🌕 就绪   另有 1 个在跑")
        scene.g.rule(status_y + 1, 0, screen_w)
        scene.g.put(0, status_y + 2, "终端鼠标移动事件到底会不会报")
        scene.g.rule(status_y + 4, 0, screen_w)
        scene.g.put(0, status_y + 5, "enter 发送 · esc 取消 · ctrl-v 右栏 · ctrl-r 平铺 · ctrl-o 左栏")
        for row in scene.g.dump():
            scene.check(dw(row) <= screen_w, f"有一行超宽：{dw(row)} > {screen_w}")
        return scene, entries, hidden, more_x, more_text

    # 帧 1：三个标签，悬浮中间那个 —— 版 B（推荐）
    scene, entries, hidden, more_x, more_text = scene_for(
        TAB_TITLES_3, 0, 1, "B",
        "帧 1 / 4 · 3 个标签 · 悬浮中间那个 · **版 B**（`◆` 退场，`✖` 顶替它那一列 → 宽度不变）")
    out.extend(scene.render())
    out.append("命中矩形（帧 1）")
    for entry in entries:
        out.append(f"  标签 {entry['index']+1}：第 {c1(entry['x'])} 列起，宽 {entry['width']} 列"
                   f"（`[X 标题]` 与 `[  标题]` 同宽）")
        if entry["index"] == 1:
            probe = Scene(screen_w, screen_h, "")
            probe.g = scene.g
            caret(probe, 0, entry["x"] + 1, 1, f"标签 2 的 `X`：第 {c1(entry['x']+1)} 列（1 列宽）★红色")
            out.extend(probe.lines)
    out.append("  后面的标签没有被推动 —— 标签宽度不变，零抖动")
    out.append("  提醒：这是纯文本帧，看不出颜色。版 B 里焦点那一签只靠 ACCENT + 粗体认 ——"
               "那正是版 B 的代价（焦点标记退场）。")
    out.append("")

    # 帧 2：同样三个标签，版 A（`◆` 保留，`✖` 多占 2 列）
    scene, entries, hidden, more_x, more_text = scene_for(
        TAB_TITLES_3, 0, 1, "A",
        "帧 2 / 4 · 3 个标签 · 悬浮中间那个 · **版 A**（`◆` 保留，`✖` 插在名字前 → 标签宽 +2）")
    out.extend(scene.render())
    out.append("命中矩形（帧 2）")
    for entry in entries:
        out.append(f"  标签 {entry['index']+1}：第 {c1(entry['x'])} 列起，宽 {entry['width']} 列")
        if entry["index"] == 1:
            probe = Scene(screen_w, screen_h, "")
            probe.g = scene.g
            caret(probe, 0, entry["x"] + 3, 1, f"标签 2 的 `X`：第 {c1(entry['x']+3)} 列（1 列宽）★红色")
            out.extend(probe.lines)
    out.append("  标签 3 被推动了 2 列 —— 每悬浮一次，后面所有标签都跳一下")
    out.append("  （本帧悬浮的是**非焦点**标签，于是是 `[  X 标题]`；悬浮焦点标签则是 `[◆ X 标题]`）")
    out.append("")

    # 帧 3：8 个标签收成 +N（版 B，不悬浮）
    scene, entries, hidden, more_x, more_text = scene_for(
        TAB_TITLES_8, 0, -1, "B",
        "帧 3 / 4 · 8 个标签放不下 → 从尾部收成 `+N` · **版 B** · 都不悬浮")
    out.extend(scene.render())
    out.append("命中矩形（帧 3）")
    for entry in entries:
        out.append(f"  标签 {entry['index']+1}：第 {c1(entry['x'])} 列起，宽 {entry['width']} 列")
    out.append(f"  `+{hidden}`：第 {c1(more_x)} 列起，宽 2 列（不进命中表）")
    out.append("")

    # 帧 4：同样 8 个标签，悬浮第 2 个（版 B）—— 证明 `+N` 与位置都不动
    scene, entries, hidden, more_x, more_text = scene_for(
        TAB_TITLES_8, 0, 1, "B",
        "帧 4 / 4 · 同样 8 个标签 · 悬浮第 2 个 · **版 B**（宽度不变 → `+N` 与位置都不动）")
    out.extend(scene.render())
    out.append("命中矩形（帧 4）")
    for entry in entries:
        out.append(f"  标签 {entry['index']+1}：第 {c1(entry['x'])} 列起，宽 {entry['width']} 列")
        if entry["index"] == 1:
            probe = Scene(screen_w, screen_h, "")
            probe.g = scene.g
            caret(probe, 0, entry["x"] + 1, 1, f"标签 2 的 `X`：第 {c1(entry['x']+1)} 列（1 列宽）★红色")
            out.extend(probe.lines)
    out.append(f"  `+{hidden}`：第 {c1(more_x)} 列起，宽 2 列 —— 与帧 3 一字不差")
    out.append("")
    return out


# --- 帧 04 / 05 / 06：左栏会话页 -------------------------------------------

SESSION_ROWS = [
    # (标记, 标题, 宽档时间(11 列), 窄档时间(5 列), 说明)
    ("▍", "把状态行那段读数再压一压", "3 分钟前", "3分", "前台"),
    ("●", "调研终端鼠标移动事件的真实行为", "刚刚", "刚刚", "已装载 · 在跑"),
    ("?", "编译与测试", "2 小时前", "2时", "已装载 · 等人类"),
    (" ", "周会纪要与下一步的分工", "昨天 14:02", "昨天", "只在盘上（没装载）"),
    (" ", "跑一遍全量测试", "08-12 14:02", "08-12", "只在盘上（没装载）"),
]


def draw_sessions_page(scene: Scene, tier, hover_row, variant, always=False, mark_shown=True):
    """画左栏「会话」页：页顶行 + 组头 + 会话行。返回每行的命中信息。"""
    wide = tier >= 40
    title_w, time_w = (27, 11) if wide else (21, 5)
    if wide:
        head = "已装载 3 个 · 2 个在跑"
        new_at = tier - dw("新建")
    else:
        head = "3 个 · 2 个在跑"
        new_at = tier - dw("新建")
    scene.g.put(0, 0, head + " " * max(0, new_at - dw(head)) + "新建")
    scene.g.put(0, 1, "fortystory/heng")
    hits = []
    for index, (mark, title, wide_time, narrow_time, note) in enumerate(SESSION_ROWS):
        time_text = wide_time if wide else narrow_time
        row = 2 + index
        hovered = always or index == hover_row
        # 只在盘上、没装载的行不显示 `✖`（frozen 7）
        if note.startswith("只在盘上"):
            hovered = False
        info = session_row(mark, title, time_text, tier, hovered, variant,
                            mark_shown=mark_shown)
        scene.g.put(0, row, info["text"])
        hits.append({"row": row, "info": info, "note": note, "hovered": hovered})
    return hits


def frame_sessions_page(tier: int, screen_w: int, screen_h: int, filename_label: str) -> list:
    out = []
    wide = tier >= 40
    title_w, time_w = (27, 11) if wide else (21, 5)
    out.append("=" * 100)
    out.append(f"帧 {filename_label} · 左栏「会话」页 · {tier} 档（{screen_w}×{screen_h} 整屏里的左栏那一列）")
    out.append("=" * 100)
    out.append(f"位置：整屏第 1–{tier} 列；本帧只画**页区**本身"
               + (f"（页区从整屏第 {1 + 1 + 23 + 3} 行起：顶上 1 行空行 + 块字标记 23 行 + 左栏页签条 3 行）"
                  if wide else f"（页区从整屏第 {1 + 1 + 1 + 3} 行起：顶上 1 行空行 + 文字身份 1 行 + 左栏页签条 3 行）"))
    out.append(f"行的列账：标记 1 + 标题 {title_w} + 间隔 1 + 时间 {time_w} = {tier} 列（正好一栏宽）")
    out.append("页顶行：左端读数 + 右端 `新建`；**`关闭` 按钮已删**（frozen 6）")
    out.append("只在盘上、没装载的两行**不显示 `✖`**（frozen 7）")
    out.append("本帧的 `X` = 红色的 ✖（U+2716，East Asian Width = N，恒 1 列）")
    out.append("")

    def scene_for(hover_row, variant, state, always=False, mark_shown=True):
        scene = Scene(tier, 12, state)
        scene.add(f"── {state}")
        scene.add("")
        hits = draw_sessions_page(scene, tier, hover_row, variant, always, mark_shown)
        for row in scene.g.dump():
            scene.check(dw(row) <= tier, f"有一行超出左栏宽度：{dw(row)} > {tier}")
        return scene, hits

    variants = [
        ("front", f"**甲**（推荐）：`✖` 顶替标记列 + 一个空格 → 标题起点第 2 列、预算 {title_w}→{title_w-1}"),
        ("back", f"**乙**：`✖` 贴在标题尾 → 标题起点不变、预算 {title_w}→{title_w-1}"),
    ]
    total = 3 if not wide else 2
    for number, (variant, label) in enumerate(variants, start=1):
        scene, hits = scene_for(1, variant, f"帧 {number} / {total} · {label}")
        out.extend(scene.render())
        out.append("命中矩形")
        for hit in hits:
            info = hit["info"]
            out.append(f"  第 {hit['row']+1} 行（{hit['note']}）："
                       + (f"`X` 第 {c1(info['x_x'])} 列 · " if hit["hovered"] else "无 `X` · ")
                       + f"标题第 {c1(info['x_title'])} 列起，预算 {info['title_budget']} 列"
                       + f"，时间列 {time_w} 列（未被挤）")
            if hit["hovered"]:
                probe = Scene(tier, 12, "")
                probe.g = scene.g
                caret(probe, hit["row"], info["x_x"], 1,
                      f"`X`：第 {c1(info['x_x'])} 列（1 列宽）★红色")
                out.extend(probe.lines)
        out.append("")

    # 窄档降级：若这一档干脆不画 `✖`
    if not wide:
        scene, hits = scene_for(1, "front", "帧 3 / 3 · **窄档降级对照**：这一档干脆不画 `✖`（不推荐）",
                                always=False)
        out.extend(scene.render())
        out.append("命中矩形：这一档没有任何 `✖` 的落点 —— 而页顶的 `关闭` 按钮已经删掉，")
        out.append("所以窄档（28 列）下**没有任何关掉一个会话的入口**。")
        out.append("")
    return out


def frame_always_visible() -> list:
    out = []
    out.append("=" * 100)
    out.append("帧 06 · 「按钮常显」的对照 —— 终端不报鼠标移动事件时的代价形态")
    out.append("=" * 100)
    out.append("前提（事实票 01）：crossterm 的 EnableMouseCapture 自带 ?1003h（全移动上报），")
    out.append("主流终端都实现；但 **herdr 默认不通**（它自己抓鼠标，mouse-first）、GNU screen 不通。")
    out.append("frozen 10 决定「不做降级」—— 悬浮在那些终端里永不出现。本帧画的是那个代价：")
    out.append("**每一行都常驻一个 `✖`**（只在盘上、没装载的行仍然没有）。")
    out.append("")
    for tier, screen_w, screen_h in ((40, 120, 40), (28, 94, 40)):
        title_w, time_w = (27, 11) if tier >= 40 else (21, 5)
        scene = Scene(tier, 12, "")
        scene.add(f"── 左栏 {tier} 档 · `✖` 常显（每一行都在）")
        scene.add("")
        hits = draw_sessions_page(scene, tier, -1, "front", always=True)
        for row in scene.g.dump():
            scene.check(dw(row) <= tier, f"有一行超宽：{dw(row)} > {tier}")
        out.extend(scene.render())
        out.append("命中矩形（常显）")
        for hit in hits:
            info = hit["info"]
            out.append(f"  第 {hit['row']+1} 行（{hit['note']}）："
                       + (f"`X` 第 {c1(info['x_x'])} 列 · " if hit["hovered"] else "无 `X` · ")
                       + f"标题第 {c1(info['x_title'])} 列起，预算 {info['title_budget']} 列")
            if hit["hovered"]:
                probe = Scene(tier, 12, "")
                probe.g = scene.g
                caret(probe, hit["row"], info["x_x"], 1,
                      f"`X`：第 {c1(info['x_x'])} 列（1 列宽）★红色")
                out.extend(probe.lines)
        out.append("")
    out.append("代价一：每一行都多一个红字形，左栏从「一列会话」变成「一列按钮」。")
    out.append("代价二：常显的 `✖` 只能一直占着标记列，于是**每一行的状态标记都没了** ——")
    out.append("          前台（在跑 / 等人类）的那三个字形（`▍`/`●`/`?`）全被顶掉，")
    out.append("          一列会话从此只剩「名字 + 时间 + 一个红叉」，看不出哪一个在跑。")
    out.append("代价三：`✖` 一旦常显，它就不再是「这一下会毁掉东西」的信号，只是又一行字 ——")
    out.append("          于是「关掉」这个动作与「删掉」「取消」「退出」在屏幕上长得一样。")
    out.append("页签条与平铺格同理：每个标签（宽 +2）、每格标题行都常驻一个 `X`，")
    out.append("页签条能放下的标签少 1–2 个，格标题行永远多一列。")
    out.append("")
    return out


# --- 帧 07：焦点标记去留的两版 ---------------------------------------------

def frame_two_versions() -> list:
    out = []
    screen_w, screen_h = 120, 24
    sidebar, rail = laid_out(screen_w, False, False)
    main_x = 0
    main_w = main_width(screen_w, sidebar, rail)
    area = top_tabs_area(main_x, main_w, screen_h)
    out.append("=" * 100)
    out.append("帧 07 · 焦点标记去留的两版对照")
    out.append("=" * 100)
    out.append("版 A：页签条标签保留 `◆`、会话页行保留前台标记 `▍` → `✖` 是**多出来的**一截")
    out.append("版 B：页签条与会话页的标记都退场，焦点只由字重/标题色承担 → `✖` **顶替**那一列")
    out.append("平铺格的标题行两版都是 `✖ 标题`（已定：格标题行只有一个标记位，没有第二个位置可让）")
    out.append("")

    # 页签条两版
    for version, label in (("A", "版 A"), ("B", "版 B")):
        scene = Scene(screen_w, screen_h, "")
        scene.add(f"── 页签条 · {label} · 3 个标签 · 悬浮**焦点**那一签（标签 1）")
        scene.add("")
        entries, hidden, more_x, more_text, more_w = top_tab_entries(
            TAB_TITLES_3, 0, 0, area, version)
        draw_tab_strip(scene, area, entries, more_x, more_text)
        for i, line in enumerate([
            "用户",
            "      页签条上的标签会不会被推动",
            "衡",
            "标签宽度一变，后面的标签就跟着跳 —— 每悬浮一次跳一次。",
        ]):
            scene.g.put(0, area["rows"] + 1 + i, line)
        status_y = screen_h - 7
        scene.g.put(0, status_y, "deepseek-v4 · high ┆ workspace ┆ 上下文 12% ┆ 🌕 就绪")
        scene.g.rule(status_y + 1, 0, screen_w)
        scene.g.put(0, status_y + 2, "页签条上的标签会不会被推动")
        scene.g.rule(status_y + 4, 0, screen_w)
        scene.g.put(0, status_y + 5, "enter 发送 · esc 取消 · ctrl-v 右栏 · ctrl-r 平铺 · ctrl-o 左栏")
        for row in scene.g.dump():
            scene.check(dw(row) <= screen_w, f"有一行超宽：{dw(row)}")
        out.extend(scene.render())
        out.append(f"命中矩形（{label}）")
        for entry in entries:
            if entry["index"] == 0:
                x_x = entry["x"] + (3 if version == "A" else 1)
                probe = Scene(screen_w, screen_h, "")
                probe.g = scene.g
                caret(probe, 0, x_x, 1, f"标签 1 的 `X`：第 {c1(x_x)} 列（1 列宽）★红色")
                out.extend(probe.lines)
            out.append(f"  标签 {entry['index']+1}：第 {c1(entry['x'])} 列起，宽 {entry['width']} 列")
        third = [e for e in entries if e["index"] == 2][0]
        out.append(f"  标签 3 的起点：第 {c1(third['x'])} 列"
                   + ("（版 A 比版 B 右移 2 列 —— 抖动）" if version == "A" else "（与不悬浮时同一位）"))
        out.append("")

    # 会话页两版
    for variant, label, mark_shown in (("keep", "版 A（标记留着）", True),
                                       ("front", "版 B（标记退场，`✖` 顶替）", False)):
        tier = 40
        scene = Scene(tier, 12, "")
        scene.add(f"── 会话页 · {label} · 40 档 · 悬浮第 2 行（`●` 在跑那一行）")
        scene.add("")
        hits = draw_sessions_page(scene, tier, 1, variant, mark_shown=mark_shown)
        for row in scene.g.dump():
            scene.check(dw(row) <= tier, f"有一行超宽：{dw(row)}")
        out.extend(scene.render())
        out.append(f"命中矩形（{label}）")
        for hit in hits:
            info = hit["info"]
            out.append(f"  第 {hit['row']+1} 行（{hit['note']}）："
                       + (f"`X` 第 {c1(info['x_x'])} 列 · " if hit["hovered"] else "无 `X` · ")
                       + f"标题第 {c1(info['x_title'])} 列起，预算 {info['title_budget']} 列")
            if hit["hovered"]:
                probe = Scene(tier, 12, "")
                probe.g = scene.g
                caret(probe, hit["row"], info["x_x"], 1,
                      f"`X`：第 {c1(info['x_x'])} 列（1 列宽）★红色")
                out.extend(probe.lines)
        out.append("  标题被挤掉的列数："
                   + ("2 列（27→25），且起点右移 2 列 —— 悬浮行的标题与别的行不齐"
                      if variant == "keep" else "0 列（27 不变），起点不动 —— 标题始终对齐"))
        out.append("")
    return out


# --- 宽度自检 ---------------------------------------------------------------

def width_audit() -> list:
    """把候选字形的 East Asian Width 摊开 —— 结论进 README。"""
    rows = []
    for ch, name in (("✖", "HEAVY MULTIPLICATION X（提案用的字形）"),
                     ("✕", "MULTIPLICATION X"),
                     ("×", "MULTIPLICATION SIGN"),
                     ("◆", "BLACK DIAMOND（页签条/格标题的旧焦点标记）"),
                     ("·", "MIDDLE DOT（格标题的旧非焦点标记）"),
                     ("▍", "LEFT THREE EIGHTHS BLOCK（会话页的前台标记）"),
                     ("●", "BLACK CIRCLE（会话页的在跑标记）"),
                     ("…", "HORIZONTAL ELLIPSIS（截断的省略号）")):
        eaw = unicodedata.east_asian_width(ch)
        width = 2 if eaw in ("W", "F") else 1
        cjk = 2 if eaw in ("W", "F", "A") else 1
        rows.append((ch, hex(ord(ch)), name, eaw, width, cjk))
    return rows


def main() -> None:
    files = {
        "01-tiles-120x40.txt": frame_tiles_120x40(),
        "02-tiles-94x40.txt": frame_tiles_94x40(),
        "03-tab-strip-120x24.txt": frame_tab_strip(),
        "04-sessions-page-40.txt": frame_sessions_page(40, 120, 40, "04"),
        "05-sessions-page-28.txt": frame_sessions_page(28, 94, 40, "05"),
        "06-always-visible.txt": frame_always_visible(),
        "07-focus-marker-two-versions.txt": frame_two_versions(),
    }
    for name, lines in files.items():
        path = HERE / name
        path.write_text("\n".join(lines) + "\n", encoding="utf-8")
        print(f"写 {name}：{len(lines)} 行")

    print("\n宽度自检（East Asian Width → 终端列）：")
    for ch, code, name, eaw, width, cjk in width_audit():
        print(f"  {ch} {code:>6}  {eaw}  默认 {width} 列 / CJK 档 {cjk} 列   {name}")
    print("\n结论：✖(U+2716) 与 ✕(U+2715) 是 **N（Neutral）→ 恒 1 列**；"
          "×(U+00D7) 才是 **A（Ambiguous）→ CJK 终端里 2 列**。")
    print("本目录所有帧都按 1 列画 `X`（✖ 的替身），所以列账与终端无关。")


if __name__ == "__main__":
    main()
