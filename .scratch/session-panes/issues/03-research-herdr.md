# herdr 的一手事实：它给 pane 里留了什么

Type: research
Status: resolved
Part of: ../map.md
Blocked by: —

## 问题

[图](../map.md) 的冻结项 9 定了：只认 `HERDR_ENV=1`、只默认关左栏、只做检测。这一票把这条判据与
它的边界钉在**一手来源**上 —— 不许凭印象，来源一律给 URL，能引官方 docs 或仓库文件就不引二手报道。

要查清六件事：

1. **环境变量全集**。herdr 在 pane 里给子进程设哪些变量（`HERDR_ENV` 之外还有没有 `HERDR_*`）？
   `HERDR_ENV=1` 是稳定契约还是实现细节？在哪些版本里成立、有没有变更记录。
2. **pane 内全屏 TUI 的行为**。herdr 对 pane 里的程序做什么（`TERM` / `COLORTERM` / 键盘增强协议 /
   窗口大小 / 鼠标上报）？它的 alt-screen 与自绘 UI 之间有没有已知冲突；heng 的全屏 TUI（ADR 0002）
   在 herdr 里有没有坑。
3. **agent 检测**。herdr 怎么认出一个 pane 里跑的是 agent（`herdr agent list` /
   `herdr agent explain`）；它的检测列表里有没有会误认 heng 的项，从而在它自己的侧栏里显示错误
   状态。要不要在 heng 侧做点什么（比如设一个变量让它认出来）—— 只给结论与代价，不做。
4. **宽度实况**。herdr 是否在 pane 里预留自己的侧栏宽度、会不会改 `COLUMNS`；分屏后 pane 的宽度
   区间，与今天 heng 的 40（`MIN_WIDTH`）/ 28 / 80 / 120 四档是什么关系。
5. **可以以后用的接口**（只记录，不规划）。`herdr` CLI 与本地 socket API 能做什么；「状态回传」
   与「把会话变成 pane」两条被判 out of scope（[图](../map.md) 的**范围之外**），只留一句边界。
6. **同类先例**。`$TMUX` / `$ZELLIJ` / `$STY` 这些既有约定长什么样、它们当初为什么那样定；
   为什么本图只认 herdr（一句话：维护者要的就是 herdr，别的等有人要再说）。

## 作答

**（2026-10-17，research subagent 完成，只读；产物 [`research/03-herdr.md`](../research/03-herdr.md)，
556 行，六节，每条带来源 URL。）**

1. **`HERDR_ENV=1` 是官方承认的一手判据，但没有任何稳定性承诺**。三处官方用法：agent 指南把它当
   「你是否在 pane 里」的判据（https://herdr.dev/agent-guide.md ）；集成文档要求第三方
   「Only report when `HERDR_ENV=1` and the other variables are set」
   （https://herdr.dev/docs/add-herdr-support/ ）；官方给的 tmux 自动连接排除写法就照它写。随包
   skill 也用它当守卫（`test "${HERDR_ENV:-}" = 1`）。**查不到**的是「它稳定」这句话：`socket-api`
   的 Protocol stability 一节只讲 socket 协议代际，不覆盖环境变量；`CHANGELOG.md` 里没有它的引入 /
   改名 / 废弃记录。版本上：`v0.1.2` 无、`v0.2.0`（2026-03-30）起确定有。**给 heng 的口径**：照
   冻结项 9 用它，但当成一个**可能消失的信号**写 —— 读不到就等于不在 herdr（默认开左栏），
   检测失败不许反过来关掉人的左栏。落点归
   [上限、配置与 herdr 默认值的落点](11-grilling-limits-config-and-herdr.md)。
2. **误认 heng 的风险：名字表不会命中**。`lookup_agent` 是**精确等值表**（`pi` / `claude` /
   `codex` / … / `muse` 共 24 项，`src/detect/mod.rs`），`heng` 不在表内、也不命中 `hermes`，全树
   grep `heng` 零命中。风险只在两处：**包装进程**（`bash -c X`、`node -e X` 会去 argv 里翻 agent
   名，herdr 有专门的防误报测试）与有人**主动**设 `HERDR_AGENT=<agent>`。结论：heng 侧**不做**任何
   事，代价写在产物 §3.3。
3. **pane 内的环境**：`TERM=xterm-256color`、`COLORTERM=truecolor`、`TERM_PROGRAM=herdr`；Kitty
   keyboard 与 pane 内的鼠标上报都支持；alt-screen 无已知冲突记录（changelog 里全是对全屏 TUI 的
   修复）；`TMUX` / `STY` / `ZELLIJ` 被删掉 —— 所以「只认 herdr」不会顺带误判别的复用器。
   `COLUMNS` 查不到：pane 宽度是 pty 的 `TIOCSWINSZ` 给的，不走环境变量。
4. **宽度实况（本图最重要的一条）**：herdr 客户端布局会**从终端宽度里扣掉自己的侧栏**
   （默认 `sidebar_width = 26`；折叠 `compact` 是 4、`hidden` 是 0；展开时钳在 18–36；
   `cols <= 64` 走 mobile、完全不扣）。所以：

   | 外层终端宽度 | heng 在 pane 里看到的宽度 | 命中 heng 的哪一档 |
   | --- | --- | --- |
   | ≥ 146 | ≥ 120 | 40 列宽档 |
   | 120 | **94** | **28 列档（不是 40 档）** |
   | 106 | 80 | 刚进 28 列档 |
   | 90 | 64 | 隐藏左栏 |
   | ≤ 64 | 等于终端宽度（mobile 不扣） | 仍可能低于 `MIN_WIDTH = 40` |

   三个直接后果：① **「在 herdr 里默认关左栏」是双重正确** —— 宽度本来就被扣掉一档；②
   [右栏 40 / 28 列里的轨迹长什么样](04-prototype-right-rail-trace.md) 的 **28 档是 herdr 常态**，
   40 档要外层约 146 列；③ [平铺的几何与两态布局](05-prototype-panes-layout.md) 的降级帧要**把
   94 列当主档**，且 ≤64 列时 herdr 自己走 mobile 而 heng 的 `MIN_WIDTH` 在那之上就已失效 ——
   两端最容易搞反。两张票的口径已按此更新。
5. **接口只记录**：`herdr` CLI 与本地 socket API 的能力清单在产物 §5；「状态回传」与「把会话变成
   pane」按冻结项 9 判 **out of scope**（图里的**范围之外**），只留一句边界。
6. **同类先例**：`$TMUX`（tmux 3.8 手册）、`ZELLIJ=0` 与 `ZELLIJ_SESSION_NAME`（zellij 源码 +
   CHANGELOG PR 305）；`$STY` 因上游文档 404 写「查不到」。只认 herdr 是维护者的选择（冻结项 9），
   不是技术判据。

## 作答之后

- 判据与降级口径 → [上限、配置与 herdr 默认值的落点](11-grilling-limits-config-and-herdr.md)。
- 两张形态票的宽度口径已按第 4 条更新。

---

**产物**：`.scratch/session-panes/research/03-herdr.md`。每条结论带来源 URL 与（能拿到的话）版本号；
查不到的写「查不到」。
