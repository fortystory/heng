#!/usr/bin/env python3
"""PROTOTYPE（throwaway）—— 右栏「流程」页的三种形状，42 / 60 两档、三态帧。

要回答的问题：右栏 42×24（宽屏 60×24）里，把 agent 一次回合的循环画成
「当前在哪个节点 + 一句说明」，哪种形状读得动。

为什么不是 HTML 原型：这是 Rust TUI，没有浏览器可挂。仓库既有先例是
`trace-tab/prototype/frames.py` 与 `tui-sidebar/prototype/`：用脚本画文本帧、
判断形状。变体的「浮动底栏切换」在这里的等价物是 `-v a|b|c` 参数。

三种形状**结构不同**（不是配色不同）：

  A 等权全图：五个节点同一权重，当前节点展开子块，每节点带耗时侧注，
    底部说明区与回合统计。信息最全，读起来是「一张图」。
  B 环形图：2×2 节点框 + 四条箭头，成为一个闭合回路（等模型 → 流式 →
    工具 → 下一轮 → 等模型）。结构上突出「循环」，信息最粗。
  C 焦点放大：当前节点一个双线大框，前后各挂一个上下文节点，更早的
    节点压成一行历史。回答「现在在干什么」最快，历史最少。

所有帧都是**形状草稿**：中文按 2 列算（用 east_asian_width），字形取仓库
现有符号表（✓ ○ ◆ │ ├ └ ┄ ┆ ─）加两个新字形（↺ →）。颜色不在文本帧里，
图例见 README.md。

跑：

    python3 .scratch/flow-page/prototype/frames.py          # 全部
    python3 .scratch/flow-page/prototype/frames.py -v a     # 只看 A

这是 prototype，不要进 product。决定记在 README.md 的「决定」里：A 等权全图胜出。
"""

import argparse
import sys
import unicodedata

H = 24  # 右栏全高 = 屏幕高（右栏从屏幕顶起算，顶部页签条只在主列里）


# --- 显示宽度 ---------------------------------------------------------------


def dw(text: str) -> int:
    """显示宽度：全角/宽字符算 2 列；`✓ ◆ ○ ┆ ┄ │ ├ └ ↺ →` 都是 Ambiguous，按 1 列算。"""
    return sum(2 if unicodedata.east_asian_width(ch) in "WF" else 1 for ch in text)


def pad(text: str, width: int) -> str:
    room = width - dw(text)
    assert room >= 0, f"超宽：{text!r} 需要 {dw(text)}，只有 {width}"
    return text + " " * room


def pair(left: str, right: str, width: int) -> str:
    """一行两段：左贴边、右贴边，中间用空格撑满。"""
    room = width - dw(left) - dw(right)
    assert room >= 0, f"超宽：{left!r} + {right!r} 需要 {dw(left) + dw(right)}，只有 {width}"
    return left + " " * room + right


def fit(top: list[str], bottom: list[str], width: int) -> list[str]:
    """顶部块贴顶、底部块贴底，中间补空行；断言总高正好 H。"""
    gap = H - len(top) - len(bottom)
    assert gap >= 0, f"太高：top {len(top)} + bottom {len(bottom)} > {H}"
    lines = top + [""] * gap + bottom
    assert len(lines) == H
    for line in lines:
        assert dw(line) <= width, f"行宽 {dw(line)} > {width}：{line!r}"
    return [pad(line, width) for line in lines]


# --- 共用零件 ---------------------------------------------------------------


def tab_bar(width: int) -> list[str]:
    """右栏页签条：上下两条 `┄`，中间一行标签（选中项在真终端里是 ACCENT+BOLD）。"""
    label = "轨迹┆流程"
    return ["┄" * width, label + "┄" * (width - dw(label)), "┄" * width]


def rule(width: int) -> str:
    """说明区上面那条小节线：内容档实线，两端各留 1 格。"""
    return " " + "─" * (width - 2) + " "


# --- A：等权全图 -------------------------------------------------------------


def variant_a(state: str, width: int) -> list[str]:
    head = {
        "idle": " 空闲",
        "waiting": pair(" 回合 3 · 迭代 1", "⟳ 1.2s", width),
        "tool": pair(" 回合 3 · 迭代 2", "⟳ 3.8s", width),
    }[state]

    top = tab_bar(width) + [head, ""]

    if state == "idle":
        # 空闲时轨道不空着：它显示上一回合走过的完整路径（全部 ✓ + 耗时）。
        top += [
            pair(" ✓ 收到你的话", "0.0s", width),
            " │",
            pair(" ✓ 等模型响应", "2.4s", width),
            " │",
            pair(" ✓ 流式输出", "6.1s · 214 字", width),
            " │",
            pair(" ✓ 工具调用", "1.3s", width),
            " │",
            pair(" ✓ 回合收尾", "3.7s", width),
        ]
        bottom = [
            rule(width),
            " 上一回合已收起。等你发一句话。",
            "",
            " 上一回合: 3 次迭代 · 2 次工具 · 14.2s",
        ]
        return fit(top, bottom, width)

    if state == "waiting":
        top += [
            pair(" ✓ 收到你的话", "0.0s", width),
            " │",
            " ◆ 等模型响应",
            " │   拼请求 · 投影 → 身份 → 裁剪",
            " │   预算预检 · 请求已发出",
            " │",
            " ○ 流式输出",
            " │",
            " ○ 工具调用",
            " │",
            " ○ 回合收尾",
        ]
        bottom = [
            rule(width),
            " 请求已发出，等第一段文本。",
            " 还没有 token。",
            " 本回合 0 次工具 · 上下文 37%",
        ]
        return fit(top, bottom, width)

    # tool：权限门已允许，bash 正在跑。宽档把侧注与统计放宽（窄档拆行、宽档一行）。
    top += [
        pair(" ✓ 收到你的话", "0.0s", width),
        " │",
        pair(" ✓ 等模型响应", "2.4s", width),
        " │",
        pair(" ✓ 流式输出", "6.1s · 214 字", width),
        " │",
        pair(" ◆ 工具调用", "1.3s", width),
        " │   ├ 权限门 · 已允许",
        " │   └ bash · 执行中 0.8s",
        " │",
        " ├─↺ 还有工具调用 → 回到「等模型响应」",
        " │",
        " ○ 回合收尾",
    ]
    if width >= 60:
        bottom = [
            rule(width),
            " bash: cargo test --lib · 已跑 0.8s",
            " 结果回来会打码、截断、追加进流；权限门已允许这一次调用。",
            " 本回合 2 次迭代 · 1 次工具 · 输入 12.4k · 输出 1.8k",
        ]
    else:
        bottom = [
            rule(width),
            " bash: cargo test --lib",
            " 结果回来会打码、截断、追加进流。",
            " 本回合 2 次迭代 · 1 次工具 · 36%",
        ]
    return fit(top, bottom, width)


# --- B：环形图 ---------------------------------------------------------------


def _box(width: int, name: str, marker: str, note: str) -> tuple[list[str], list[str], list[str]]:
    """一个 3 行小框的三行（顶 / 名行 / 注行），宽 `width`。返回 (top, name_row, bottom)。"""
    inner = width - 2
    top = "╭" + "─" * inner + "╮"
    name_row = "│ " + pad(f"{marker} {name}", inner - 2) + " │"
    note_row = "│" + pad(" " + note, inner) + "│"
    bottom = "╰" + "─" * inner + "╯"
    return [top], [name_row], [note_row, bottom]


def variant_b(state: str, width: int) -> list[str]:
    # 两列框 + 中间一条放箭头的缝。
    box_w = 16 if width < 60 else 22
    gap = width - 2 * box_w
    assert gap >= 8, f"{width} 列放不下两个 {box_w} 的框 + 箭头缝"
    left_x, right_x = 0, width - box_w

    def row(left: str, middle: str, right: str) -> str:
        return pad(left, box_w) + pad(middle, gap) + pad(right, box_w)

    if state == "idle":
        marks = {"wait": "○", "stream": "○", "call": "○", "again": "○"}
    elif state == "waiting":
        marks = {"wait": "◆", "stream": "○", "call": "○", "again": "○"}
    else:
        marks = {"wait": "✓", "stream": "✓", "call": "◆", "again": "○"}

    wait = _box(box_w, "等模型响应", marks["wait"], "2.4s" if state != "waiting" else "1.2s")
    stream = _box(box_w, "流式输出", marks["stream"], "6.1s · 214字" if state == "tool" else "")
    call = _box(box_w, "工具调用", marks["call"], "1.3s" if state == "tool" else "")
    again = _box(box_w, "下一轮迭代", marks["again"], "")

    top = tab_bar(width)
    if state == "idle":
        top += [" 空闲", "", " ○ 收到你的话 · 准备请求（上一回合 0.1s）", ""]
        # 空闲：环全暗，不做逐框动画。
    else:
        head = pair(" 回合 3 · 迭代 2", "⟳ 3.8s" if state == "tool" else "⟳ 1.2s", width)
        top += [head, "", " ✓ 收到你的话 · 准备请求（0.1s）", ""]

    top += [
        row(wait[0][0], "", stream[0][0]),
        row(wait[1][0], " " + "─" * (gap - 2) + "▶", stream[1][0]),
        row(wait[2][0], "", stream[2][0]),
        row(wait[2][1], "", stream[2][1]),
        row(" " * 7 + "▲", "", " " * 7 + "▼"),
        row(again[0][0], "", call[0][0]),
        row(again[1][0], "◀" + "─" * (gap - 2) + " ", call[1][0]),
        row(again[2][0], "", call[2][0]),
        row(again[2][1], "", call[2][1]),
    ]

    if state == "idle":
        bottom = [rule(width), " 等你发一句话。", "", " 上一回合: 3 次迭代 · 2 次工具"]
    elif state == "waiting":
        bottom = [
            rule(width),
            " 请求已发出，等第一段文本。",
            " 还没有 token。",
            " 本回合 0 次工具 · 上下文 37%",
        ]
    else:
        bottom = [
            rule(width),
            " 权限门已允许 · bash 执行中 0.8s",
            " 结果回来会打码、截断、追加进流。",
            " 本回合 2 次迭代 · 1 次工具 · 36%",
        ]
    return fit(top, bottom, width)


# --- C：焦点放大 -------------------------------------------------------------


def variant_c(state: str, width: int) -> list[str]:
    box = width - 2  # 框总宽，左右各留 1 格
    inside = box - 2

    def big_box(title: str, rows: list[str], note: str) -> list[str]:
        out = [" " + "╔" + "═" * inside + "╗"]
        out.append(" " + "║" + pad(f" {title}  {note}", inside) + "║")
        out.extend(" " + "║" + pad(f" {row}", inside) + "║" for row in rows)
        out.append(" " + "╚" + "═" * inside + "╝")
        return out

    top = tab_bar(width)

    if state == "idle":
        top += [
            pair(" 空闲", "上一回合 14.2s", width),
            "",
        ]
        top += big_box(
            "○ 上一回合已收起",
            ["等你发一句话。", "上一回合 3 次迭代 · 2 次工具 · 36%"],
            "",
        )
        bottom = [rule(width), " 上一次回合的路径已收起。", "", ""]
        return fit(top, bottom, width)

    if state == "waiting":
        head = pair(" 回合 3 · 迭代 1", "⟳ 1.2s", width)
        top += [
            head,
            "",
            " ✓ 收到你的话 0.0s",
            "",
            " ✓ 准备请求 0.1s",
            "   │",
            "   ▼",
        ]
        top += big_box("◆ 等模型响应", ["拼请求 · 投影 → 身份 → 裁剪", "预算预检 · 请求已发出"], "1.2s")
        top += ["   │", "   ▼", " ○ 流式输出 · 工具调用 · 回合收尾"]
        bottom = [rule(width), " 请求已发出，等第一段文本。", " 还没有 token。", " 本回合 0 次工具 · 上下文 37%"]
        return fit(top, bottom, width)

    # tool
    head = pair(" 回合 3 · 迭代 2", "⟳ 3.8s", width)
    top += [
        head,
        "",
        " ✓ 收到你的话 0.0s · ✓ 等模型响应 2.4s",
        "",
        " ✓ 流式输出 6.1s · 214 字",
        "   │",
        "   ▼",
    ]
    top += big_box("◆ 工具调用", ["权限门 · 已允许", "$ bash · 执行中 0.8s"], "1.3s")
    top += ["   │", "   ▼", " ○ 回合收尾"]
    bottom = [
        rule(width),
        " bash: cargo test --lib",
        " 结果回来会打码、截断、追加进流。",
        " 本回合 2 次迭代 · 1 次工具 · 36%",
    ]
    return fit(top, bottom, width)


# --- 输出 --------------------------------------------------------------------


VARIANTS = {"a": ("A 等权全图", variant_a), "b": ("B 环形图", variant_b), "c": ("C 焦点放大", variant_c)}
STATES = [("idle", "空闲"), ("waiting", "等模型响应"), ("tool", "工具调用（权限已允许）")]


def show(names: list[str], widths: list[int]) -> None:
    for width in widths:
        for key in names:
            label, render = VARIANTS[key]
            for state, state_label in STATES:
                print()
                print("=" * (width + 4))
                print(f"{label} · {width}×{H} · {state_label}")
                print("=" * (width + 4))
                for line in render(state, width):
                    print("  " + line)
    print()
    print("图例（真终端里的颜色）：")
    print("  ✓ 已过 = MUTED    ◆ 当前 = ACCENT+BOLD    ○ 未到 = MUTED")
    print("  │ 连接 = MUTED     子块 ├ └ = MUTED       说明文字 = MUTED")
    print("  页签条的选中项（流程）= ACCENT+BOLD；小节线 = 内容档实线 ─")


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="右栏流程页的形状草稿（PROTOTYPE）")
    parser.add_argument("-v", "--variant", choices=sorted(VARIANTS), help="只看一个变体")
    parser.add_argument("-w", "--width", type=int, choices=[42, 60], help="只看一档宽度")
    args = parser.parse_args(argv)
    names = [args.variant] if args.variant else sorted(VARIANTS)
    widths = [args.width] if args.width else [42, 60]
    show(names, widths)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
