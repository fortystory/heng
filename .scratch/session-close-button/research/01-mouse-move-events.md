# 终端鼠标移动事件的真实行为：谁会报、报多少

配套票：[`../issues/01-research-mouse-move-events.md`](../issues/01-research-mouse-move-events.md) ·
图：[`../map.md`](../map.md)。只读调研，未改任何源码。

## 三句结论

- **支持面**：主流终端（kitty、Alacritty、Ghostty、wezterm、foot、GNOME Terminal/Konsole、VS Code 集成终端）**都实现 `?1003h`（任何事件上报，含无按键移动）**（Windows Terminal 确认到「模式表里有 1003」这一层，实际上报行为未取证；iTerm2 只有应用级开关的文档）；转发链里 **tmux 与 zellij 会把 1003 请求传给 pane 内程序**（tmux 还要求外层终端自己支持 1003），**GNU screen 最高只到 `?1000`**（会吃掉移动事件），**herdr 默认自己捕获鼠标**（`mouse_capture = false` 可关）。
- **密度**：主流终端按 **cell 变化**去重，一次横扫 80 列约 **80** 个 `Moved`、斜穿 80×24 约 **103** 个、理论上限 **1920** 个/次；crossterm **无节流**，heng 每来一个事件置一次 `dirty = true`，事件洪峰会变成「一事件一帧 diff + 一帧终端写」。
- **离开窗口**：**没有「指针离开」事件**（kitty 有私有扩展，但 crossterm 0.29 把它判为解析错误直接丢弃），所以**最后一次坐标会黏住**——`✖` 会留在屏幕上不走，除非另有机制清掉。

## 0. 证据等级与本机实测状态

本笔记分四档标注，不混：

| 标记 | 含义 |
|---|---|
| **源码** | 读了上游仓库的对应文件，给出文件与行号、给出 URL |
| **文档** | 官方文档/手册的明文，给出 URL |
| **实测** | 在本机跑过，给出可复现命令 |
| **未取证** | 这次没拿到一手依据，明确写出来（不拿二手解读冒充结论） |

**本机实测做到了哪一步**（本机：Arch Linux，X11 `DISPLAY=:0` + `WAYLAND_DISPLAY=wayland-1`，装了 kitty 0.49.2、alacritty 0.17.0、ghostty 1.3.1、wezterm 20260716、foot 1.28.0、konsole 26.08.2、tmux 3.8）：

- ✅ **实测（字节级）**：pty 里 SGR 鼠标序列原样通过，不被行规程改写。命令：
  ```bash
  script -qec 'stty raw -echo; printf "\033[<35;10;5M" > /dev/tty; timeout 1 cat < /dev/tty | od -c | head -3' /dev/null
  # => [<35;10;5M
  ```
- ✅ **实测（按源码复刻 crossterm 的判定）**：把 crossterm 0.29 的 `parse_cb` 逐行翻成 Python 再喂真实字节，见 §5；脚本在 `.cache/research/crossterm_parse_model.py`。
- ❌ **实测（GUI 终端是否真的在 `?1003h` 下上报移动）没做成**：本机起 GUI 终端失败——kitty/alacritty 报 `MESA: failed to create dri2 screen` / `ZINK: vkEnumeratePhysicalDevices failed`，XWayland 下没有可用 GL；用 XTest 注入指针的脚本（`.cache/research/probe_mouse.py`）在枚举窗口时 core dump。
  **因此 §1 里每一个终端的「会上报」都是源码/文档依据，不是本机实测。** 想补实测，用 §1 末尾给的探测脚本（DECRQM 查询 + XTest 注入），在一台有 GL 的桌面上跑即可。

---

## 1. 哪些终端与转发链真的会按 `?1003h` 上报移动

规范原文（xterm 是这一族协议的源头）：

> Any-event mode is the same as button-event mode, except that all motion events are reported, even if no mouse button is down. It is enabled by specifying 1003 to DECSET.
> —— <https://invisible-island.net/xterm/ctlseqs/ctlseqs.html>

逐个结论：

| 终端 / 链路 | 结论 | 依据 |
|---|---|---|
| **kitty** | **支持**。`MOUSE_MOVE_TRACKING (1003 << 5)`；`mouse_tracking_mode == ANY_MODE` 时无按键移动也上报 | **源码** [kitty/modes.h](https://github.com/kovidgoyal/kitty/blob/master/kitty/modes.h)、[kitty/mouse.c](https://github.com/kovidgoyal/kitty/blob/master/kitty/mouse.c)（`in_tracking_mode = (mode == ANY_MODE \|\| (mode == MOTION_MODE && button >= 0))`） |
| **Alacritty** | **支持**。`TermMode::MOUSE_MOTION`；1003 下移动到**新 cell** 就发 `mouse_report(35, …)`（35 = motion + 无按键） | **源码** [alacritty_terminal/src/term/mod.rs](https://github.com/alacritty/alacritty/blob/master/alacritty_terminal/src/term/mod.rs)、[alacritty/src/input/mod.rs](https://github.com/alacritty/alacritty/blob/master/alacritty/src/input/mod.rs) |
| **Ghostty** | **支持**。`GHOSTTY_MODE_ANY_MOUSE (1003)`；`eventSendsMotion(.any)` 为真 | **源码** [include/ghostty/vt/modes.h](https://github.com/ghostty-org/ghostty/blob/main/include/ghostty/vt/modes.h)、[src/terminal/mouse.zig](https://github.com/ghostty-org/ghostty/blob/main/src/terminal/mouse.zig)、[src/input/mouse_encode.zig](https://github.com/ghostty-org/ghostty/blob/main/src/input/mouse_encode.zig) |
| **wezterm** | **支持**。`MouseTracking = 1000` / `ButtonEventMouse = 1002` / `AnyEventMouse = 1003` / `SGRMouse = 1006` 都在解析表里 | **源码** [wezterm-escape-parser/src/csi.rs](https://github.com/wezterm/wezterm/blob/main/wezterm-escape-parser/src/csi.rs)；**文档**「applications can emit escape sequences to request mouse event tracking … passed through to the application」<https://wezterm.org/config/mouse.html> |
| **foot** | **支持**。`case 1003: term->mouse_tracking = MOUSE_MOTION;`，枚举注释 `MOUSE_MOTION, /* ?1003h - report clicks and motion */` | **源码** [src/term/csi.c](https://codeberg.org/foot/foot/src/branch/master/term/csi.c)、[src/term/terminal.h](https://codeberg.org/foot/foot/src/branch/master/term/terminal.h)；**文档** man 页「disabled whenever the client has enabled mouse tracking」<https://man.archlinux.org/man/foot.1.en> |
| **GNOME Terminal**（VTE） | **支持**。`MouseTrackingMode::eALL_MOTION_TRACKING`；1003 下**连同格移动也发** | **源码** [vte/src/vte.cc](https://gitlab.gnome.org/GNOME/vte/-/blob/master/src/vte.cc)（`maybe_send_mouse_drag`） |
| **Konsole** | **支持**。`case token_csi_pr('h', 1003): setMode(MODE_Mouse1003)`；motion 事件 `cb += 0x20` | **源码** [konsole/src/Vt102Emulation.cpp](https://invent.kde.org/konsole/konsole/-/blob/master/src/Vt102Emulation.cpp) |
| **Windows Terminal** | **支持（模式层面）**。`ANY_EVENT_MOUSE_MODE = DECPrivateMode(1003)` 在私有模式表里 | **源码** [microsoft/terminal src/terminal/adapter/DispatchTypes.hpp](https://github.com/microsoft/terminal/blob/main/src/terminal/adapter/DispatchTypes.hpp) |
| **iTerm2** | **有条件（应用级开关）**。Profiles → Terminal 有「Enable mouse reporting」「Report mouse clicks & drags」；开了之后应用可拿到鼠标信息 | **文档** <https://iterm2.com/documentation-preferences-profiles-terminal.html>；**未取证**：没找到 iTerm2 对 1003 具体行为的官方明文或源码依据 |
| **VS Code 集成终端**（xterm.js） | **支持**。`case 1003: // any event mouse` → `mouseProtocol = 'ANY'` | **源码** [xterm.js src/common/InputHandler.ts](https://github.com/xtermjs/xterm.js/blob/master/src/common/InputHandler.ts)；**旁证** VS Code issue 记录了 1003+1006 下鼠标移动把强制选择清掉的现象 <https://github.com/microsoft/vscode/issues/194554> |
| **tmux** | **支持**。内层程序请求 1003 → tmux 记 `screen->mode |= MODE_MOUSE_ALL` → 对**外层终端**发 `?1000h ?1002h ?1003h`（并强制 `?1006h`）→ 收到 motion 后转成 SGR 转发给 pane 内程序。**与 `mouse` 选项无关**（该选项只管 tmux 自己要不要识别鼠标；默认 off，但选项说明写明「Applications inside panes can use the mouse even when 'off'」） | **源码** [tmux/tty.c `tty_update_mode`](https://github.com/tmux/tmux/blob/master/tty.c)、[tmux/input-keys.c `input_key_get_mouse`](https://github.com/tmux/tmux/blob/master/input-keys.c)、[tmux/input.c](https://github.com/tmux/tmux/blob/master/input.c)（`case 1003: screen_write_mode_set(sctx, MODE_MOUSE_ALL)`）；**文档** <https://man7.org/linux/man-pages/man1/tmux.1.html>（`mouse [ on \| off ]`；默认值见 `tmux.h` 的 `#define TMUX_MOUSE 0`） |
| **zellij** | **支持**。pane 的 `mouse_tracking` 支持 `AnyEventTracking`（1003），`mouse_event_signal` 在 AnyEventTracking 下把**所有**事件（含 Motion）编码成 SGR 写给 pane 内程序 | **源码** [zellij-server/src/panes/grid.rs](https://github.com/zellij-org/zellij/blob/main/zellij-server/src/panes/grid.rs)、[zellij-server/src/panes/terminal_pane.rs](https://github.com/zellij-org/zellij/blob/main/zellij-server/src/panes/terminal_pane.rs)；客户端向**外层终端**请求 `?1000h ?1002h ?1003h ?1015h ?1006h`（[os_input_output.rs](https://github.com/zellij-org/zellij/blob/main/zellij-client/src/os_input_output.rs)） |
| **GNU screen** | **不支持**。screen 的手册只列 `?9`（X10）与 `?1000`（VT200 mouse tracking），**没有 `?1002`/`?1003`** → 应用在 screen 里请求 1003 不会换来移动事件 | **文档** [screen(1) 手册的 escape sequence 一节](https://manpages.debian.org/unstable/screen/screen.1.en.html) |
| **SSH 转发链** | **透传**。SSH 只搬字节，不解释也不改写控制序列 → 终端 → ssh → 远端 pty 的鼠标事件原样过去 | **文档**（ssh 是纯字节转发，协议本身不涉及控制序列）<https://www.openssh.com/manual.html>；**推论**：真正的过滤发生在链路里的 multiplexer（screen / 某些 ssh 代理）上，不在 ssh |
| **herdr 的 pane** | **默认不通**。herdr 是 mouse-first 的复用器（点 pane、拖边框、右键菜单），配置项 `[ui] mouse_capture = false` 专门用来「让 herdr 停止捕获鼠标」→ 默认它自己吃鼠标事件 | **文档** <https://herdr.dev/docs/concepts/>（Mouse UI 一节）；**未取证**：herdr 是否在 `mouse_capture = false` 时把 1003 motion 转发给 pane 内程序，文档没写 |

**补测方法（想在本机/别的机器上确认某个终端）**：

```bash
# 1) 该终端是否「认识」1003（DECRQM 查询，终端会回 CSI ? 1003 ; <state> $ y）
#    放进真终端里跑，把回复收进文件：
script -qec 'stty raw -echo; printf "\033[?1003$p" > /dev/tty; timeout 1 cat < /dev/tty' /dev/null | od -c
# 2) 该终端在 1003 下是否真的上报无按键移动：
#    终端里跑 sh -c 'printf "\033[?1003h\033[?1006h" > /dev/tty; timeout 5 cat < /dev/tty > /tmp/mouse.bin'
#    然后用 XTest 把指针移进窗口（需要 X11 + 有 GL 的桌面）：
#      python3 - <<'PY'
#      import ctypes,ctypes.util
#      x=ctypes.CDLL(ctypes.util.find_library("X11")); t=ctypes.CDLL(ctypes.util.find_library("Xtst"))
#      x.XOpenDisplay.restype=ctypes.c_void_p; d=x.XOpenDisplay(None)
#      for i in range(40): t.XTestFakeMotionEvent(d,-1,300+i*8,300+i*5,0); x.XFlush(d)
#      PY
#    之后看 /tmp/mouse.bin 里有没有 ESC [ < 35 ; col ; row M（35 = 无按键移动）
```

---

## 2. 不支持的典型原因

1. **只实现到 1000/1002，不实现 1003**。典型代表是 **GNU screen**：手册只列 `?9`/`?1000`；它作为终端模拟器把未识别的 `?1003h` 忽略掉，**应用因此永远收不到移动事件**。这一类里，程序「另发一串 ANSI」救不回来——1003 就是那串 ANSI，终端不认识就没有。
2. **转发链自己吃事件**。**herdr** 默认捕获鼠标（`mouse_capture`），鼠标事件用于它自己的 pane UI。**tmux / zellij** 是「自己也用，但会转发」的一类：tmux 用鼠标做 pane 边界拖动（受 `mouse` 选项控制，默认 off），同时把请求 1003 的 pane 的 motion 事件转下去（`tty_update_mode` + `input_key_get_mouse`）。差别就在这里：**tmux/zellij 通，herdr（默认）不通，screen 也不通。**
3. **只在按住时上报（= 只做了 1002 的行为）**。协议层面的 1002（button-event）只报「有按键时的移动」。**没实现 1003 的终端**即使把 1002 也支持了，无按键移动照样不来。VTE 的实现里能看到这条分界：`< eALL_MOTION_TRACKING` 时若 `m_mouse_pressed_buttons == 0` 直接 return（[vte.cc](https://gitlab.gnome.org/GNOME/vte/-/blob/master/src/vte.cc)）。
4. **Wayland 与 X11 的差别**。差别**不在协议层**：1003 是终端与程序之间的字节协议，Wayland 终端照样实现。差别在**输入事件的产生频率与注入方式**：Wayland 下 motion 由合成器派发（wl_pointer motion），X11 下由 X server 的 pointer sampling 派发；两者都按 cell 去重的终端（kitty/alacritty/ghostty）输出密度一致。真正的影响是：**在 Wayland 上没法用 XTest 注入指针**（这正是本机实测失败的原因之一），以及「鼠标离开窗口」在 Wayland 下由合成器决定何时停止派发。
5. **默认不报但能开**。终端层面没有「默认关、可以开 1003」的开关——1003 是应用**按需请求**的（终端默认关鼠标上报，程序发 `?1003h` 才开）。真正的「默认关」在**复用器与 iTerm2 那一层**：
   - iTerm2：Profiles → Terminal 的「Enable mouse reporting」/「Report mouse clicks & drags」是应用级开关（<https://iterm2.com/documentation-preferences-profiles-terminal.html>）。
   - tmux：`mouse` 选项（**默认 off**）只决定 tmux 自己要不要识别鼠标/执行 mouse 绑定；**不影响** pane 内程序使用鼠标（选项说明：「Applications inside panes can use the mouse even when 'off'」）。
   - herdr：`[ui] mouse_capture = false`。
   - heng 自己：**没有任何开关**——`EnableMouseCapture` 是无条件发的（`src/render/tui.rs:776`）。

---

## 3. 事件密度：一次移动多少个 `Moved`，有没有节流

**终端侧：主流终端按 cell 变化去重**（这本身就是最有效的一层节流）：

- **Alacritty**：只在 `cell_changed` 且模式含 `MOUSE_MOTION` 时才 `mouse_report(35, …)`（[input/mod.rs](https://github.com/alacritty/alacritty/blob/master/alacritty/src/input/mod.rs)）。
- **Ghostty**：`// We only send motion events when the cell changed unless we're tracking raw pixels`（[mouse_encode.zig](https://github.com/ghostty-org/ghostty/blob/main/src/input/mouse_encode.zig)）。
- **kitty**：`if (mouse_cell_changed || … == SGR_PIXEL_PROTOCOL)` 才发（[mouse.c](https://github.com/kovidgoyal/kitty/blob/master/kitty/mouse.c)）。
- **VTE**：1003 下**不过滤同格**（比上面几家更密），频率由 GTK 的 motion 事件决定（[vte.cc](https://gitlab.gnome.org/GNOME/vte/-/blob/master/src/vte.cc)）。

**数量估算**（cell 去重、80×24 屏）：

| 动作 | 事件数 |
|---|---|
| 横扫一整行 80 列 | ≈ 80 |
| 斜穿整屏（80 列 + 24 行） | ≈ 80 + 24 − 1 = 103 |
| 任意路径扫过全屏 | ≤ 80 × 24 = **1920**（上界） |
| 原地抖动（同格内移动） | 0（cell 去重）或 1（VTE 这类不过滤同格的） |

**crossterm 侧：无节流**。crossterm 0.29 把每个 SGR/X10 鼠标序列直接翻译成一个 `Event::Mouse`，没有合并、没有时间窗（`crossterm-0.29.0/src/event/sys/unix/parse.rs`）。

**heng 侧的代价**（事实，不含建议）：

- 事件流：`EventStream` → `tokio::select!`（`src/render/tui.rs:500`）→ `terminal_event`（`:586`、`:605`、`:8201-8217`）→ `self.mouse(mouse)`（`:5474` 附近）。
- `mouse()` **无条件** `self.dirty = true`（`src/render/tui.rs:5474`）——注意它在 `match mouse.kind` **之前**，所以即便 `Moved` 落进 `_ => {}` 被丢掉，dirty 也已经置上了。
- 主循环每轮末尾 `if state.is_dirty()` 就 `paint_frame` 一次（`src/render/tui.rs:669-680`），外面还包了一层 `BeginSynchronizedUpdate` / `EndSynchronizedUpdate`。
- 后果：**一个事件 = 一轮 select + 一次 ratatui diff + 一次终端写**。一次横扫 80 列 ≈ 80 帧；连续甩鼠标时事件来得比处理快，会把这条循环变成「事件 → 帧」的 1:1 流水线。ratatui 的 `draw` 只写变化的 cell，所以单帧的**字节量**不大；**帧数**才是压力所在。

**补充事实**：kitty/ghostty 的 **SGR-Pixels（1016）** 模式按像素而非 cell 上报（[kitty/mouse.c](https://github.com/kovidgoyal/kitty/blob/master/kitty/mouse.c)、[ghostty mouse_encode.zig](https://github.com/ghostty-org/ghostty/blob/main/src/input/mouse_encode.zig)）。heng 用的是 1006（cell 坐标），不涉及这一档；但若将来发 `?1016h`，密度会升一个量级。

---

## 4. 指针离开终端窗口 / 终端失焦

- **没有「离开」事件**（除 kitty 的私有扩展外）。协议里没有这种报告：1003 只在**指针在窗口内移动**时产生事件；指针移出窗口后，终端不再有 motion 可报，于是**程序收到的最后一个坐标停在原地**。xterm 文档对 1003 的定义也只有 motion 报告，没有 leave（<https://invisible-island.net/xterm/ctlseqs/ctlseqs.html>）。
- **kitty 有私有扩展**：「Reporting when the mouse leaves the window」——kitty 扩展 SGR 协议，鼠标离开时把第一个参数的 **bit 8 置位**（并置 bit 5 表示 motion），坐标必须忽略。文档原文：「kitty extends the SGR Pixel mouse reporting protocol created by xterm to also report when the mouse leaves the window … When bit 8 is set it means the event is a mouse has left the window event」。来源：<https://sw.kovidgoyal.net/kitty/misc-protocol/#mouse_leave_window>，实现见 [kitty/mouse.c](https://github.com/kovidgoyal/kitty/blob/master/kitty/mouse.c)（`LEAVE_INDICATOR (1 << 8)`、`cb = LEAVE_INDICATOR | MOTION_INDICATOR` = **160**）。
- **但 crossterm 0.29 拿不到它**：`parse_cb` 把 cb 拆成 `button_number = (cb & 0b11) | ((cb & 0b1100_0000) >> 4)`，cb=160 → `button_number = 8` → 落到 `_ => Err(could_not_parse_event_error())`，**整个事件被当成解析错误丢掉**（`crossterm-0.29.0/src/event/sys/unix/parse.rs:776-795`；文件头注释「Err(_) -> failed to parse event, clear the buffer」）。**结论：kitty 上「离开」信号存在，但 heng 用 crossterm 收不到。**
- **失焦**是另一条通道：`?1004` 的 FocusIn/FocusOut（`CSI I` / `CSI O`），与鼠标协议正交、可以同时开。xterm 文档：「FocusIn/FocusOut can be combined with any of the mouse events … causes xterm to send CSI I when the terminal gains focus, and CSI O when it loses focus」（<https://invisible-island.net/xterm/ctlseqs/ctlseqs.html>）。crossterm 0.29 有 `EnableFocusChange`（发 `?1004h`，`crossterm-0.29.0/src/event.rs:387`）与 `Event::FocusGained` / `Event::FocusLost`；**heng 目前没发 `?1004h`**（`src/render/tui.rs:776` 只发了 `EnableMouseCapture` + `EnableBracketedPaste` + Kitty 键盘协议）。
- **于是**：`✖` 会在指针移出窗口或终端失焦后**留在屏幕上**。这不是实现 bug，是协议里没有这条信息。可用的替代信号有三条（各自代价不同，属决策）：焦点事件（`?1004`，失焦即清）、超时（hover 状态带寿命）、以及「任何键盘输入即清」。

---

## 5. crossterm 0.29 对 `Moved` 的映射细节

crossterm 0.29 的判定逻辑（`crossterm-0.29.0/src/event/sys/unix/parse.rs:776-795`）：

```rust
let button_number = (cb & 0b0000_0011) | ((cb & 0b1100_0000) >> 4);
let dragging = cb & 0b0010_0000 == 0b0010_0000;
match (button_number, dragging) {
    (0, false) => Down(Left),   (0, true)  => Drag(Left),
    (3, false) => Up(Left),     (3, true)  => Moved,
    (4, false) => ScrollUp,     (4, true)  => Moved,
    (5, false) => ScrollDown,   (5, true)  => Moved,
    …
    _ => return Err(could_not_parse_event_error()),
}
```

**「悬浮」与「拖选」在事件上是分得开的**：

| 终端发来的 cb | 含义 | crossterm 事件 |
|---|---|---|
| 35（32 motion + 3 无按键） | 无按键移动（1003 的「悬浮」） | `MouseEventKind::Moved` |
| 32（32 motion + 0 左键） | 按住左键拖动 | `MouseEventKind::Drag(MouseButton::Left)` |
| 0 + `M` / 0 + `m` | 左键按下 / 抬起 | `Down(Left)` / `Up(Left)` |
| 64 | 滚轮上 | `ScrollUp` |

**实测（按源码复刻判定，非直接跑 crossterm）**：

```bash
$ python3 .cache/research/crossterm_parse_model.py
SGR 无按键移动（1003）                       cb=  35 button_number=3 dragging=True -> Moved
SGR 按住左键拖动（1002/1003）                 cb=  32 button_number=0 dragging=True -> Drag(Left)
SGR 左键按下                                cb=   0 button_number=0 dragging=False -> Down(Left)
kitty 鼠标离开窗口（cb = 128|32 = 160）        cb= 160 button_number=8 dragging=True -> None   # 解析错误，被丢弃
kitty 鼠标离开窗口（cb = 128）                 cb= 128 button_number=8 dragging=False -> None   # 解析错误，被丢弃
SGR 滚轮上（cb=64）                           cb=  64 button_number=4 dragging=False -> ScrollUp
```

所以：

- **按住左键拖动报的是 `Drag(Left)`，不是 `Moved`**——heng 现有的 match 已经把 `Drag(Left)` 单独接走（`src/render/tui.rs:5485`），**拖选与 hover 不会互相污染**。
- **`Moved` 只表示「无按键的移动」**（button_number 3 + motion 位）。heng 现有的 `_ => {}`（`:5487`）正是把 `Moved` 丢掉的地方。
- crossterm 0.29 **不解析 urxvt 1015 编码**（源码里没有 1015 的解析分支；它发了 `?1015h`，但只有 1006(SGR) 与 X10 两种能被解）。因为 1006 在同一串里更靠后、通常优先生效，所以实际不影响。
- crossterm 0.29 **不区分 1002 与 1003**：它只按终端发来的 cb 判定，模式由终端决定。

---

## 6. 别的项目怎么处理「hover 才出现」

（旁证，不是结论。）

- **确实有人做了 hover 层**：`CodeWhale`（Rust TUI）在 `MouseEventKind::Moved` 分支里调用 `crate::tui::hover_layer::set_pointer(mouse.column, mouse.row)`，并据此决定是否显示 sidebar 的 hover popover 与 tooltip。来源：<https://github.com/Hmbown/CodeWhale/blob/46e3a646ed41cdf1ae74cecacb71c6d32ee7d0a0/crates/tui/src/tui/mouse_ui.rs>（`MouseEventKind::Moved => { … set_pointer(…) }`）。它用的就是 crossterm 的 `Moved`——**这条路是通的**。
- **Rust 生态的默认状态**：`crossterm::EnableMouseCapture` 自带 `?1003h`，ratatui 的 `enable_mouse_capture()` 就是它，所以**大多数 ratatui 程序其实已经开着 1003**，只是没人处理 `Moved`。这意味着「做悬浮」在多数终端上**不需要新开任何模式**（heng 已经在开）。
- **生态里 1003 有摩擦**：ncurses 邮件列表上有「SGR (1006) all events (1003) mouse tracking issue」的讨论（<https://lists.gnu.org/archive/html/bug-ncurses/2022-04/msg00020.html>）；VS Code 也有 1003+1006 下鼠标移动把终端强制选择清掉的报告（<https://github.com/microsoft/vscode/issues/194554>）。这些是「开了 1003 会遇到的副作用」，不是「不能开」。
- **多数 TUI 不做 hover**：原因多半是**没有离开事件**（§4）与**事件密度**（§3），而不是终端不支持。

---

## 7. 对 heng 的三条含义

1. **悬浮目标怎么记**：现在的 `mouse()` 在 `match mouse.kind` 之前就 `self.dirty = true`（`src/render/tui.rs:5474`），所以 `Moved` 已经被收到、只是被 `_ => {}` 丢掉。悬浮状态需要**另存一个「指针当前落在哪」**（`Option<(u16, u16)>` 之类），并在每帧用命中表（`action_at` 的同一张表）把它翻译成「哪一行被悬浮」；`Drag(Left)` 与 `Moved` 在事件层已分得开，拖选进行中不必特殊处理，但**拖选时是否保留 hover 是个待定语义**。
2. **黏住怎么办**：协议没有「指针离开」事件，kitty 的扩展又被 crossterm 判为解析错误丢掉（§4），所以**必须自己清**：可用信号是焦点事件（crossterm 的 `EnableFocusChange` / `Event::FocusLost`，heng 尚未发 `?1004h`）、hover 状态超时、或任意键盘输入即清；三条的代价不同（多发一串 ANSI / 悬浮会闪 / 键盘操作后按钮消失），属决策。
3. **要不要节流**：终端侧已按 cell 去重（§3），一次横扫约 80 个事件、最坏 1920 个；crossterm 不节流，heng 每事件一次 `dirty` → 一轮 select + 一次 ratatui diff + 一次终端写。要么在 heng 侧合并（例如同一帧内只取最后一个 `Moved`、或对 hover 状态变化做一帧一次的节流），要么接受「鼠标一动就一帧」的成本——**代价是明确的、可测的**（在真机上数一次横扫产生了几帧）。

---

## 附：来源清单

**规范 / 文档**

- xterm 控制序列（含 1000/1002/1003 定义、1004 focus 事件）：<https://invisible-island.net/xterm/ctlseqs/ctlseqs.html>
- kitty「Reporting when the mouse leaves the window」：<https://sw.kovidgoyal.net/kitty/misc-protocol/#mouse_leave_window>
- kitty mouse 协议文档入口：<https://sw.kovidgoyal.net/kitty/protocol-extensions/>
- wezterm mouse 配置文档：<https://wezterm.org/config/mouse.html>
- Ghostty 配置参考（`mouse-reporting`）：<https://ghostty.org/docs/config/reference#mouse-reporting>
- foot(1) man 页：<https://man.archlinux.org/man/foot.1.en>
- iTerm2 Profiles → Terminal 设置：<https://iterm2.com/documentation-preferences-profiles-terminal.html>
- tmux(1) man 页：<https://man7.org/linux/man-pages/man1/tmux.1.html>
- screen(1) man 页：<https://manpages.debian.org/unstable/screen/screen.1.en.html>
- herdr 文档（Mouse UI / `mouse_capture`）：<https://herdr.dev/docs/concepts/>
- OpenSSH 手册：<https://www.openssh.com/manual.html>

**源码**

- kitty：[modes.h](https://github.com/kovidgoyal/kitty/blob/master/kitty/modes.h)、[mouse.c](https://github.com/kovidgoyal/kitty/blob/master/kitty/mouse.c)
- Alacritty：[term/mod.rs](https://github.com/alacritty/alacritty/blob/master/alacritty_terminal/src/term/mod.rs)、[input/mod.rs](https://github.com/alacritty/alacritty/blob/master/alacritty/src/input/mod.rs)
- Ghostty：[modes.h](https://github.com/ghostty-org/ghostty/blob/main/include/ghostty/vt/modes.h)、[terminal/mouse.zig](https://github.com/ghostty-org/ghostty/blob/main/src/terminal/mouse.zig)、[input/mouse_encode.zig](https://github.com/ghostty-org/ghostty/blob/main/src/input/mouse_encode.zig)
- wezterm：[wezterm-escape-parser/src/csi.rs](https://github.com/wezterm/wezterm/blob/main/wezterm-escape-parser/src/csi.rs)
- foot：[term/csi.c](https://codeberg.org/foot/foot/src/branch/master/term/csi.c)、[term/terminal.h](https://codeberg.org/foot/foot/src/branch/master/term/terminal.h)
- VTE：[src/vte.cc](https://gitlab.gnome.org/GNOME/vte/-/blob/master/src/vte.cc)
- Konsole：[src/Vt102Emulation.cpp](https://invent.kde.org/konsole/konsole/-/blob/master/src/Vt102Emulation.cpp)
- Windows Terminal：[src/terminal/adapter/DispatchTypes.hpp](https://github.com/microsoft/terminal/blob/main/src/terminal/adapter/DispatchTypes.hpp)
- xterm.js：[src/common/InputHandler.ts](https://github.com/xtermjs/xterm.js/blob/master/src/common/InputHandler.ts)
- tmux：[input-keys.c](https://github.com/tmux/tmux/blob/master/input-keys.c)、[server-client.c](https://github.com/tmux/tmux/blob/master/server-client.c)
- zellij：[zellij-server/src/panes/grid.rs](https://github.com/zellij-org/zellij/blob/main/zellij-server/src/panes/grid.rs)、[zellij-client/src/os_input_output.rs](https://github.com/zellij-org/zellij/blob/main/zellij-client/src/os_input_output.rs)
- crossterm 0.29（本机 registry 路径 `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/crossterm-0.29.0/`）：[src/event.rs](https://github.com/crossterm-rs/crossterm/blob/master/src/event.rs)、[src/event/sys/unix/parse.rs](https://github.com/crossterm-rs/crossterm/blob/master/src/event/sys/unix/parse.rs)

**旁证**

- CodeWhale 的 hover 层实现：<https://github.com/Hmbown/CodeWhale/blob/46e3a646ed41cdf1ae74cecacb71c6d32ee7d0a0/crates/tui/src/tui/mouse_ui.rs>
- ncurses 邮件列表关于 1003+1006：<https://lists.gnu.org/archive/html/bug-ncurses/2022-04/msg00020.html>
- VS Code issue #194554（1003+1006 下鼠标移动的副作用）：<https://github.com/microsoft/vscode/issues/194554>
- VS Code issue #96058（集成终端 mouse reporting）：<https://github.com/microsoft/vscode/issues/96058>
