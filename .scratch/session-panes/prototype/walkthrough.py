#!/usr/bin/env python3
"""示例走查（不属于任何票）：把已经定下的决定串成一条使用路径，每步给出屏幕与按键。

跑法：`python3 .scratch/session-panes/prototype/walkthrough.py > .scratch/session-panes/prototype/walkthrough.txt`

这不是 prototype 票的产物（**与票 06 无关**，文件名故意不带序号），而是给维护者看的**示例**：
屏幕尺寸取 herdr 的实况（外层 120 列 → heng 面板 94 列），高度 30。几何照 `layout.rs` 的常量
（`MIN_WIDTH = 40`、`MIN_HEIGHT = 10`、pane 区 = 屏高 − 9）。走查里用到的每一条决定都标了它住在
哪张票里。
"""

from __future__ import annotations

import unicodedata

WIDTH, HEIGHT = 94, 30
STRIP_ROWS, FIXED_BELOW = 2, 7
MIN_W, MIN_H = 40, 10


def w(text: str) -> int:
    return sum(2 if unicodedata.east_asian_width(ch) in "WF" else 1 for ch in text)


def fit(text: str, width: int) -> str:
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


class Screen:
    def __init__(self) -> None:
        self.cells = [[" "] * WIDTH for _ in range(HEIGHT)]

    def put(self, x: int, y: int, text: str, limit: int) -> None:
        if y < 0 or y >= HEIGHT:
            return
        col, used = x, 0
        for ch in fit(text, limit):
            cw = 2 if unicodedata.east_asian_width(ch) in "WF" else 1
            if used + cw > limit or col + cw > WIDTH:
                break
            self.cells[y][col] = ch
            if cw == 2:
                self.cells[y][col + 1] = ""
            col += cw
            used += cw

    def hline(self, x: int, y: int, length: int, char: str = "─") -> None:
        for col in range(x, min(WIDTH, x + length)):
            self.cells[y][col] = char

    def vline(self, x: int, y: int, length: int, char: str = "│") -> None:
        for row in range(y, min(HEIGHT, y + length)):
            self.cells[row][x] = char

    def lines(self) -> list[str]:
        return ["".join(cell for cell in row) for row in self.cells]


def box(screen: Screen, x: int, y: int, w: int, h: int, title: str, body: list[str], focus: bool) -> None:
    tl, tr, bl, br, hz, vt = ("┏", "┓", "┗", "┛", "━", "┃") if focus else ("┌", "┐", "└", "┘", "─", "│")
    screen.put(x, y, tl, 1)
    screen.hline(x + 1, y, w - 2, hz)
    screen.put(x + w - 1, y, tr, 1)
    screen.vline(x, y + 1, h - 2, vt)
    screen.vline(x + w - 1, y + 1, h - 2, vt)
    screen.put(x, y + h - 1, bl, 1)
    screen.hline(x + 1, y + h - 1, w - 2, hz)
    screen.put(x + w - 1, y + h - 1, br, 1)
    screen.put(x + 2, y + 0, f" {title} ", w - 4)
    for index, text in enumerate(body):
        if 1 + index >= h - 1:
            break
        screen.put(x + 2, y + 1 + index, text, w - 4)


def scene(
    *,
    tabs: str,
    panes: list[tuple[str, list[str]]],
    focus: int | None = None,
    extra: str = "",
    right_rail: bool = False,
    status: str = "kimi · high │ ask │ 上下文 12%",
    input_text: str = "",
    hints: str = "ctrl-r 平铺 · ctrl-v 右栏 · ctrl-o 左栏 · ctrl-1..4 焦点",
) -> list[str]:
    screen = Screen()
    screen.put(0, 0, tabs, WIDTH)
    screen.hline(0, 1, WIDTH, "┄")
    rail = 28 if right_rail else 0
    inner_w = WIDTH - (rail + 1 if rail else 0)
    rows = HEIGHT - STRIP_ROWS - FIXED_BELOW
    count = len(panes)
    if count == 1:
        plan = [(0, 0, inner_w, rows)]
    elif count == 2:
        half = (inner_w - 1) // 2
        plan = [(0, 0, half, rows), (half + 1, 0, inner_w - half - 1, rows)]
    elif count == 3:
        half = (inner_w - 1) // 2
        half_h = (rows - 1) // 2
        plan = [
            (0, 0, half, rows),
            (half + 1, 0, inner_w - half - 1, half_h),
            (half + 1, half_h + 1, inner_w - half - 1, rows - half_h - 1),
        ]
    else:
        half = (inner_w - 1) // 2
        half_h = (rows - 1) // 2
        plan = [
            (0, 0, half, half_h),
            (half + 1, 0, inner_w - half - 1, half_h),
            (0, half_h + 1, half, rows - half_h - 1),
            (half + 1, half_h + 1, inner_w - half - 1, rows - half_h - 1),
        ]
    for index, ((title, body), (x, y, pw, ph)) in enumerate(zip(panes, plan)):
        box(screen, x, STRIP_ROWS + y, pw, ph, title, body, focus == index)
    if rail:
        screen.vline(inner_w, STRIP_ROWS, HEIGHT - STRIP_ROWS, "│")
        screen.put(inner_w + 1, STRIP_ROWS, "右栏 · 焦点会话的轨迹", rail)
        for row, text in enumerate(
            ["┄┄ 回合 1 · 49 s ┄┄┄┄", "09:12:11 [kimi] 调用", "          read_file…", "09:12:27 [kimi] 调用", "          bash…"], start=1
        ):
            screen.put(inner_w + 1, STRIP_ROWS + row, text, rail)
    screen.put(0, HEIGHT - FIXED_BELOW + 1, f"{status}   另有 2 个在跑 · 1 个等人类", WIDTH)
    if input_text:
        screen.put(0, HEIGHT - FIXED_BELOW + 3, input_text, WIDTH)
    else:
        screen.put(0, HEIGHT - FIXED_BELOW + 3, "（输入区：全局一份，绑焦点 pane）", WIDTH)
    screen.put(0, HEIGHT - FIXED_BELOW + 4, hints, WIDTH)
    if extra:
        screen.put(0, HEIGHT - FIXED_BELOW + 6, extra, WIDTH)
    return screen.lines()


CONVO_A = ["[用户] 帮我把 grep 的输出截断修一下", "[kimi] 看过 src/grep.rs:412 了", "[kimi] ▸ 调用 edit_file", "  src/grep.rs  in=21620"]
CONVO_B = ["[用户] 超时改成 120 秒", "[kimi] … 正在思考", "", ""]
CONVO_C = ["[用户] 写下 README 的安装一节", "[kimi] ▸ 调用 read_file", "  README.md  in=18234"]


def walkthrough() -> list[tuple[str, str, list[str]]]:
    steps: list[tuple[str, str, list[str]]] = []

    steps.append(
        (
            "第 1 步 · 在 herdr 里启动 heng",
            "冻结项 9：`HERDR_ENV=1` 时**左栏默认关**（herdr 自己的侧栏已经占了 26 列）；"
            "冻结项 6：**右栏默认开**，第一个页面是轨迹。",
            scene(
                tabs="[◆会话 A · 调研 ●]                    ▤ 平铺  ▥ 右栏",
                panes=[("会话 A · 调研  ● 忙碌", CONVO_A)],
                focus=0,
                right_rail=True,
                input_text="帮我把 grep 的输出截断修一下，超限时只留头尾",
            ),
        )
    )

    steps.append(
        (
            "第 2 步 · 另外两个会话（标签态）",
            "左栏会话页管状态（票 09 的分工），顶部页签条管显示。点一个标签 = 切焦点（票 07）。",
            scene(
                tabs="[◆会话 A ●]  [会话 B]  [会话 C]        ▤ 平铺  ▥ 右栏",
                panes=[("会话 A · 调研  ● 忙碌", CONVO_A)],
                focus=0,
                right_rail=True,
                input_text="再帮我看下 bash 工具的超时",
            ),
        )
    )

    steps.append(
        (
            "第 3 步 · 按 `Ctrl-R` 进平铺（集合还是空的）",
            "票 07：**集合为空时，自动把焦点会话加进来** —— 否则第一次永远进不去。"
            "`[+ 加入]` 格**参与布局**（集合 n 个 → n + 1 格），所以现在是 2 格。"
            "平铺态**让两侧栏让位**（票 05 的结论：94 列三块全开主列只剩 36 列）。",
            scene(
                tabs="[◆会话 A ●]  [会话 B]  [会话 C]        ▤ 平铺  ▥ 右栏",
                panes=[
                    ("会话 A · 调研  ● 忙碌", CONVO_A),
                    ("[+ 加入]", ["点这里把另一个会话", "摆进这一屏"]),
                ],
                focus=0,
            ),
        )
    )

    steps.append(
        (
            "第 4 步 · 点 `[+ 加入]`：会话选择器",
            "复用现成的 picker 通道（`Ctrl-T` 那个模型选择器同一套件）。它是个浮层，"
            "照旧**整屏居中**（票 06：浮层不缩进 pane）。",
            scene(
                tabs="[◆会话 A ●]  [会话 B]  [会话 C]        ▤ 平铺  ▥ 右栏",
                panes=[
                    ("会话 A · 调研  ● 忙碌", CONVO_A),
                    ("[+ 加入]", []),
                ],
                focus=0,
                extra="　◆ 选一个会话加入平铺　│　会话 B · 编译 ? 等人类　│　会话 C · 文档 ○ 空闲　（Esc 取消）",
            ),
        )
    )

    steps.append(
        (
            "第 5 步 · 选中会话 B：2 格（左右）",
            "排法照冻结项 4。焦点仍在 A（双线框那格），`Ctrl-2` 可以跳到 B。",
            scene(
                tabs="[◆会话 A ●]  [◆会话 B ?]  [会话 C]        ▤ 平铺  ▥ 右栏",
                panes=[("会话 A · 调研  ● 忙碌", CONVO_A), ("会话 B · 编译  ? 等人类", CONVO_B)],
                focus=0,
            ),
        )
    )

    steps.append(
        (
            "第 6 步 · 再加一个：3 格（L 形）",
            "94×30 下每格 46×10 或 46×21 —— 刚好压到 `MIN_HEIGHT = 10` 的线上（票 05 的判据）。"
            "4 格要屏高 ≥ 30 行，这里正好。",
            scene(
                tabs="[◆会话 A ●]  [◆会话 B ?]  [◆会话 C ○]   ▤ 平铺  ▥ 右栏",
                panes=[
                    ("会话 A · 调研  ● 忙碌", CONVO_A),
                    ("会话 B · 编译  ? 等人类", CONVO_B),
                    ("会话 C · 文档  ○ 空闲", CONVO_C),
                ],
                focus=0,
            ),
        )
    )

    steps.append(
        (
            "第 7 步 · 按 `Ctrl-2`：焦点跳到第 2 格",
            "票 07：`Ctrl-1`…`Ctrl-4` 直接跳第 N 格（注意 `map_key` 今天忽略修饰键，要先加判断）。"
            "焦点格画双线框 —— 全屏唯一的反显已经给了问卷与菜单，焦点只能靠结构（票 05 的帧）。",
            scene(
                tabs="[会话 A ●]  [◆会话 B ?]  [会话 C ○]     ▤ 平铺  ▥ 右栏",
                panes=[
                    ("会话 A · 调研  ● 忙碌", CONVO_A),
                    ("会话 B · 编译  ? 等人类", CONVO_B),
                    ("会话 C · 文档  ○ 空闲", CONVO_C),
                ],
                focus=1,
                status="kimi · high │ ask │ 上下文 41%",
                input_text="超时改成 120 秒",
            ),
        )
    )

    steps.append(
        (
            "第 8 步 · 会话 B 弹出权限门：全屏接管",
            "票 06：问卷与权限门**全屏接管、焦点自动跳过去**；接管期间其余 pane **照常更新**"
            "（它们还在跑），键盘归弹出来的这一层。94 列的面板上，题面按整屏排。",
            scene(
                tabs="[会话 A ●]  [◆会话 B ?]  [会话 C ○]     ▤ 平铺  ▥ 右栏",
                panes=[
                    ("会话 A · 调研  ● 忙碌", CONVO_A),
                    ("会话 B · 编译  ? 等人类", CONVO_B),
                    ("会话 C · 文档  ○ 空闲", CONVO_C),
                ],
                focus=1,
                extra="　┏━ 会话 B 想调用 bash（write）━━━━━━━━━━━━┓",
            ),
        )
    )

    steps.append(
        (
            "第 9 步 · 答完 + 按 `Ctrl-C` 取消",
            "票 06：`Ctrl-C` 只取消**焦点 pane**（这里就是 B）的当前动作，沿用 session-tab 票 23 "
            "拆掉「第二下变退出」之后的语义；退出举手仍是进程级一个。",
            scene(
                tabs="[会话 A ●]  [◆会话 B ●]  [会话 C ○]     ▤ 平铺  ▥ 右栏",
                panes=[
                    ("会话 A · 调研  ● 忙碌", CONVO_A),
                    ("会话 B · 编译  ● 运行中", ["[用户] 超时改成 120 秒", "[kimi] ▸ 调用 bash  cargo test", "（已取消）"]),
                    ("会话 C · 文档  ○ 空闲", CONVO_C),
                ],
                focus=1,
                status="kimi · high │ ask │ 上下文 41%",
            ),
        )
    )

    steps.append(
        (
            "第 10 步 · 关掉会话 C：从集合移出",
            "票 07：关闭 → **从集合移出、焦点给集合里的下一个**。焦点仍在 B，C 那格消失，"
            "L 形退回 2 格。",
            scene(
                tabs="[会话 A ●]  [◆会话 B ●]                ▤ 平铺  ▥ 右栏",
                panes=[("会话 A · 调研  ● 忙碌", CONVO_A), ("会话 B · 编译  ● 运行中", CONVO_B)],
                focus=1,
                input_text="超时改成 120 秒",
            ),
        )
    )

    steps.append(
        (
            "第 11 步 · 再关一个：只剩一个 → 自动回标签态",
            "票 07 定的这条来自票 05 的帧：1 个 pane 的平铺态与标签态几乎一样，所以不必停在那里。"
            "回到标签态，**右栏回来**（默认开），显示焦点会话的轨迹 —— 而平铺态的轨迹入口正是"
            "票 08 要拍的那条（每格没有页签条）。",
            scene(
                tabs="[会话 A ●]  [◆会话 B ●]                ▤ 平铺  ▥ 右栏",
                panes=[("会话 B · 编译  ● 运行中", CONVO_B)],
                focus=0,
                right_rail=True,
                input_text="超时改成 120 秒",
                hints="ctrl-r 平铺 · ctrl-v 右栏 · ctrl-o 左栏 · ctrl-1..4 焦点",
            ),
        )
    )

    return steps


def main() -> None:
    problems: list[str] = []
    print("=" * 78)
    print("示例走查：在 herdr 里用衡跑三个会话（94×30，即外层 120 列终端）")
    print("这不是任何一张票的产物 —— 它是把已经定下的决定串起来看一眼形状。")
    print("=" * 78)
    for title, why, lines in walkthrough():
        print()
        print(f"── {title} {'─' * max(0, 60 - w(title))}")
        for text in why.splitlines():
            print(f"   {text}")
        print()
        for line in lines:
            if w(line) > WIDTH:
                problems.append(f"{title}：{w(line)} > {WIDTH}")
            print(f"   |{line}")
        print()
    print("=" * 78)
    print("自检：没有溢出" if not problems else f"自检：{len(problems)} 行溢出 {problems}")
    print("=" * 78)


if __name__ == "__main__":
    main()
