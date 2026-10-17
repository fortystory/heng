#!/usr/bin/env python3
"""终局外壳：按 session-panes 图 12 张票收口后的决定画的两帧（2026-10-17）。

这不是现状，是 destination 的样子：

- 顶部页签条 2 行、横跨整屏，列**宿主表**、标签只写标题 + 焦点 + 集合成员（票 09）。
- 标签态：左栏 40 + 主列 50 + 右栏 28 —— 票 11 的右栏让位：120 列两侧栏同开时右栏降到 28 档。
- 平铺态：左栏收起、右栏照开，pane 区 91 列，两格各 45（票 08 第 11 条、票 09 第 9 条）。
- 平铺集合是 `Vec<SessionId>`（有序、≤4、不重复）；`[+ 加入]` 格参与布局（票 07）。
- 底部三行（状态行 / 输入区 / 提示行）只跨 pane 区，右栏到底；状态行带全局读数（票 06）。
- 外壳开销 9 行 = 顶部 2 + 状态行 1 + 两条线 2 + 输入区 3 + 提示行 1（票 05）。
"""

import unicodedata

W, H = 120, 40
TOP, BOTTOM = 2, 7
CONTENT = H - TOP - BOTTOM          # 31
LEFT_W, RIGHT_W = 40, 28


def cw(ch: str) -> int:
    return 2 if unicodedata.east_asian_width(ch) in "WF" else 1


def dw(text: str) -> int:
    return sum(cw(ch) for ch in text)


def fit(text: str, width: int) -> str:
    out, used = "", 0
    for ch in text:
        step = cw(ch)
        if used + step > width:
            return (out[:-1] + "…") if out else "…"
        out += ch
        used += step
    return out


def pad(text: str, width: int) -> str:
    text = fit(text, width)
    return text + " " * (width - dw(text))


class Grid:
    def __init__(self) -> None:
        self.cells = [[" "] * W for _ in range(H)]

    def put(self, x: int, y: int, text: str) -> None:
        if not (0 <= y < H):
            return
        col = x
        for ch in text:
            step = cw(ch)
            if 0 <= col < W:
                self.cells[y][col] = ch
                # 宽字符占两列：第二格放占位符，`join` 之后自然消失，
                # 于是「列」与「字符」不会错开（旧帧那个 bug 就出在这里）。
                if step == 2 and col + 1 < W:
                    self.cells[y][col + 1] = ""
            col += step

    def hline(self, y: int, x0: int, x1: int, ch: str = "┄") -> None:
        self.put(x0, y, ch * max(0, x1 - x0 + 1))

    def line(self, y: int) -> str:
        return "".join(self.cells[y]).rstrip()


# --- 各区域 -----------------------------------------------------------------


def top_bar(g: Grid, labels, tiled: bool) -> None:
    """顶部页签条：标签行 + 虚线，横跨整屏。"""
    x = 1
    for title, focused, member in labels:
        mark = "◆" if focused else ("▪" if member else " ")
        tag = f"[{mark} {title}]"
        g.put(x, 0, tag)
        x += dw(tag) + 1
    button = "▤ 平铺（已开）" if tiled else "▤ 平铺"
    right = f"{button}   ▥ 右栏"
    g.put(W - 1 - dw(right), 0, right)
    g.hline(1, 0, W - 1)


def bottom_rows(g: Grid, x0: int, width: int, focus: str, others: int, compact: bool) -> None:
    """状态行 / 输入区 / 提示行：只跨 pane 区。窄档按优先级丢字段（票 06）。"""
    status = (
        "workspace ┆ 上下文 12% ┆ 🌕 就绪"
        if compact
        else f"deepseek-v4 · high ┆ workspace ┆ 上下文 12% ┆ 🌕 就绪"
    )
    if others:
        status += f"   另有 {others} 个在跑"
    g.put(x0 + 1, TOP + CONTENT, pad(fit(status, width - 1), width - 1))
    g.hline(TOP + CONTENT + 1, x0, x0 + width - 1, "─")
    g.put(x0 + 1, TOP + CONTENT + 2, "❯ 把状态行那段读数再压一压")
    g.put(x0 + 1, TOP + CONTENT + 3, "  " + "─" * (width - 3))
    g.put(x0 + 1, TOP + CONTENT + 4, "  （草稿两行，Shift+Enter 换行）")
    g.hline(TOP + CONTENT + 5, x0, x0 + width - 1, "─")
    hints = (
        "enter 发送 · ctrl-j 换行 · esc 取消 · shift+tab 模式 · "
        "PgUp/PgDn 滚动 · ctrl-t 换模型 · ctrl-v 右栏 · ctrl-r 平铺 · ctrl-o 左栏"
    )
    g.put(x0 + 1, TOP + CONTENT + 6, fit(hints, width - 2))


def left_sidebar(g: Grid, rows: int) -> None:
    """左栏：身份 + 页签条 + 会话页（票 09：左栏只管状态）。"""
    g.put(1, TOP, "衡  ·  session-panes")
    g.put(1, TOP + 1, "┄" * (LEFT_W - 2))
    tabs = "会话  调用量  todo  文件  改动"
    g.put(1, TOP + 2, fit(tabs, LEFT_W - 2))
    g.put(1, TOP + 3, f"已装载 3 个 · {rows} 个在跑")
    g.put(1, TOP + 4, "┄" * (LEFT_W - 2))
    lines = [
        "▍会话 A · 调研            12:04",
        " 会话 B · 编译            11:58",
        " 会话 C · 文档            11:41",
        "┄ fortystory/heng ┄┄┄┄┄┄┄┄┄┄",
        " 会话 D · 迁移            10:22",
        " 会话 E · 审查            09:51",
    ]
    for i, line in enumerate(lines):
        g.put(1, TOP + 5 + i, fit(line, LEFT_W - 2))
    for y in range(TOP, H):
        g.put(LEFT_W, y, "│")
    g.put(0, H - 1, "＋7 个会话")


def right_rail(g: Grid, x0: int, title: str) -> None:
    """右栏：轨迹（28 档 = 缩略索引，票 08 第 8 条）。"""
    for y in range(TOP, H):
        g.put(x0 - 1, y, "│")
    g.put(x0 + 1, TOP, fit(f"轨迹 · {title}", RIGHT_W - 2))
    g.put(x0 + 1, TOP + 1, "┄" * (RIGHT_W - 2))
    trace = [
        "回合 1 · 49 s",
        " bash ✓ cargo test",
        " read ✓ tui.rs",
        " edit ✓ layout.rs",
        " bash ✓ cargo fmt",
        "回合 2 · 12 s",
        " bash ✓ cargo test",
        " read ✓ pane.rs",
        "（正在跑）",
    ]
    for i, line in enumerate(trace):
        g.put(x0 + 1, TOP + 2 + i, fit(line, RIGHT_W - 2))


def conversation(g: Grid, x0: int, width: int, title: str, rail_rows: int) -> None:
    """主列对话视图：名字独占一行、用户气泡贴右，回合条在最右一列。"""
    g.put(x0 + 1, TOP + 1, f"衡  ·  {title}")
    bubble = "把状态行那段读了什么再压一压"
    g.put(x0 + width - 1 - dw("用户") - 1, TOP + 3, "用户")
    g.put(x0 + width - 1 - dw(bubble), TOP + 4, bubble)
    g.put(x0 + 1, TOP + 6, "衡")
    body = [
        "那段全局读数收成了「另有 N 个在跑 / M 个等人类」，",
        "焦点那一路的读数一个字没动。状态行现在长这样：",
        "",
        "  deepseek-v4 · high ┆ workspace ┆ 上下文 12% ┆ 🌕 就绪",
        "",
        "平铺态下这段读数仍然全局一份，读的是焦点格。",
    ]
    for i, line in enumerate(body):
        g.put(x0 + 1, TOP + 7 + i, fit(line, width - 3))
    # 回合条：最右 1 列，一格一个单位
    for i in range(rail_rows):
        g.put(x0 + width - 1, TOP + 1 + i, "┆")
    g.put(x0 + width - 1, TOP + 8, "┃")


def tile(
    g: Grid,
    x: int,
    y0: int,
    width: int,
    height: int,
    title: str,
    focused: bool,
    body: str,
) -> None:
    """一格：顶行标题 + 虚线框，内容从第二行起（框架那套是虚线）。"""
    mark = "◆" if focused else "·"
    g.hline(y0, x, x + width - 1)
    g.put(x + 2, y0, f" {mark} {title} ")
    for y in range(y0 + 1, y0 + height):
        g.put(x, y, "┆")
        g.put(x + width - 1, y, "┆")
    for i, line in enumerate(body.splitlines()):
        g.put(x + 2, y0 + 2 + i, fit(line, width - 4))


def show(title: str, g: Grid, width: int = W) -> None:
    print(f"\n── {title} {'─' * max(0, width - dw(title) - 4)}")
    for y in range(H):
        line = g.line(y)
        assert dw(line) <= width, f"第 {y} 行超宽：{dw(line)}"
        print(line)


def frame_tagged() -> Grid:
    g = Grid()
    top_bar(
        g,
        [
            ("会话 A · 调研", True, True),
            ("会话 B · 编译", False, True),
            ("会话 C · 文档", False, True),
        ],
        tiled=False,
    )
    left_sidebar(g, rows=1)
    conversation(g, LEFT_W + 1, W - LEFT_W - RIGHT_W - 2, "会话 A · 调研", 30)
    right_rail(g, W - RIGHT_W, "会话 A · 调研")
    bottom_rows(g, LEFT_W + 1, W - LEFT_W - RIGHT_W - 2, "会话 A", others=1, compact=True)
    return g


def frame_tiled() -> Grid:
    g = Grid()
    top_bar(
        g,
        [
            ("会话 A · 调研", True, True),
            ("会话 B · 编译", False, True),
            ("会话 C · 文档", False, True),
            ("会话 D · 迁移", False, True),
        ],
        tiled=True,
    )
    pane_w = (W - RIGHT_W - 1 - 1) // 2          # 91 → 45
    pane_h = (CONTENT - 1) // 2                  # 31 → 15
    col2 = pane_w + 1
    row2 = TOP + pane_h + 1
    # 上下两排格之间那条横分隔（框架那套是虚线）。
    g.hline(TOP + pane_h, 0, W - RIGHT_W - 2)
    tile(
        g, 0, TOP, pane_w, pane_h, "会话 A · 调研 · 忙碌", True,
        "衡 · deepseek\n用户\n          把读法再压一压\n衡\n"
        "那段读数收成「另有 1 个在跑」。\n\n▍ 正在跑：bash",
    )
    tile(
        g, col2, TOP, pane_w, pane_h, "会话 B · 编译 · 等人类", False,
        "衡 · minimax\n\n  权限门在等你决定：\n\n  bash 要在工作区外写\n"
        "  /tmp/bench.log\n\n  [允许] [总是允许] [拒绝]",
    )
    tile(
        g, 0, row2, pane_w, pane_h, "会话 C · 文档 · 空闲", False,
        "衡 · kimi\n用户\n        把那段说明补一句\n衡\n补好了，在 §3 末。",
    )
    tile(
        g, col2, row2, pane_w, pane_h, "会话 D · 迁移 · 跑完", False,
        "衡 · deepseek\n用户\n     跑一遍全量测试\n衡\n237 个通过，0 失败。",
    )
    right_rail(g, W - RIGHT_W, "会话 A · 调研")
    bottom_rows(g, 0, W - RIGHT_W - 1, "会话 A", others=1, compact=False)
    return g


if __name__ == "__main__":
    show("帧 1 · 标签态（120×40）：顶部页签条 + 左栏 40 + 主列 50 + 右栏 28", frame_tagged())
    show("帧 2 · 平铺态（120×40）：四格 2×2，各 45×15，左栏收起、右栏照开", frame_tiled())
    print("\n自检：每帧每行 ≤ 120 列。")
