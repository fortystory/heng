# herdr 的一手事实：它给 pane 里留了什么

本票是 [herdr 的一手事实](../issues/03-research-herdr.md) 的产物，服务于[决策图](../map.md)的冻结项 9：
**只认 `HERDR_ENV=1`、在 herdr 里默认关左栏、`[ui]` 可覆盖，不做状态回传与 socket 集成**。
下面每条结论都给出处；查不到的写「查不到」，不补猜测。

## 版本与取证方式

- 本机装的 herdr：`herdr 0.9.5`（`/usr/bin/herdr`，Arch 包 `herdr-bin 0.9.5-1`，编译于 2026-10-16），
  与官方最新文档版本 `0.9.5` 一致 —— https://herdr.dev/docs/
- 官方文档页面本地快照：https://herdr.dev/docs/ 及其子页（`agents/`、`configuration/`、
  `config-reference/`、`cli-reference/`、`socket-api/`、`keyboard/`、`add-herdr-support/`、
  `integrations/`、`troubleshooting/` 等）。
- 官方仓库源码：https://github.com/herdrdev/herdr （master，`distribution/agent-detection/`、
  `src/detect/`、`src/pane.rs`、`src/client/shell/config.rs`、`CHANGELOG.md`、`docs/versions/`）。
  本文引用的源码行号取自该仓库当前 master。
- 版本溯源用仓库的 git 标签（`v0.1.0` … `v0.9.5`）逐标签比对，不引二手报道。

---

## 1. 环境变量全集

### 1.1 herdr 往 pane 里注入什么

`add-herdr-support` 是 herdr 官方给 agent 作者看的「pane 内继承哪些变量」表，原文是
「Every process in a Herdr pane inherits these environment variables」：

| 变量 | 含义 |
| --- | --- |
| `HERDR_ENV` | `1`（在 Herdr 内） |
| `HERDR_PANE_ID` | 你这个 agent 所在的 pane |
| `HERDR_BIN_PATH` | 拥有这个 pane 的 Herdr 可执行文件 |
| `HERDR_SOCKET_PATH` | Herdr 的 API socket |

来源：https://herdr.dev/docs/add-herdr-support/ （0.9.5）

`cli-reference` 的「Environment variables」表是官方的变量清单，它把用途写得很具体：

| 变量 | 文档里的用途 |
| --- | --- |
| `HERDR_CONFIG_PATH` | 覆盖配置文件路径 |
| `HERDR_SESSION` | 给 CLI 命令选命名会话 |
| `HERDR_SOCKET_PATH` | 低层 socket 路径覆盖 |
| `HERDR_PROCESS_DETECTION` | Linux 进程探测策略：`native`（默认）或 opt-in `child-groups` |
| `HERDR_ENV` | 「Set to `1` inside Herdr-managed pane processes.」 |
| `HERDR_PANE_ID` | 「Public pane id for the running pane process.」 |
| `HERDR_TAB_ID` | 「Public tab id for the running pane process.」 |
| `HERDR_WORKSPACE_ID` | 「Public workspace id for the running pane process.」 |
| `HERDR_LOG` | 日志过滤，例如 `HERDR_LOG=herdr=debug` |
| `HERDR_DISABLE_SOUND` | 即使开了声音通知也不放声音 |

（这张表没有把「pane 内」与「herdr 自己」分成两栏，分栏是本文按用途归的类，不是官方措辞。）
来源：https://herdr.dev/docs/cli-reference/ （0.9.5）

对应的源码注入点是 `src/pane.rs` 的 `apply_pane_launch_env`：
`cmd.env(crate::HERDR_ENV_VAR, crate::HERDR_ENV_VALUE)` 无条件执行；
`HERDR_WORKSPACE_ID` / `HERDR_TAB_ID` / `HERDR_PANE_ID` 只在 `PaneLaunchIdentity::Managed` 分支下设置
（`PaneLaunchIdentity::OmitPane` 反而会 `env_remove("HERDR_PANE_ID")`）。
`HERDR_SOCKET_PATH` 与 `HERDR_BIN_PATH` 由 `src/integration/env.rs` 的 `apply_pane_base_env` 设置。
常量定义在 `src/main.rs`（`HERDR_ENV_VAR` / `HERDR_ENV_VALUE`）、`src/integration/env.rs`
（三个 `*_ID`）与 `src/api/mod.rs`（`SOCKET_PATH_ENV_VAR`）。
来源：https://github.com/herdrdev/herdr/blob/master/src/pane.rs 、
https://github.com/herdrdev/herdr/blob/master/src/integration/env.rs

**插件 pane、custom command、popup 另有一套**：`HERDR_PLUGIN_ID`、`HERDR_PLUGIN_ROOT`、
`HERDR_PLUGIN_CONFIG_DIR`、`HERDR_PLUGIN_STATE_DIR`、`HERDR_PLUGIN_ENTRYPOINT_ID`、
`HERDR_PLUGIN_CONTEXT_JSON`、`HERDR_PLUGIN_ACTION_ID`、`HERDR_PLUGIN_EVENT` / `HERDR_PLUGIN_EVENT_JSON`、
`HERDR_PLUGIN_CLICKED_URL`、`HERDR_PLUGIN_LINK_HANDLER_ID`。这不是普通 pane 的变量面。
来源：https://herdr.dev/docs/plugins/ （0.9.5）

**新 pane 会主动清掉一批继承变量**（这一点比「设了哪些」更容易踩到）：
`CODEX_THREAD_ID`、`OMPCODE`、`CLAUDECODE`、`CLAUDE_CODE_CHILD_SESSION`、`CLAUDE_CODE_SESSION_ID`、
`CLAUDE_CODE_MESSAGING_TOKEN`（`src/pane.rs`），以及 `apply_pane_terminal_env` 里的
`ITERM_SESSION_ID`、`LC_TERMINAL`、`LC_TERMINAL_VERSION`、`WEZTERM_PANE`、`KITTY_WINDOW_ID`、
`WT_SESSION`、`TMUX`、`TMUX_PANE`、`STY`、`ZELLIJ`。官方文档对此的说明是
「Herdr removes inherited outer-terminal session markers (iTerm2, WezTerm, Kitty, Windows Terminal,
tmux, screen, and Zellij), iTerm2's `LC_TERMINAL` identity, and known Claude Code, Codex, and OMP
session markers before applying explicit launch environment values.」
来源：https://herdr.dev/docs/cli-reference/ （0.9.5）、
https://github.com/herdrdev/herdr/blob/master/src/pane.rs

对 heng 的直接含义：在 herdr 的 pane 里 **`TMUX` / `STY` / `ZELLIJ` 一定不存在**，
别的复用器的老判据在 herdr 里不会误触发；`HERDR_ENV`、`HERDR_PANE_ID`、`HERDR_BIN_PATH`、
`HERDR_SOCKET_PATH` 一定存在（还有 `HERDR_TAB_ID` / `HERDR_WORKSPACE_ID`）。

### 1.2 `HERDR_ENV=1` 是稳定契约还是实现细节

**结论：是「被公开文档承认的、面向第三方集成的一手判据」，但 herdr 官方没有任何一处把它写成
「永不更改的稳定契约」——文档里找不到稳定性承诺，这一点就是查不到的。**

支持它是「公开面」的证据（都来自官方一手文档）：

1. 官方 agent 指南把 `HERDR_ENV=1` 当作「你是否在 herdr pane 里」的判据：
   「If you are running *inside* a Herdr pane (the environment variable `HERDR_ENV=1` is set)…」
   —— https://herdr.dev/agent-guide.md
2. 官方给出的 tmux 自动连接排除写法，就是照这个变量写：
   `if [[ -z ${TMUX:-} && ${HERDR_ENV:-} != 1 ]]; then tmux new-session -A -s my-session fi`
   —— https://herdr.dev/docs/agents/ （0.9.5，「If your shell automatically attaches to tmux…」）
3. 官方让第三方 agent 自接入的文档，明确要求「Only report when `HERDR_ENV=1` and the other
   variables are set. Outside Herdr, your integration should do nothing.」
   —— https://herdr.dev/docs/add-herdr-support/ （0.9.5）
4. herdr 随包发布的 agent skill 文件把它当守卫条件：`test "${HERDR_ENV:-}" = 1`
   —— https://github.com/herdrdev/herdr/blob/master/skills/herdr/SKILL.md
   （同一份文件也发布在 https://raw.githubusercontent.com/herdrdev/herdr/master/skills/herdr/SKILL.md）
5. socket API 文档把 Herdr 管理的变量写成对调用方 `--env` 有「权威性」：
   「Herdr-managed variables such as `HERDR_SOCKET_PATH`, `HERDR_BIN_PATH`, `HERDR_ENV`, … stay
   authoritative when they conflict with caller-provided env.」
   —— https://herdr.dev/docs/cli-reference/ （0.9.5）
6. 它还被 herdr 自己用来阻止嵌套启动（`src/main.rs` 的 `should_block_nested_for_env`，
   由 `experimental.allow_nested` 开关），以及判断「是否在 herdr 里跑 `herdr update`」
   （`src/update.rs` 的 `running_inside_herdr_env`）。
   —— https://github.com/herdrdev/herdr/blob/master/src/main.rs

查不到的部分：

- 「`HERDR_ENV` 是稳定契约」这句话在官方文档里**查不到**。`socket-api` 只有一节
  「Protocol stability」讲 socket 协议代际（「The client-rendered Herdr UI uses a stable endpoint
  generation for local and SSH servers. Client and server builds do not need to match.」），
  **不覆盖环境变量**。https://herdr.dev/docs/socket-api/ （0.9.5）
- 环境变量没有任何弃用/变更记录。`CHANGELOG.md` 全文里 `HERDR_ENV` 只出现在与 Windows `PATH`
  继承相关的一条无关记录里；没有任何一条说 `HERDR_ENV` 被引入、改名或废弃。
  https://github.com/herdrdev/herdr/blob/master/CHANGELOG.md

### 1.3 哪些版本成立

用仓库标签逐个比对 `src/pane.rs`：

| 标签 | 有 `HERDR_ENV`？ |
| --- | --- |
| `v0.1.0`、`v0.1.1`、`v0.1.2` | 无 |
| `v0.2.0` | **有**（`version = "0.2.0"`，`src/pane.rs` 注入） |
| `v0.3.0`、`v0.4.0`、`v0.4.11`、`v0.5.0`、`v0.5.11`、`v0.5.12`、`v0.9.5` | 有 |

`v0.2.0` 的发布日期是 **2026-03-30**（`CHANGELOG.md` 的 `## [0.2.0] - 2026-03-30` 条）。
`v0.1.2` 与 `v0.2.0` 之间没有其它标签，所以**引入点落在 0.1.2 之后、0.2.0 之内**；更精确的提交
没有逐提交追（`git log -S` 在浅克隆上太慢，已放弃）——这一格写「0.1.2 到 0.2.0 之间引入，
0.2.0 起确定成立」。

版本化站内文档只能佐证「至少从 0.5.12 起就在公开文档里」：`docs/versions/0.5.11/` 没有任何文件提到
`HERDR_ENV`，`docs/versions/0.5.12/agent-skill.mdx` 起有了。更早版本没有版本化文档目录，查不到。

来源：https://github.com/herdrdev/herdr/blob/master/CHANGELOG.md 、
仓库 git 标签 `v0.1.2` / `v0.2.0` / `v0.5.11` / `v0.5.12` / `v0.9.5` 的 `src/pane.rs`。

---

## 2. pane 内全屏 TUI 的行为

### 2.1 herdr 给 pane 里的程序设成什么

`src/pane.rs` 的 `apply_pane_terminal_env`（注释明确说「Each pane is rendered by herdr's own
terminal layer, not the outer terminal that launched the app」）：

| 变量 | 值 | 出处 |
| --- | --- | --- |
| `TERM` | `xterm-256color`（常量 `PANE_TERM = crate::ghostty::TERM`） | https://github.com/herdrdev/herdr/blob/master/src/pane.rs |
| `COLORTERM` | `truecolor` | 同上 |
| `TERM_PROGRAM` | `herdr` | 同上 |
| `TERM_PROGRAM_VERSION` | herdr 的版本号 | 同上 |
| `fish_handle_reflow` | `0`（若调用方没给） | 同上 |

官方文档的同一段话（「New panes identify as `TERM_PROGRAM=herdr`, with `TERM_PROGRAM_VERSION` set
to Herdr's version. They keep `TERM=xterm-256color` and `COLORTERM=truecolor`.」）：
https://herdr.dev/docs/cli-reference/ （0.9.5）

对 heng 的含义：**`COLORTERM=truecolor` 在 herdr 里恒成立**；`TERM` 永远是 `xterm-256color`
（不会透传外层 Ghostty 的 `xterm-ghostty`），所以任何依赖 `TERM` 的 terminfo 分叉在 herdr 里
只走这一支。`TERM_PROGRAM=herdr` 是识别「我在 herdr 里」的第二条路，但冻结项 9 只用 `HERDR_ENV`。

### 2.2 窗口大小

herdr 自己拥有 pane 的伪终端，尺寸用 `ioctl(TIOCSWINSZ)` 设到 pty 主端
（`src/pty/fd.rs` 的 `resize_pty_fd`，调用点在 `src/pty/actor/unix.rs`）。所以 pane 里的程序
通过 `TIOCGWINSZ` 拿到的是 **herdr 分配给这个 pane 的尺寸**，不是外层终端的尺寸——
这也正是 herdr 侧栏能「占列」的实现方式（见第 4 节）。pane 最小尺寸是
`MIN_PANE_COLS = 4` / `MIN_PANE_ROWS = 2`（`src/pane.rs`）。
来源：https://github.com/herdrdev/herdr/blob/master/src/pty/fd.rs 、
https://github.com/herdrdev/herdr/blob/master/src/pane.rs

**herdr 会不会改 `COLUMNS`：查不到。** 在 `src/` 全树里没有 `COLUMNS` 的写入点，官方文档 22 个
页面里也没有 `COLUMNS` 这个词。heng 本身不读 `COLUMNS`（`src/render/layout.rs` 用的是 ratatui
后端给的可绘制区宽度），所以这条不影响本图。

### 2.3 键盘增强协议

官方 `keyboard` 页有一句直接管到 pane 输入：

> Pane input follows the keyboard modes requested by the app. Plain shells receive Ctrl+Shift+letter
> as its control byte; Super, Hyper, and Meta chords are dropped when the pane cannot represent them.
> **Apps using Kitty keyboard input receive the modifiers and key events they request.** Without
> report-all mode, modified Enter, Tab, and Backspace do not receive release reports. Keypad keys and
> modifier-only presses are not forwarded.

来源：https://herdr.dev/docs/keyboard/ （0.9.5）

也就是说 **heng 在 pane 里 push Kitty keyboard protocol（`PushKeyboardEnhancementFlags`）是被支持的**：
`src/client/shell/tests/input_conformance.rs` 里有一类测试就叫「Kitty keyboard host (Ghostty, kitty,
WezTerm, foot...). Herdr pushes 7, …」，`src/client/mod.rs` 里还有「failed to re-assert host
keyboard protocol」的告警路径（说明 herdr 自己也会向宿主终端推协议，并会在客户端重连后重申）。
heng 侧的做法见 `src/render/tui.rs` 的 `enable_terminal_modes`：push
`DISAMBIGUATE_ESCAPE_CODES`（为了让 `Shift+Enter` 与裸 `Enter` 分得开），退出时成对 `Pop`。
来源：https://github.com/herdrdev/herdr/blob/master/src/client/shell/tests/input_conformance.rs

已知的兼容性边界（官方自己记的）：`troubleshooting` 页说
「Older versions of some terminals can emit the release of Enter, Tab, and Backspace as the same
bytes as the press when an application enables Kitty keyboard event reporting. Herdr cannot
distinguish those duplicate bytes after the terminal sends them.」——
这是宿主终端一侧的旧 bug，不是 pane 里程序的问题。
来源：https://herdr.dev/docs/troubleshooting/ （0.9.5）

### 2.4 鼠标上报

- herdr 默认 **自己抓鼠标**：`[ui] mouse_capture = true`，「Capture mouse input for Herdr's mouse UI.
  Set false to let the terminal handle normal clicks」；同一段还写明
  「Pane apps like lazygit and btop can still receive mouse when they request it.」
  —— https://herdr.dev/docs/config-reference/ （0.9.5），也见 https://herdr.dev/docs/concepts/
- 分派规则是「谁先要谁先得」那一套：官方 `socket-api` 说 `terminal.mouse` 事件
  「Herdr encodes the event for the application's mouse mode and **drops it when the application has
  not enabled mouse reporting**, so a plain shell never receives stray input.」
  —— https://herdr.dev/docs/socket-api/ （0.9.5）
- herdr 侧的鼠标状态是逐 pane 跟踪的（`src/pane.rs` 的 `TerminalPatchOutcome`
  里有 `mouse_reporting` / `sgr_pixel_mouse` / `alternate_screen_active` 三个字段）。
  —— https://github.com/herdrdev/herdr/blob/master/src/pane.rs
- 与 herdr 自己 UI 的冲突有明确边界：`pane.input.set` 的 `right_click` 可以设成 `pane`，
  让「unmodified right-click hold and drag gestures」转给请求了鼠标上报的程序；
  「Right-clicking the pane frame always opens Herdr's menu.」左键点击仍然用来聚焦 pane。
  —— https://herdr.dev/docs/socket-api/ （0.9.5）

对 heng 的含义：heng 进 alt-screen 后自己开 `EnableMouseCapture` + `EnableBracketedPaste`
（ADR 0002），在 herdr 里**能收到鼠标**，但外层的 herdr 也开着鼠标捕获，所以「点击聚焦 pane」
「拖边框」「右键菜单」这些手势在 pane 边框与 herdr 自己的 UI 上优先归 herdr；
「拖选复制」也会被 herdr 的 `copy_on_select` 插一手（默认 `true`，
https://herdr.dev/docs/config-reference/ ）。**要不要为此改 heng 的手势不在本票范围。**

### 2.5 alt-screen 与 herdr 自绘 UI 有没有已知冲突

结论：**没有「alt-screen 与 herdr 自绘 UI 冲突」这类已知缺陷记录**；相反，herdr 的 changelog 里
多条修复都是「让 pane 里的全屏 TUI 更正常」：

- 「Pane terminal colors now return to the outer terminal theme after fullscreen TUIs exit, fixing
  cases like Droid leaving stale background colors behind.」（**0.4.6**，2026-04-09，`CHANGELOG.md`）
- 「Full-screen TUIs such as Neovim now receive resize-generated terminal responses after Herdr
  internal pane resizes, so grown panes redraw without waiting for extra input. (#471)」（**0.6.9**，2026-06-10）
- 「Fixed Codex in herdr panes losing transcript/history while running in alternate screen, so past
  output remains scrollable instead of disappearing as the session grows.」（更早，0.3/0.4 线）
- 「Alternate-screen pane reads avoid unnecessary ANSI formatting work, reducing read latency for TUI
  agents. (#2387, #2426)」（0.8.0 线）
- 「Pane apps that request any-motion mouse tracking now receive hover/move events, making
  Textual-style TUI mouse interaction more reliable inside Herdr. (#419)」（**0.6.7**，2026-06-03）
- 「Pane apps that query OSC 4 ANSI palette colors now receive the active terminal palette response,
  so OpenCode and similar TUIs can enable system-theme behavior inside Herdr. (#387)」（**0.6.7**）
  以及 OSC 10/11 默认前景/背景色的同类支持（#253）。

来源：https://github.com/herdrdev/herdr/blob/master/CHANGELOG.md

与 alt-screen 直接相关的**功能边界**（不是缺陷）有两条：

- `clear_pane` 动作「Sends no input to the running program and **does nothing on the alternate screen**
  (for example, Vim).」—— https://herdr.dev/docs/config-reference/ （0.9.5）
- herdr 自己对「全屏 agent 应用」有专门读取路径：「Full-screen agents such as Claude Code and
  OpenCode render transcript history in the terminal's alternate screen instead of Herdr's host
  scrollback. For an idle, recognized agent at the bottom of its transcript, text reads from `recent`
  or `recent-unwrapped` automatically use the agent's mouse-scroll interface…」
  —— https://herdr.dev/docs/agent-automation/ （0.9.5）。注意这条只对**被识别为 agent 的 pane**生效。

另外一条会咬到 alt-screen 应用的交互细节：herdr 在 macOS 上为「隐藏硬件光标的 AI agent TUI」
提供了 `experimental.reveal_hidden_cursor_for_cjk_ime`（默认 false），打开后外层终端会多出一个
硬件光标（vim normal 模式之类）——这属于 opt-in，不是默认行为。
https://herdr.dev/docs/config-reference/ （0.9.5）

**没查到的**：官方文档里没有一节专门讲「pane 里跑全屏 alt-screen TUI 的注意事项 / 已知坑」；
上面这些只能从 changelog 与各页零散条目拼出来。

---

## 3. agent 检测

### 3.1 它怎么认出 pane 里跑的是 agent

两层，官方文档写得很直白：

1. **进程名**（决定「这个 pane 里有没有 agent、是哪一个」）。源码是
   `src/detect/mod.rs` 的 `lookup_agent`：一个写死的名字表，取 `argv0`（或进程名）的 **basename**、
   转小写、剥掉 `.exe`/`.cmd`/`.bat`/`.ps1`/`.js` 后缀，再精确匹配。
   —— https://github.com/herdrdev/herdr/blob/master/src/detect/mod.rs
2. **屏幕内容**（在进程被认出是某个 agent 之后，决定 `idle` / `working` / `blocked`）。官方 agents 页：
   「For agents Herdr supports, Herdr finds the agent's process in each pane and reads the live bottom
   of the pane, not the part you scrolled to. Rules in a detection manifest decide whether that screen
   means `idle`, `working`, or `blocked`.」规则住在 `distribution/agent-detection/<agent>.toml`
   （22 份屏幕 manifest，与 `src/detect/manifests/` 同名同数，对应源码
   `Agent::SCREEN_MANIFEST_AGENTS` 的 22 项），
   还能从 herdr.dev 远程更新（`[update] manifest_check`）。
   —— https://herdr.dev/docs/agents/ （0.9.5）、
   https://github.com/herdrdev/herdr/tree/master/distribution/agent-detection

排查工具：`herdr agent list`（看它认成了什么）、`herdr agent explain <target> [--json|--verbose]`
（看为什么这么判：matched rule、manifest source/version、fallback 原因等）。
—— https://herdr.dev/docs/cli-reference/ （0.9.5）、https://herdr.dev/agent-guide.md

### 3.2 检测列表里有没有会误认 heng 的项

**结论：常量表里没有任何一项会匹配 `heng`，所以按进程名不会误认。** 逐项核对如下（0.9.5 的
`src/detect/mod.rs` `lookup_agent` 全表）：

```
pi | claude, claude-code | codex | gemini | cursor, cursor-agent
devin, devin-cli, "devin cli" | agy, antigravity, antigravity-cli | cline, .cline
omp | mastracode, mastra-code, "mastra code" | opencode, opencode2, open-code
copilot, github-copilot, ghcs | kimi, kimi-code, "kimi code" | kiro, kiro-cli
droid | amp, amp-local | grok, grok-build | hermes, hermes-agent
kilo, kilo-code, "kilo code" | qodercli, qoderclicn, qoder, qodercn
qwen, qwen-code, "qwen code" | letta, letta-code, "letta code" | maki
muse, muse-code, muse-cli | muse-bin-<数字开头>
```

匹配是**精确等值**（`match name { … }`），不是子串：`heng` 不在表里，`heng` 也不会命中
`hermes` / `hermes-agent`。唯一「前缀式」的例外是 `muse-bin-` 后必须紧跟 ASCII 数字
（`is_muse_versioned_binary`），与 `heng` 无关。仓库全树 grep `heng` 也是零命中。
来源：https://github.com/herdrdev/herdr/blob/master/src/detect/mod.rs

**误认的风险面不在名字，而在「谁来报」：**

- **包装进程会扩大匹配面。** `normalized_process_name` 会在进程是 `sh`/`bash`/`zsh`/`fish`
  （带 `-c`）、`node`/`bun`（带 `-e`/`--eval`/`-p`/`--print`）、python（`-c`）、`cmd`、`powershell`/`pwsh`
  时，去 argv 里翻**被包装的 agent 名字**（`wrapped_agent_name_from_runtime_argv`）。
  源码里有一组测试专门防误报，例如「`identify_agent_in_job_ignores_python_c_argument_named_codex`」
  「…ignores_node_eval_argument_named_codex」「…ignores_shell_c_argument_named_codex」。
  也就是说：**只有当 heng 的启动方式恰好把某个表内名字当成 `-c`/`-e` 的实参时才有可能被包装识别**，
  正常 `heng` 二进制不会。https://github.com/herdrdev/herdr/blob/master/src/detect/mod.rs
- **有人可以显式把 pane 冒充成某个 agent**：官方文档给了 `HERDR_AGENT=<agent>` 这个提示变量
  （给 VM/沙箱包装器用）：「Set `HERDR_AGENT=<agent>` on the wrapper command to tell Herdr which
  existing agent screen manifest to use. … The hint applies only to that foreground process.」
  这是**我们可以主动选**的，不是 herdr 的误判。
  —— https://herdr.dev/docs/agents/ （0.9.5）
- 识别出「agent」之后，状态由屏幕规则决定；规则不匹配时官方明确会退回 `idle` 并标
  `default_known_agent_idle_fallback`（在 explain 输出里）。所以「被认成某 agent」的代价是
  **侧栏状态变成屏幕规则猜的**，而不是没有状态。
  —— https://herdr.dev/docs/agents/ （0.9.5）

### 3.3 要不要在 heng 侧做点什么（只给结论与代价）

**结论：本图不做，代价也说明白——不值得。**

- **不做**（推荐，等同冻结项 9）：heng 保持现状，herdr 侧栏把它当普通终端（plain terminal，
  `unknown` 状态）。这与「状态回传 out of scope」自洽，也零成本、零耦合。
- **要让 herdr 认对 heng 的正路是自接入**：官方 `add-herdr-support` 说明第三方 agent 可以自己调
  `"$HERDR_BIN_PATH" pane report-agent "$HERDR_PANE_ID" --source … --agent … --state …`
  上报 `idle`/`working`/`blocked`。代价是 heng 要新增一条「退出/切换状态时上报」的生命周期路径、
  要处理上报失败、要在 `HERDR_ENV=1` 时才有副作用（官方明确要求「Only report when `HERDR_ENV=1`
  and the other variables are set」），并且这条路径与「状态回传不做」的冻结直接冲突。
  —— https://herdr.dev/docs/add-herdr-support/ （0.9.5）
- **冒充路（`HERDR_AGENT=claude` 之类）明确不要**：它只替换屏幕 manifest，不会让 herdr 真的理解
  heng 的界面；一旦屏幕规则碰巧匹配上（例如某些确认框形状），侧栏会显示一个**错的**状态，
  比 `unknown` 更坏。官方自己也提醒「Unusual new agent prompts may initially show as `idle`
  instead of `blocked`… idle status is not authorization to approve an action.」
  —— https://herdr.dev/docs/agents/ （0.9.5）
- **不做也能被识别**的第三种可能：heng 自己实现官方那套「Supported by the agent」协议
  （像 Command Code / Crush / Kiro CLI 那样「report their own state to Herdr」），但那是同一个
  上报面，代价同上。 —— https://herdr.dev/docs/agents/ （0.9.5）

---

## 4. 宽度实况

### 4.1 herdr 是否在 pane 里预留侧栏宽度、会不会改 COLUMNS

**预留：会。** 这是 herdr 客户端布局的直接后果。`src/client/shell/config.rs` 的 `layout()`：

- 若 `cols <= ui.mobile_width_threshold`（默认 **64**）：侧栏 `Rect::default()`（宽度 0），
  pane 面占满整个宽度，另外顶部给 2 行「mobile header」。
- 否则：侧栏宽度 = 折叠时（`sidebar_collapsed_mode`，`compact` = **4** 列、`hidden` = **0** 列）
  或展开时 `sidebar_width.clamp(sidebar_min_width, sidebar_max_width)`，
  再 `.min(cols - 1)`；然后 `main = Rect::new(sidebar_width, 0, cols - sidebar_width, rows)`，
  pane 面从 `main` 里再让掉页签条那一行。

默认值（`herdr --default-config`，0.9.5 本机实测，同官方配置参考）：
`sidebar_width = 26`、`sidebar_min_width = 18`、`sidebar_max_width = 36`、
`sidebar_start_collapsed = false`、`sidebar_collapsed_mode = "compact"`、
`mobile_width_threshold = 64`。
来源：https://herdr.dev/docs/config-reference/ （0.9.5）、
https://github.com/herdrdev/herdr/blob/master/src/client/shell/config.rs

`COLUMNS`：**查不到**（见 2.2）。pane 的宽度是通过 pty 的 `TIOCSWINSZ` 给的，不是靠环境变量。

### 4.2 分屏后 pane 的宽度区间

分成两层：

1. **herdr 客户端这一层**：pane 面的总宽度 = 终端宽度 − 侧栏（0 / 4 / 18–36）。
   在默认展开侧栏下，120 列的终端给 pane 面的宽度是 **120 − 26 = 94**。
   只有 `cols <= 64` 时才完全不扣。
2. **herdr 内部再分屏**：每个 pane 从 pane 面里切；单个 pane 的最小尺寸是
   `MIN_PANE_COLS = 4` / `MIN_PANE_ROWS = 2`（`src/pane.rs`）。阈值与切法在
   `src/ui/panes.rs`、`src/app/api/layouts.rs`；**具体比例（`pane.split` 的默认 ratio、
   边界拖拽的最小值）本文没有逐行核实**，写「未核实」。

所以「分屏后 pane 的宽度区间」的**保守说法**是：`max(4, …)` 到「pane 面宽度」；
在 120 列的常见终端、默认展开侧栏、单个 pane 的情形是 **94 列**，
折叠侧栏（compact）是 **116 列**，折叠成 `hidden` 或 ≤64 列时是 **120 列**。

### 4.3 与 heng 今天的四档是什么关系

heng 的档位判据（`src/render/layout.rs`）：`width >= 120` → 40 列左栏；
`width >= 80` → 28 列；否则隐藏；`MIN_WIDTH = 40`（低于它视为放不下）、`MIN_HEIGHT = 10`。
这里的 `width` 是 ratatui 后端给**整个可绘制区**的宽度。在 herdr 里，heng 是 pane 里的程序，
它拿到的可绘制区就是 pane 的 pty 尺寸——**这一步是把 2.2 的两条一手事实（pane 尺寸由 herdr 用
`TIOCSWINSZ` 设定、herdr 客户端布局会从终端宽度里扣掉侧栏宽度）接起来的推断，官方文档里没有
一句直接写「pane 内程序看到的宽度 = 终端宽度 − 侧栏」**。

因此映射关系是（默认展开侧栏、26 列，不含分屏）：

| 外层终端宽度 | herdr 客户端布局 | heng 在 pane 里看到的宽度 | 命中 heng 的哪一档 |
| --- | --- | --- | --- |
| ≥ 146 | 桌面，扣 26 | ≥ 120 | 40 列宽档 |
| 120 | 桌面，扣 26 | 94 | 28 列档（**不是** 40 列档） |
| 106 | 桌面，扣 26 | 80 | 刚进 28 列档 |
| 90 | 桌面，扣 26 | 64 | 隐藏左栏 |
| ≤ 90 | 桌面直到 65 列 | 39 及以下 | 低于 `MIN_WIDTH = 40`，heng 判为放不下 |
| ≤ 64 | mobile 单列，**不扣侧栏** | 等于终端宽度 | 64 也低于 `MIN_WIDTH`，仍判放不下 |

也就是说：**在 herdr 里，heng 的「宽档（40 列左栏）」要在外层终端约 146 列以上才出现**；
而 120 列的终端下 heng 会画 28 列左栏。另外两端最容易搞反的两点：① herdr 的 mobile 阈值 64
是在 heng 的 `MIN_WIDTH = 40` **之上**就已生效（64 列以下 herdr 自己就不画侧栏了），
所以「herdr 走 mobile」并不等于「heng 放得下」；② 140 列左右的终端在 herdr 里只给 heng
114 列，heng 会把左栏降成 28 列。

来源：heng 侧是本仓库的 [layout.rs](../../../src/render/layout.rs)（`MIN_WIDTH` / `MIN_HEIGHT` /
`SIDEBAR_WIDE` / `SIDEBAR_NARROW` / `SIDEBAR_WIDE_FROM` / `SIDEBAR_NARROW_FROM` / `sidebar_tier`）；
herdr 侧 https://github.com/herdrdev/herdr/blob/master/src/client/shell/config.rs 、
https://herdr.dev/docs/config-reference/ （0.9.5）

---

## 5. 可以以后用的接口（只记录，不规划）

### 5.1 `herdr` CLI

顶层：`herdr`（启动/接入持久会话）、`herdr status [server|client]`、`herdr update`、
`herdr server stop|reload-config`、`herdr channel`、`herdr machine`、`herdr workspace`、
`herdr worktree`、`herdr tab`、`herdr notification`、`herdr agent`、`herdr pane`、
`herdr session`、`herdr integration`、`herdr api`、`herdr completion`、`herdr config`。
来源：https://herdr.dev/docs/cli-reference/ （0.9.5），与本机 `herdr --help` 0.9.5 一致。

- `agent`：`list | get | read | send-keys | prompt | rename | focus | wait | attach | start | explain`
- `pane`：`list | current | get | layout | process-info | neighbor | edges | focus | resize | zoom |
  read | rename | input | split | swap | move | close | send-text | send-keys | wait-output | run |
  report-agent | report-agent-session | release-agent | report-metadata`
- `api`：`snapshot`（打印 live `session.snapshot`）、`schema`（打印/写出内置 API schema）
- `terminal`：`attach | session control | session observe | title set | title clear`
- `integration`：安装/卸载内置 agent 集成

### 5.2 本地 socket API

传输与路径：JSON over local socket。`HERDR_SOCKET_PATH` 是官方的「低层 socket 路径覆盖」
（https://herdr.dev/docs/cli-reference/ ，0.9.5），源码注释把它称为
「Contractual override behavior for auto-detect uses `HERDR_SOCKET_PATH`」。另有一个只在源码里
出现的 `HERDR_CLIENT_SOCKET_PATH`（`src/server/socket_paths.rs` 的 `CLIENT_SOCKET_PATH_ENV_VAR`，
注释写「Legacy environment variable for overriding the client socket path… kept as a fallback for
callers that explicitly need a client-only override when `HERDR_SOCKET_PATH` is not set」；
在那里 `HERDR_SOCKET_PATH` 优先）；它**不在官方文档的变量表里**，只能算源码事实。
`HERDR_SESSION` 给 CLI 选命名会话。pane 进程天然拿到 `HERDR_SOCKET_PATH`
与 `HERDR_BIN_PATH`，所以在 pane 里不需要额外发现步骤。
来源：https://herdr.dev/docs/socket-api/ 、https://herdr.dev/docs/cli-reference/ （0.9.5）、
https://github.com/herdrdev/herdr/blob/master/src/server/socket_paths.rs

能力面（文档「What you can control」原文要点）：建/列/聚焦/改名/关 workspace 与 tab；
列/查/切/换位/聚焦/改尺寸/改名/读/关 pane 与向 pane 送输入；经 CLI helper 列/查/读/prompt/
等待/改名/聚焦/启动/直连 agent；从 hook 与插件上报自定义 agent 状态；订阅事件、等待输出或状态变化；
安装/卸载内置集成；停 server、重载配置。
来源：https://herdr.dev/docs/socket-api/ （0.9.5）

raw method 清单（分域）：`ping`、`server.stop`、`server.reload_config`、`server.agent_manifests`、
`server.reload_agent_manifests`；`notification.show`；`client.window_title.set|clear`；
`session.snapshot`；`workspace.*`、`worktree.*`、`tab.*`、`pane.*`（含 `pane.report_agent`、
`pane.report_agent_session`、`pane.report_metadata`、`pane.release_agent`、`pane.clear_agent_authority`、
`pane.wait_for_output`）；`layout.export|apply|set_split_ratio`；`agent.*`（含 `agent.explain`、
`agent.view.set|clear`）；`events.subscribe|wait`；`integration.install|uninstall`；`plugin.*`。
来源：https://herdr.dev/docs/socket-api/ （0.9.5）

### 5.3 边界（一句话）

按[图](../map.md)的**范围之外**：**状态回传**（`pane report-agent` / `pane.report_metadata` /
`agent.explain` 这类让 herdr 侧栏认对 heng 的路径）与**把 heng 会话变成 herdr pane**
（`pane.split` / `agent.start` / `terminal attach` 这类把会话交给 herdr 管的路径）都不做，
本图只做「读 `HERDR_ENV=1` + 默认关左栏」；上面这张能力清单只是记录，留给人以后自己决定。

---

## 6. 同类先例：`$TMUX` / `$ZELLIJ` / `$STY`

### 6.1 `$TMUX`（tmux 3.8，本机 `man tmux`）

- **有 `TMUX`**：「tmux also initialises the `TMUX` variable with some internal information to allow
  commands to be executed from inside, and the `TERM` variable with the correct terminal setting of
  ‘screen’.」——`man tmux` 的 ENVIRONMENT 一节。也就是说 `TMUX` 的**存在性**是判据，
  文档只说它的**值**是「tmux 自用的内部信息」；`man tmux` 在这一句里**没有写出值的格式**
  （常见实现里是 `socket_path,pid,session_id`，但手册原文没写，按纪律标为未在手册中确认）。
- **`TMUX_PANE`**：「The pane ID is passed to the child process of the pane in the `TMUX_PANE`
  environment variable.」`man tmux` 的 PANES 一节。
- **`TERM`**：「The `TERM` environment variable must be set to ‘screen’ or ‘tmux’ for all programs
  running inside tmux. New windows will automatically have ‘TERM=screen’ added to their environment…」
  `man tmux` 的 `new-session` 一节；`default-terminal` 也要求它是 screen/tmux 的派生。
- 版本：`tmux 3.8`（本机 `tmux -V`），手册日期 September 23, 2026。

**为什么那样定（这一条是本文的解读，不是手册原文）**：手册把 `TMUX` 描述成「为了让命令能从里面
执行」的内部信息，而不是给人当身份标识解析的字段；`TERM` 则是功能性的（pane 里的程序要有一份
匹配的 terminfo，且必须与 tmux 的绘制能力一致），所以 tmux 会「自动加上 `TERM=screen`」。

### 6.2 `$ZELLIJ`（zellij 仓库 master）

- `zellij-utils/src/envs.rs` 定义 `ZELLIJ`、`ZELLIJ_SESSION_NAME`、`ZELLIJ_SOCKET_DIR`，
  注释是「Uniformly operates ZELLIJ* environment variables」。
- **`ZELLIJ` 的值是 `"0"`**：`zellij-client/src/lib.rs` 与 `zellij-server/src/lib.rs` 里
  `envs::set_zellij("0".to_string())`。它是在 **zellij 进程自己**启动时设的，
  **不是**「你在 zellij pane 里」的判据——值本身是 `0`，判断「在不在 zellij 里」要看
  `ZELLIJ_SESSION_NAME` 这类会话变量（`zellij-client/src/session_resolution.rs` 读它）。
  注释与 CHANGELOG 都没写这个 `0` 的用意，**本文不对它的动机下结论**。
- 真正随 pane 走的是会话与 pane 身份：`zellij-window/src/spawn.rs` 列出
  `LAUNCHING_SESSION_VARS = [ZELLIJ, ZELLIJ_SESSION_NAME, ZELLIJ_PANE_ID]`，
  并在 server 启动时把 `ZELLIJ_SESSION_NAME`（会话名）、`TERM=xterm-256color`、`COLORTERM=truecolor`
  发给 server；`zellij-server/src/os_input_output_unix.rs` 给子进程设 `ZELLIJ_PANE_ID`。
  同一个文件里还广告 `KITTY_WINDOW_ID=1`（`ADVERTISED_KITTY_WINDOW_ID`）。
- 变更记录：`CHANGELOG.md` 的 `## [0.5.1] - 2021-04-23` 下有
  「Add ZELLIJ environment variable on startup (https://github.com/zellij-org/zellij/pull/305)」，
  以及后来的「Fix: unify zellij environment variable handling (…/pull/842)」。

来源：https://github.com/zellij-org/zellij/blob/main/zellij-utils/src/envs.rs 、
https://github.com/zellij-org/zellij/blob/main/zellij-window/src/spawn.rs 、
https://github.com/zellij-org/zellij/blob/main/zellij-server/src/os_input_output_unix.rs 、
https://github.com/zellij-org/zellij/blob/main/CHANGELOG.md

### 6.3 `$STY`（GNU screen）

上游文档（GNU screen 手册）本次**没抓到**：`man.openbsd.org/screen.1` 与 `man.openbsd.org/screen`
都返回 404，本机也没装 `screen`（`screen --version` 未找到命令）。所以这一格写「查不到」。

可以确认的只能到这一层：herdr 自己把 `STY` 归入「outer-terminal session markers」并在新 pane 里
删掉（见 1.1），说明 `STY` 的存在性是既有共识的复用器判据——**但 screen 官方的原文这次没拿到**，
按纪律不补猜测。https://herdr.dev/docs/cli-reference/ （0.9.5）、
https://github.com/herdrdev/herdr/blob/master/src/pane.rs

### 6.4 为什么本图只认 herdr

一句话：[图](../map.md)的维护者要的就是 herdr；其它复用器等有人要再说。冻结项 9 因此把判据收到
「只认 `HERDR_ENV=1`」，不铺开 `TMUX` / `ZELLIJ` / `STY` 的组合判据。

---

## 遗留的「查不到」

1. `COLUMNS` 是否被 herdr 改动 —— 源码与官方文档都没有，**查不到**（第 2.2、4.1 节）。
2. `HERDR_ENV` 的逐提交引入点 —— 只定位到「v0.1.2 之后、v0.2.0 之内」，**更精确的提交未追**。
3. herdr 官方是否承诺环境变量稳定 —— **文档里没有这句话**（只有 socket 协议的「Protocol
   stability」一节，不覆盖环境变量）。
4. GNU screen 官方对 `STY` 的定义 —— **上游文档未取到**（见 6.3）。
5. herdr 分屏的具体比例与边界最小值（`pane.split` 默认 ratio、拖拽下限）—— **未逐行核实**。
