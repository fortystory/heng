# 终端鼠标移动事件的真实行为：谁会报、报多少

Type: research
Status: resolved
Part of: ../map.md
Blocked by: —

## 问题

heng 要做「**指针悬浮在会话名上时出现关闭按钮**」。这件事的地基是**终端会不会把鼠标移动
报上来**。已知：crossterm 0.29 的 `EnableMouseCapture` 实发 `?1000h ?1002h ?1003h ?1015h ?1006h`
（`crossterm-0.29.0/src/event.rs:322-335`），`?1003h` 是「任何事件都上报」；heng 无条件发它
（`src/render/tui.rs:776`），而 `TuiState::mouse` 的 match 只认 5 种手势，`MouseEventKind::Moved`
落进 `_ => {}` 被丢掉（`src/render/tui.rs:5481-5488`）。

要查清的是**终端那一侧的事实**，不是代码该怎么改：

1. **哪些终端与转发链真的会按 `?1003h` 上报移动**？逐个给结论（ kitty / Alacritty / Ghostty /
   wezterm / foot / GNOME Terminal / Konsole / Windows Terminal / iTerm2 / VS Code 终端 /
   tmux / screen / zellij / SSH 转发 / herdr 的 pane ）：支持、不支持、还是「有条件」。
2. **不支持的典型原因**是什么（不实现 1003 / 只在按住时上报 / 转发链过滤 / Wayland 与 X11 的差别）？
   有没有终端**默认不报但能开**（需要 heng 另发一串 ANSI）？
3. **事件密度**：一次跨越 80×24 的移动会产生多少个 `Moved`？终端与 crossterm 有没有节流
   （kitty 的 mouse reporting 有最小间隔吗）？一条 `Moved` 走一遍 heng 的事件循环要多久
   （`EventStream` → `tokio::select!` → `terminal_event` → `self.mouse(mouse)`，
   `src/render/tui.rs:500` / `:586` / `:605` / `:8201-8217`）？每来一个事件 `mouse()` 都置
   `dirty = true`（`:5474`）—— 持续移动会不会把重画队列打满？
4. **指针离开终端窗口 / 终端失焦**时有没有事件（`Moved` 到窗口外、或某个「离开」信号）？
   没有的话，最后一次坐标会黏在屏幕上 —— 这条直接决定「`✖` 会不会黏住」。
5. **crossterm 0.29 对 `Moved` 的映射细节**：`(3|4|5, true)` 映成 `Moved`（`crossterm-0.29.0/src/event/sys/unix/parse.rs:788`）——
   按住左键拖动时是 `Drag(Left)` 而不是 `Moved`？那「悬浮」与「拖选」在事件上怎么区分？
6. **别的项目怎么处理这件事**：有没有 TUI 实现了「hover 才出现的按钮」？它们靠 `?1003h`、靠别的
   终端能力，还是干脆不做？（这一条是**旁证**，不是结论。）

## 产出

一份 Markdown 笔记：[`../research/01-mouse-move-events.md`](../research/01-mouse-move-events.md)。
每条结论**带来源 URL**（终端文档、源码、issue 追踪），并分开写「实测到的」与「文档说的」。

末尾给三句可直接引用的结论：

- **支持面一句话**（哪些终端与转发链会报、哪些不会）。
- **密度一句话**（一次移动多少事件、对重画队列的压力有多大）。
- **离开窗口一句话**（`✖` 会不会黏住；要不要额外手段清它）。

## 范围

- **只读**。不碰 heng 的源码，不改本图与其它票。
- 本机可用终端探测（`printf` 一串 `?1003h` 看回包、用 `script` 造 pty 喂 SGR 序列），但**结论要能
  在别的机器上复现**：写清探测方法。
- **不答**形态问题（`✖` 长什么样 —— 归[形态票](02-prototype-close-button-shapes.md)）、
  **不答**关闭契约（关掉前台落到哪 —— 归[决策票](03-grilling-close-contract.md)）。
- 结论若推翻本图的冻结项 10（「不做降级」），**如实写**并说明代价，不要替维护者改主意。

## 作答

产物：[`../research/01-mouse-move-events.md`](../research/01-mouse-move-events.md)（240 行；外部结论逐条带来源 URL；
「文档说的」与「实测到的」分开写）。

**1. 支持面**。主流终端都实现 `?1003h`（任何事件上报，含无按键移动）：kitty、Alacritty、Ghostty、
wezterm、foot、GNOME Terminal（VTE）、Konsole、VS Code 集成终端（xterm.js）。**Windows Terminal 只
确认到「模式表里有 `ANY_EVENT_MOUSE_MODE = 1003`」这一层**（`DispatchTypes.hpp`），实际上报行为未
取证；iTerm2 只有应用级开关的文档（Profiles → Terminal），没有它对 1003 行为的一手说明。语义以
xterm 文档为准：<https://invisible-island.net/xterm/ctlseqs/ctlseqs.html>。

**转发链**：**tmux 通**（内层请求 1003 → tmux 对外层发 `?1000h ?1002h ?1003h` 并强制 `?1006h`，再把
motion 转成 SGR 转发给 pane；与 tmux 自己的 `mouse` 选项无关，该选项默认 off 且只管 tmux 自己的
pane 边界拖动）、**zellij 通**（pane 的 `AnyEventTracking` 下 motion 也写给 pane 内程序）、
**GNU screen 不通**（手册只列 `?9` / `?1000`，会吃掉移动事件）、**herdr 默认不通**（herdr 是
mouse-first 的复用器，自己的 `[ui] mouse_capture = false` 才放行；**文档没写**那时它是否把 1003
转发给 pane 内程序）、**SSH 本身透传**（纯字节转发，过滤发生在链路里的复用器上）。

**2. 密度**。kitty / Alacritty / Ghostty **按 cell 变化去重**（源码级证据），VTE 在 1003 下连同格也发。
一次横扫 80 列 ≈ **80** 个 `Moved`，斜穿 80×24 ≈ **103** 个，任意路径上界 80×24 = **1920**。
crossterm 0.29 **无节流**；heng 侧每个事件在 match 之前就 `self.dirty = true`
（`src/render/tui.rs:5474`），主循环每轮 `is_dirty()` 就 `paint_frame` 一次（`:669-680`）→
**事件洪峰 = 一事件一帧 diff + 一帧终端写**。

**3. 离开窗口**。**协议里没有「指针离开」事件**（xterm ctlseqs），最后坐标会黏住 —— `✖` 会留在
屏幕上不走。kitty 有私有扩展（cb = 128|32 = **160**，<https://sw.kovidgoyal.net/kitty/misc-protocol/#mouse_leave_window>），
但 crossterm 0.29 的 `parse_cb` 把 cb=160 拆成 `button_number=8` → 落 `_ => Err(...)` → **事件被当
解析错误丢弃**（`crossterm-0.29.0/src/event/sys/unix/parse.rs:776-795`）。失焦是另一条通道 `?1004`
（`CSI I` / `CSI O`），crossterm 有 `EnableFocusChange` / `Event::FocusLost`，但 **heng 现在没发
`?1004h`**。

**4. crossterm 映射（按源码复刻实测）**。cb=35 → `Moved`（无按键移动）；cb=32 → `Drag(Left)`（按住
左键拖动）；cb=0 + `M`/`m` → `Down(Left)` / `Up(Left)`；cb=64 → `ScrollUp`。**悬浮与拖选在事件层
分得开**（heng 现有 match 已分别接走 `Drag(Left)`，`Moved` 落 `_ => {}`）。crossterm 0.29 不解析
urxvt 1015（无影响，1006 优先生效）。

**5. 旁证**。Rust TUI `CodeWhale` 确实用 `MouseEventKind::Moved` 做 hover 层与 sidebar popover
（<https://github.com/Hmbown/CodeWhale/blob/46e3a646ed41cdf1ae74cecacb71c6d32ee7d0a0/crates/tui/src/tui/mouse_ui.rs>）。
`crossterm::EnableMouseCapture` 自带 `?1003h`，所以**多数 ratatui 程序其实已经开着 1003**，只是没人
处理 `Moved` —— 旁证「做悬浮不需要新开模式」。

**6. 实测边界（如实写）**。**实测到的**：pty 字节级（SGR 序列不被行规程改写，
`script -qec 'stty raw -echo; printf "\033[<35;10;5M" > /dev/tty; timeout 1 cat < /dev/tty | od -c'`
输出 `[<35;10;5M`）+ **按 crossterm 0.29 源码逐行复刻**的 `parse_cb` 判定（脚本
`.cache/research/crossterm_parse_model.py`）—— 这是「按源码复刻 + 真实字节」，不是直接跑 crossterm。
**没验成的**：**每个 GUI 终端是否真在 `?1003h` 下上报移动，一次都没验成** —— 本机 kitty / alacritty
起窗时 GL 失败（`MESA: failed to create dri2 screen` / `ZINK: vkEnumeratePhysicalDevices failed`），
XTest 注入指针的脚本在枚举窗口时 core dump。笔记 §1 末尾给了可复现的探测方法（DECRQM 查询
`CSI ? 1003 $ p` + XTest 注入 + 看 `ESC [ < 35 ; col ; row M`），在一台有 GL 的桌面上即可补上。

**7. 对 heng 的三条含义**（只给事实与代价）。① 悬浮目标怎么记：需要一份独立的「指针当前落在哪」
状态，每帧用命中表翻译成「哪一行被悬浮」；`Moved` 与 `Drag(Left)` 已分开，拖选与 hover 不会互相污染
（拖选时是否保留 hover 是待定语义）。② 黏住怎么办：必须自己清，候选信号三条、代价不同 ——
焦点事件（crossterm `EnableFocusChange`，需多发 `?1004h`）/ hover 超时（悬浮会闪）/ 任意键盘输入即清
（键盘操作后按钮消失）。③ 要不要节流：终端侧已按 cell 去重（80～1920 事件/次），crossterm 不节流，
heng 一事件一帧；要么在 heng 侧合并（同一帧只取最后一个 `Moved`、或只对 hover 状态变化重画），要么
接受「鼠标一动就一帧」—— 代价明确且可在真机上数帧验证。
