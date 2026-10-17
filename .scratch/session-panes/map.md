# 多会话平铺与右侧栏（wayfinder 决策图）

Label: `wayfinder:map`
Status: 12/12 resolved —— **图已走完并折成 [`spec.md`](spec.md)**（2026-10-17；三 research + 两 prototype + 七 grilling 全部收口）
Tracker: local markdown —— 见 [`docs/agents/issue-tracker.md`](../../docs/agents/issue-tracker.md)
Charting: **已完成**（2026-10-17，三轮 grilling 共十九问：终点形状 / 范围 / 相邻意向 / 轨迹落点 /
平铺的进程模型 / 左右侧栏 / 右栏默认 / 标签的含义 / 键盘归属 / 右栏归谁 / 主列页签条 / herdr 边界 /
谁进平铺 / 平铺排法 / 平铺入口 / 右栏容器 / 右栏宽度与轨迹降级 / 左栏会话页去留 / herdr 默认可否覆盖）。
本图只做**规划**，不产代码改动。

## 目的地

一份**可执行的 spec**（交给 `/to-spec` 折成构建计划）：把衡的外壳从「一次一个会话 + 左栏 +
主列页签条」改成「**顶部页签条** + 最多**四个会话平铺** + **右栏**（第一个页面是轨迹）」，
并在 herdr 里**默认关左栏**。

**范围**：`src/render/`（`tui.rs` / `layout.rs` / `pane.rs` / `transcript.rs` / `wording.rs` /
`palette.rs`）、`src/session/host.rs` 与装载路径（`src/cli/interactive.rs`、`src/cli/loading.rs`）、
`src/config.rs` 的 `[ui]` 段、`CONTEXT.md` 的词条、`docs/render.md`、
[`docs/tui-manual-checklist.md`](../../docs/tui-manual-checklist.md) 与
[`docs/lifecycle.md`](../../docs/lifecycle.md)。**模型可见文本与事件 schema 默认零变化** —— 要动
必须以某张票的结论为准。

## 笔记

- **领域**：衡（heng）的 TUI 外壳与多会话层。今天一屏只有一个会话（宿主表里可以有几个，但前台
  指针全进程唯一），轨迹是主列页签条上的第二页（`trace-in-main`），左栏四个页面靠点切换
  （`session-tab` / `files-page` / `todo-page` / `diff-page`），左栏整栏由 `Ctrl-O` 开关
  （`sidebar-toggle`：**意愿 × 宽度档**，意愿只活在进程内）。
- **既有决定必须先读**：[`sidebar-toggle/spec.md`](../sidebar-toggle/spec.md)（左栏开关）、
  [`trace-in-main/spec.md`](../trace-in-main/spec.md) 与 [`trace-tab/spec.md`](../trace-tab/spec.md)
  （轨迹的两次搬家，**本图第三次推翻它**）、[`trace-shortcuts/spec.md`](../trace-shortcuts/spec.md)
  （`Ctrl-V` 与两套提示行）、[`session-tab/spec.md`](../session-tab/spec.md)（宿主表、前台指针、
  装载、未读寿命）、[`trace-ledger/spec.md`](../trace-ledger/spec.md)（轨迹的键盘层序、宽度重放、
  按行键替换任意一行）、[`questionnaire-reading/spec.md`](../questionnaire-reading/spec.md) 与
  [ADR 0010](../../docs/adr/0010-questionnaire-keys-dispatch-by-zone.md)（键盘跟着当前那一层）、
  [`tui-visual-language/spec.md`](../tui-visual-language/spec.md)（语义色板与字形语法）、
  [`tui-chrome/`](../tui-chrome/spec.md)（外壳已经收过一轮）。
- **一手事实（charting 时核过，票里要复核到行）**：左栏宽度档 = **40 列（≥120）/ 28 列（≥80）/
  隐藏（<80）**，`layout::MIN_WIDTH` 是 40；已占的 Ctrl 位是 `O`（左栏）、`V`（主列两页）、
  `T`（模型）、`C`（取消）、`D`（退出）、`Z`（挂起）、`J`（换行）、`K` / `U`（输入区行编辑）；
  herdr 在 pane 里设 `HERDR_ENV=1`，并且**从终端宽度里扣掉自己的侧栏（默认 26 列）** —— 外层
  120 列的终端里 heng 只见 **94 列**（落 28 档），要外层约 146 列才命中 40 档（来源见
  [herdr 的一手事实](issues/03-research-herdr.md)）。
- **要咨询的 skills**：`/research`（三张事实票）、`/prototype`（两张形态票）、`/grilling` +
  `/domain-modeling`（七张决策票；术语会进 `CONTEXT.md`）。
- **每张票的答案必须自足**：`/to-spec` 会在别处的会话里读它，看不到本图与 charting 对话。
- **文档纪律**：新增或改写的 `docs/**` 要过
  [`scripts/check-doc-size.py`](../../scripts/check-doc-size.py) 的压表达规则（`docs-slim`）。
- **相邻的种子**：`multi-role-view`（多角色分屏）与 `agent-teams`（同目录会话通信）**都不进本图**；
  平铺是后者的承载，接口留在 **尚未明确** 里。

### 冻结项（charting 的 grilling 定下，票里不得重开）

1. **destination 是一份 spec**，不是「只做一个决定」；本图不含实现落地。
2. **平铺 = 同进程多会话**：复用宿主表与 `Harness`；**不**做多进程，**不**把分屏交给 herdr / tmux
   的 pane。
3. **平铺集合是显式的**（人把会话加进来，不自动按活跃度挑）；上限**默认 4**、落 `[ui]` 配置；
   超出**拒绝并给回执**。
4. **排法**：1 独占 / 2 左右 / 3 L 形 / 4 上下各二；宽或高不够时**自动回标签态**，不硬挤。
5. **默认标签态**；**新造顶部页签条**；左栏【会话】页**保留** —— **左栏管状态，顶部管显示**
   （维护者在第三轮没走「会话页退役」那条推荐）。
6. **右栏与左栏并存**，窄屏**右栏先让位**；右栏**默认开、可手势关**；宽度**同左栏制**
   （40 / 28 / 隐藏）；右栏顶部一条**页面标签**，**轨迹是第一页**。
7. **轨迹从主列搬进右栏**（推翻 `trace-in-main`）；**主列页签条删掉**，`Ctrl-V` **改指右栏开关**。
8. 平铺时**底部全局一份输入区**、绑**焦点 pane**；**右栏全局一份**、跟**焦点会话**。
9. **herdr 判据只认 `HERDR_ENV=1`**：只默认关左栏（`Ctrl-O` 仍能叫回），`[ui]` 可显式覆盖；
   状态回传与 socket 集成不做。
10. **`multi-role-view` 不进本图**：那是同一份流的多个发言人，本图是多份流。

## 任务清单

<!-- 条目数必须等于 issues/ 下的子票数（scripts/wayfinder-check.py 校验）。frontier 的权威查询仍是扫 issues/ 里 open + unblocked + unclaimed 的票。 -->

**事实票**（AFK，charting 当天派出 subagent）：

- [x] [宿主表与多会话基础设施：同屏多会话站在哪些件上](issues/01-research-session-infrastructure.md)
- [x] [视图外壳与键盘：哪些件假定「全屏只有一份」](issues/02-research-view-shell-and-keys.md)
- [x] [herdr 的一手事实：它给 pane 里留了什么](issues/03-research-herdr.md)

**形态票**（HITL，先提 fidelity）：

- [x] [右栏 40 / 28 列里的轨迹长什么样](issues/04-prototype-right-rail-trace.md)
- [x] [平铺的几何与两态布局：几帧真字符](issues/05-prototype-panes-layout.md)

**决策票**（HITL，grilling 收口）：

- [x] [全屏唯一的东西在平铺下怎么活](issues/06-grilling-single-instance-surfaces.md)
- [x] [平铺的手势与焦点：谁吃键盘、怎么切](issues/07-grilling-pane-gestures-and-focus.md)
- [x] [右栏的键盘与页面容器](issues/08-grilling-right-rail-keyboard.md)
- [x] [顶部页签条与左栏会话页的分工](issues/09-grilling-tab-strip-and-session-page.md)
- [x] [每会话的视图状态与两态切换](issues/10-grilling-per-session-view-state.md)
- [x] [上限、配置与 herdr 默认值的落点](issues/11-grilling-limits-config-and-herdr.md)
- [x] [轨迹搬走之后：主列、页签条与文档](issues/12-grilling-main-column-after-move.md)

**实现票**（2026-10-17 `/to-tickets` 拆出，`Type: implement`，由实现批次认领 —— 不进 wayfinder 的
frontier）：

- [ ] [13: 右栏容器与轨迹搬家](issues/13-right-rail-and-trace.md)
- [ ] [14: 右栏开关与键盘层序](issues/14-right-rail-keyboard.md)
- [ ] [15: 顶部页签条](issues/15-top-tab-strip.md)
- [ ] [16: 删主列页签条与提示行改造](issues/16-remove-main-tabs-and-hints.md)
- [ ] [17: 平铺两态与几何](issues/17-tiles-two-modes-and-geometry.md)
- [ ] [18: 平铺集合与焦点手势](issues/18-tiles-set-and-focus.md)
- [ ] [19: 集合生灭与满员拒绝](issues/19-tiles-lifecycle-and-limit.md)
- [ ] [20: 问卷与权限门每会话](issues/20-question-and-ask-per-session.md)
- [ ] [21: 输入区草稿与状态行全局读数](issues/21-draft-and-status-readout.md)
- [ ] [22: 详情覆盖层与拖选](issues/22-detail-overlay-and-drag.md)
- [ ] [23: `View` 每会话：投影常驻](issues/23-view-per-session.md)
- [ ] [24: 重放保真](issues/24-width-replay-fidelity.md)
- [ ] [25: 会话属性每会话](issues/25-session-facts-per-session.md)
- [ ] [26: 配置的两个新键](issues/26-ui-config-keys.md)
- [ ] [27: herdr 检测与左栏初值](issues/27-herdr-detection-and-sidebar.md)
- [ ] [28: 词条、文档与 ADR](issues/28-glossary-docs-and-adr.md)

共 **28** 张票：12 张决策票（3 research + 2 prototype + 7 grilling）**全部收口**，另 16 张实现票
（`13`–`28`）待实现批次认领。

## 已定的决定

<!-- 索引：每条一行，够判断相关性即可；细节住在票里。按名字引用，不写裸编号。 -->

- [视图外壳与键盘：哪些件假定「全屏只有一份」](issues/02-research-view-shell-and-keys.md) — 键盘阶梯是
  **17 个提前返回的守卫层 + 一段兜底**（`tui.rs:7279`–`:7509`，注释自称「十六层」，不符）；
  **「全屏唯一」分两类** —— 屏幕资源（一块浮层、一个指针、一个 pty、一把举手）本就只能一份，真正
  的改造主战场是**每会话数据画在唯一矩形里**的那三处（状态行 `tui.rs:9429`、提示行 `:8129`、
  输入区 `:2506`）；逐件归属：输入区与模型选择器**全局一份读焦点**、问卷与权限门**全局一份绑发起
  会话**（reply 通道唯一）、详情覆盖层**不缩进 pane**、拖选 / OSC 52 / 退出举手**不动**；几何今天
  只有一个输入 `TuiState::area`（`tui.rs:2667`），pane 的内接矩形要由**新增的一个外层分栏**供给
  （点击链上 6 处重跑 `layout::plan`）；焦点 pane 层插在左栏之前，但**最省的形态是它不持有键盘**；
  `FrontEndEvent` 里**六个手势不带 `SessionId`**（含 `Cancel`），多 pane 后 `Ctrl-C` 必须补身份；
  每 pane 一份的样板已有（`SessionLight` 整份收发、两个 `Viewport`、按 `BlockId` 记的过滤 / 折叠 /
  选中）。票面「18 变体」不实：`layout::Regions` 17 字段、`HitAction` 19 变体。产物
  [`research/02-view-shell-and-keys.md`](research/02-view-shell-and-keys.md)（285 行，含「全屏唯一物 ×
  归属改造」全表）。
- [宿主表与多会话基础设施：同屏多会话站在哪些件上](issues/01-research-session-infrastructure.md) —
  **通道层已经就绪，缺的是消费端**：`RenderEvent` 七种全带 `session: SessionId`
  （`render/mod.rs:86`–`:139`），而 `live_event` 第一句就把非前台事件丢掉（`tui.rs:4441`），投影
  只有一份 —— 多 pane 的核心改造是**按 `SessionId` 路由 + 每个 pane 一份 `View`**。前台指针
  （`host.rs:260`）在生产里只有一个真订阅者 `SessionAsker`（`asker.rs:62`），**够当「焦点」、不够当
  「平铺集合」**。两个最容易忽视的单例：`TuiState.cwd` / `home` 由组装钉死且 `SwitchSession`
  不更新（`tui.rs:2479`、`:6987`），终端标题与 `@` 候选因此**钉在启动那个会话的 cwd 上**（现存缺陷，
  本图顺手收）；整屏几何 `layout::plan` / `Regions` 没有 pane 维度。**术语撞车**：`pane.rs:30` 的
  `Pane` 是转录滚动缓冲，本图说的「pane」在代码里没有对应类型（命名归
  [轨迹搬走之后](issues/12-grilling-main-column-after-move.md)）。宿主表本身**无上限**
  （`host.rs:258`），「4」是装载策略。**没有「多份流」的既有先例**：执行者（`executor.rs:126`）与
  讨论者（`session/mod.rs:301`）都与父会话共用同一个 `SessionId` —— 这**证实**了冻结项 10。产物
  [`research/01-session-infrastructure.md`](research/01-session-infrastructure.md)（310 行，两张底账表）。
- [herdr 的一手事实：它给 pane 里留了什么](issues/03-research-herdr.md) — **`HERDR_ENV=1` 是官方
  承认的一手判据，却没有任何稳定性承诺**（agent 指南、集成文档的「Only report when `HERDR_ENV=1`」、
  官方给的 tmux 排除写法、随包 skill 都依赖它；而 `socket-api` 的 Protocol stability 只覆盖 socket
  协议，`CHANGELOG` 里没有它的引入 / 废弃记录；`v0.1.2` 无、`v0.2.0`（2026-03-30）起确定有）→ heng
  侧把它当成**可能消失的信号**：读不到就等于不在 herdr（默认开左栏）。**误认 heng 的风险是零**：
  `lookup_agent` 是**精确等值表**（24 项，`src/detect/mod.rs`），`heng` 不在表内；风险只在包装进程
  与有人主动设 `HERDR_AGENT` —— 结论是 heng 侧**不做**任何事。pane 内已设
  `TERM=xterm-256color` / `COLORTERM=truecolor` / `TERM_PROGRAM=herdr`，并**删掉**
  `TMUX` / `STY` / `ZELLIJ`（所以「只认 herdr」不会顺带误判）。**宽度实况（本图最重要的一条）**：
  herdr 客户端**从终端宽度里扣掉自己的侧栏**（默认 26；折叠 `compact` 4；`cols <= 64` 走 mobile
  不扣）—— 外层 **120 列的终端里 heng 只见 94 列（落 28 档）**，要外层约 **146 列**才命中 40 档；
  90 列终端只剩 64 列（左栏隐藏）；≤64 列时 herdr 自己走 mobile，而 heng 的 `MIN_WIDTH = 40` 在那
  之上就已失效（两端最容易搞反）。**三个直接后果**：①「在 herdr 里默认关左栏」是双重正确；②右栏
  轨迹的 **28 档是 herdr 常态**；③平铺的降级帧要把 94 列当主档。两张形态票的口径已按此更新。产物
  [`research/03-herdr.md`](research/03-herdr.md)（556 行，六节，每条带来源 URL）。
- [右栏 40 / 28 列里的轨迹长什么样](issues/04-prototype-right-rail-trace.md) — 12 帧、200 行，
  维护者过帧判定「够用」。**答案一句话：右栏里的轨迹不是主列轨迹的缩小版。** 28 档下
  「时刻 + 用量」两样都保不住（保样样只剩 15 列、保时刻只剩 19 列且用量被丢），唯一读得通的排法
  是把两样都拿掉（正文 28 列）—— 右栏轨迹只能当**缩略索引**，或者行格式要改。40 档四档候选的
  正文分别是 15 / 18 / 24 / 31 列，没有免费的。**「组头并进那条虚线」在右栏失效**（40 档剩四列、
  28 档长组头整条丢掉、退成裸文字）。**时间轴在右栏永远只有 Compact 一档**（40 档轴 24 列、28 档
  轴 12 列，Full 要 44 列）。整屏：herdr 实况 94 列下三块全开**主列只剩 36 列**、80 列全开只剩
  22 列 —— **「窄屏右栏先让位」是必需项**。四条读法问题（28 档读法、40 档选哪档、组头、时间轴 +
  详情入口归属）交给 [右栏的键盘与页面容器](issues/08-grilling-right-rail-keyboard.md)；
  让位阈值交给 [平铺的几何与两态布局](issues/05-prototype-panes-layout.md) 与
  [全屏唯一的东西在平铺下怎么活](issues/06-grilling-single-instance-surfaces.md)。产物
  [`prototype/04-right-rail-trace.txt`](prototype/04-right-rail-trace.txt) +
  [`04-right-rail-trace.py`](prototype/04-right-rail-trace.py)（带显示列自检）。
- [平铺的几何与两态布局：几帧真字符](issues/05-prototype-panes-layout.md) — 11 帧、518 行，
  维护者过帧判定「够用」。**答案：平铺的门槛一在宽度、二在高度，而最锋利的一条是「平铺态必须
  收起两侧栏」**。判据 = 每格 ≥ 40 列 × 10 行，pane 区 = 屏高 − 9。三档数字：**24 行的终端只
  撑得住 2 个 pane**（3 / 4 个都要 ≥ 30 行）；**2 个 pane 要 ≥ 81 列**（80 列时每格 39 列）；
  herdr 实况 94×40 下 4 个各 46×15、可行。另外：**标签态自己已经越界** —— 120 列两侧栏全开时
  主列只剩 38 列（低于 `MIN_WIDTH`），所以「窄屏右栏先让位」的阈值不能等到 120 列以下；**1 个
  pane 的平铺态与标签态几乎一样**（要不要允许归票 07）；**pane 边界按字形语法应是虚线**（帧里
  实线只为看清几何）。产物 [`prototype/05-panes-layout.txt`](prototype/05-panes-layout.txt) +
  [`05-panes-layout.py`](prototype/05-panes-layout.py)。
- [全屏唯一的东西在平铺下怎么活](issues/06-grilling-single-instance-surfaces.md) — 两轮十问。
  **全屏唯一物分三档**：①**读焦点**（输入区、状态行、提示行、终端标题）；②**绑发起会话、全屏
  接管**（问卷、权限门、详情覆盖层、文件查看器）；③**本来就只能一份**（拖选、举手、剪贴板）。
  逐条：输入区草稿**每会话各存**（沿用 `SessionLight::editor` 的 stash / load）；状态行与提示行全局
  一份读焦点，并加一段**全局读数**「另有 N 个在跑 / M 个等人类」（`background_waiting` 本来就是这
  一档）；**问卷与权限门全屏接管、焦点自动跳过去，而接管期间可以切焦点、题面跟着焦点走** —— 这条
  推翻本票最初的设想：`TuiState.pending`（`tui.rs:2678`）要从**单例**改成**每会话一份**（每份自带
  自己的 reply），票 02 的作答里已留校订行；详情覆盖层照旧**整屏居中**、**记录「打开方」**按它
  还原、已开着的**不因切焦点而换归属**；拖选**仍夹在一块文本区域里**（跨 pane 不做）；`Ctrl-C`
  取消**焦点 pane**、**举手仍进程级一个**；终端标题写**焦点会话**，顺手修掉票 01 报的 `TuiState.cwd`
  不跟 `SwitchSession` 走那个缺陷。**没动冻结项。**
- [平铺的手势与焦点：谁吃键盘、怎么切](issues/07-grilling-pane-gestures-and-focus.md) — 两轮十一问，
  **三处维护者没走推荐**。**手势表**：标签态 ⇄ 平铺态 = **`Ctrl-R`**（不是推荐的 `Ctrl-F`）+ 顶部
  页签条那个按钮；加入 = 平铺态末尾的 **`[+ 加入]` 格**（左栏会话页的双击已被「关闭会话」占着，
  票 32）；移出 = 焦点格标题行的 `×`；换焦点 = **`Ctrl-1`…`Ctrl-4`** + 鼠标点格子；切标签 =
  点标签 + **`Ctrl-左右方向键`**；满 4 个再加 = **拒绝**并给回执。**语义**：集合按**加入顺序**
  （不按活跃度漂），`[+ 加入]` 参与布局（集合 n 个 → n + 1 格）；**焦点就是宿主表前台，集合是另
  一个有序列表** —— **集合为空**时进平铺**自动加入焦点**（否则第一次永远进不去）、**非空而焦点不在
  里面**时**拒绝进平铺**并提示（维护者的选择）；平铺态内切焦点**只在集合内**，几何不因切焦点而
  变；关闭 → 移出并把焦点给集合里的下一个，新开 / 装载 → **自动加入**，**只剩一个 → 自动回标签
  态**（帧 05 报的），`/clear` 翻页 → 新会话**接替**旧位。**一项新事实**：`map_key`
  （`tui.rs:172`）今天**忽略所有修饰键**，`Ctrl-数字` / `Ctrl-方向` 要先加修饰判断，而 `Ctrl-方向`
  在部分终端 / 复用器里可能被吃掉（鼠标点击是退路）。**没动冻结项**（3 的三条被落实）。
- [右栏的键盘与页面容器](issues/08-grilling-right-rail-keyboard.md) — 三轮十三问，**两处维护者没走
  推荐**。**容器**：**`Ctrl-V` 一次按下 = 开右栏 + 把键盘交给右栏**，再按一次交还键盘并关栏；
  **点输入区 = 交还键盘但右栏留着**；页面标签**横排、单页时不画**；滚动 / 贴底跟随 / 新内容指示器
  **跟会话走**；窄档照旧，宽度 < 80 列（右栏隐藏）时 `Ctrl-V` 给一句回执；右栏里点开的详情**还原给
  右栏自己的滚动位置**。**轨迹在右栏的形态**（接票 04 的四条）：40 档 = **保九列时刻、弃行尾用量**
  （正文 31 列）；**28 档 = 缩略索引**（不画时刻、不带用量，正文 28 列）；**保留组头、放弃「并进
  虚线」**；时间轴**只画 Compact 那一档**（2 行、一行三态）。**与平铺的关系**（走查暴露）：
  **平铺态右栏照开**（与票 05 的「平铺必须收起两侧栏」相反）、**每格只画对话视图**，代价是
  **平铺态的 pane 宽度下限从 40 降到 32**（标签态仍用 40）—— 这条**修正了票 05 的判据**，票 05 的
  作答里已留校订行（2 格门槛 81 → 65 列）。
- [顶部页签条与左栏会话页的分工](issues/09-grilling-tab-strip-and-session-page.md) — **顶部列宿主表、
  左栏列盘上会话**（两处数据源不同）。标签只写标题 + 焦点 + 集合成员标记，**状态类一个都不上标签**
  （等人类也不破例，由状态行的全局读数承担）；**未读仍只在终端标题**（左栏行本来就没有这个点）；
  顺序＝宿主表装载顺序，溢出收成不可点的 `+N`，**顶部不做过滤**；标题与左栏同一份 `title_of`、
  **单个标签 ≤ 24 列**、不写 `父/基名`；几何＝**横跨整屏 2 行**（标签行 + 虚线）、删主列页签条那
  2 行（`CHROME` 6 → 6）、左栏顶部三件下移 2 行、右栏从条下起算、**屏高 < 20 收成 1 行**；
  **平铺态自动收起左栏**（补上票 05 被票 08 只改了一半的那条，也才让票 08 的 32 列下限成立）。
  两条事实校订：未读从不在左栏会话页；左栏单击不切前台（票 32：`Enter` / 双击才切）。
- [每会话的视图状态与两态切换](issues/10-grilling-per-session-view-state.md) — **一张 25 行的归属表**
  （`/to-spec` 直接用）：`View` 升为 `HashMap<SessionId, View>`（投影常驻、不再切会话整块重造）；
  平铺集合是 `Vec<SessionId>`（有序、≤4、**不允许重复**），所以**没有新的每 pane 状态**，`Viewport`
  留每会话；`cwd` / `home` / `files` / `catalog` / `read_only` 从单例改成**每会话**（校订票 01
  表二四条初判，顺带修掉 `SwitchSession` 不换 cwd 的既有缺陷），`catalog` 切焦点要重发；
  `pending` 每会话一份，`detail_opener` / `detail_rect` / `detail_serial` 搬进 `SessionLight`
  （今天那个「切回旧会话按错档还原」的缺口）；浮层本体、`regions` / `drag`、未读账、退出举手仍全局。
  **两态切换走既有宽度重放**（按 `top_source` 保位 + 按 `BlockId` 认选中 / 过滤 / 折叠）；
  **轨迹的宽度只剩一个来源（右栏内宽）**，两态切换不重放 —— 票面第 4 条就此消解。容量**不做懒算**，
  `CAP = 20_000` 不动。
- [上限、配置与 herdr 默认值的落点](issues/11-grilling-limits-config-and-herdr.md) — **`[ui]` 两个新键**：
  `tile_limit`（`2..=4`，默认 4，超出拒绝）与 `sidebar`（`auto` / `always` / `never`，默认
  `auto`）；非法值一律**拒绝启动**（照 `number_style` 惯例）。超上限回执走提示行一句
  「平铺最多 {N} 个；先移出一个再加」。**herdr 只认 `HERDR_ENV == "1"` 精确等值**、只在 `auto`
  下**读一次**（TUI 组装时），plain / headless 不读；结果只当 `sidebar_wanted` 的初值，
  `Ctrl-O` 照旧改它、重启回到默认关；读不到 = 不在 herdr。**右栏让位**：档从 40 往下降，取第一个
  让主列 ≥ `MIN_WIDTH`(40) 的（120 列两侧栏同开 → 右栏 28、主列 50；≥ 122 右栏才是 40；
  < 98 隐藏），平铺态按「pane 区 ≥ 32 × 列数 + 间隔」判。这条给票 10 的 `trace_width` 定了来源。
- [轨迹搬走之后：主列、页签条与文档](issues/12-grilling-main-column-after-move.md) — **主列只剩
  对话视图**：`MainTab` / `main_tabs` / `MAIN_TABS_ROWS` / `draw_main_tab_bar` / `SwitchMainTab`
  整条线删掉，`CHROME` 6 → 6 由顶部页签条补回。**提示行改按「键盘在谁手里」分两套**：
  `KEY_HINTS` 变成 `ctrl-v 右栏` + `ctrl-r 平铺`（9 条），`TRACE_HINTS` 改名 `RAIL_HINTS`
  （`ctrl-v 交还`）。本图那个「屏上的一块」定名**格（`Tile`）**、当前那个叫**焦点格**
  （`_Avoid_`：pane / 窗格 = `render::pane::Pane`）。`CONTEXT.md` **新立五个**（右栏 / 顶部
  页签条 / 平铺 / 格 / 焦点格）、**改四个**（轨迹视图、对话视图、左栏、前台）。**立两条 ADR**
  （0023 平铺的进程模型、0024 轨迹的落点），herdr 不立。文档面改 `docs/render.md` 五处并新增
  「右栏」一节、`docs/tui-manual-checklist.md` 加一条真机项、`docs/lifecycle.md` 不动。
  `architecture-deepening` 那两条**都不做**。

## 尚未明确

<!-- 图已走完（2026-10-17）：fog 已全部消化，没有留下「还不够清晰到能成票」的视野。 -->

图收口时这里没有剩下的视野。四条旧 fog 的去处：

- 「平铺态的新会话 / 装载 / 关闭 / 翻页语义」→ 已由
  [平铺的手势与焦点](issues/07-grilling-pane-gestures-and-focus.md) 拍到「够 spec 动手」的那一档，
  剩下的句子归 spec。
- 「pty 回归的形状」→ 实现阶段的活，不是决策。
- 「平铺时的鼠标手势」→ 移进**范围之外**（排法定死，拖拽调边界不做）。
- 「团队视图与右栏容器的接缝」→ 移进**范围之外**（团队本体另开；本图只保证容器留得下）。

## 范围之外

<!-- scope 边界，不是路线的一步：封闭的结论，永远不 graduate。 -->

- **多进程分屏 / 把分屏交给 herdr、tmux、zellij 的 pane**：冻结项 2 选了同进程 —— 进程间无法共享
  宿主表与团队通信。见 [平铺的进程模型](issues/05-prototype-panes-layout.md) 所在那轮的 grilling。
- **herdr 的深层集成**：状态回传、`herdr` CLI / socket API、把 heng 会话变成 herdr pane、
  让 herdr 的侧栏认对 heng 的状态。冻结项 9 只做「检测 + 默认关左栏」。
  **2026-10-17 注记**：维护者随后要求实现「让 herdr 认得出 heng」，已在图外落地为
  `src/herdr.rs` 的自接入上报（见 [`docs/herdr.md`](../../docs/herdr.md)）；本图的 destination
  与其余范围不变。
- **`multi-role-view`（多角色分屏 / 分 tab）**：同一份流的多个发言人，另一张图。冻结项 10。
- **团队视图的本体（含右栏第二页、信箱与共享任务板）**：独立种子
  [`agent-teams`](../agent-teams/seed.md)；本图只负责它在屏幕上的承载（平铺与右栏容器）。
- **平铺态的拖拽与右键手势**：拖拽调整 pane 边界、双击标签、右键菜单。排法由冻结项 4 定死
  （1 / 2 / 3 / 4 的固定几何），没有可拖的边界；字符格里的 `DRAG_THRESHOLD` 仍只分
  「点击 vs 拖选」。
- **超过 4 个 pane**，以及为「更多 pane」做虚拟化：冻结项 3。
- **右栏后续页面的本体**（团队、任务板、用量面板……）：本图只定容器与轨迹这一页。
- **图片与附件面**：`image-input` 那条 seed 的事。

## 进度

**charting 完成于 2026-10-17**：三轮 grilling（十九问）定下目的地与十条冻结项，同日建出十二张票。
charting 当天并行派出三张 research 票的 subagent，**三张当天全部回收**：
[视图外壳与键盘：哪些件假定「全屏只有一份」](issues/02-research-view-shell-and-keys.md)（285 行）、
[宿主表与多会话基础设施：同屏多会话站在哪些件上](issues/01-research-session-infrastructure.md)
（310 行）、[herdr 的一手事实：它给 pane 里留了什么](issues/03-research-herdr.md)（556 行，每条带
来源 URL）；结论见「已定的决定」，其中 herdr 的宽度实况回头改了两张形态票的口径。
frontier 上现在只剩两张 **HITL 形态票**（[右栏里的轨迹](issues/04-prototype-right-rail-trace.md)、
[平铺的几何与两态布局](issues/05-prototype-panes-layout.md)），七张决策票各自等它们的事实或帧。

**同日沿图推进第一张：** [右栏 40 / 28 列里的轨迹长什么样](issues/04-prototype-right-rail-trace.md)
（编号最小的 frontier 票）跑出 12 帧、维护者判定够用，结论见「已定的决定」。它一收口，阻塞它的
[右栏的键盘与页面容器](issues/08-grilling-right-rail-keyboard.md) 也进了 frontier —— 现在 frontier
是五张：[平铺的几何与两态布局](issues/05-prototype-panes-layout.md)、
[全屏唯一的东西在平铺下怎么活](issues/06-grilling-single-instance-surfaces.md)、
[右栏的键盘与页面容器](issues/08-grilling-right-rail-keyboard.md)、
[每会话的视图状态与两态切换](issues/10-grilling-per-session-view-state.md)、
[上限、配置与 herdr 默认值的落点](issues/11-grilling-limits-config-and-herdr.md)。

**接着推进第二张：** [平铺的几何与两态布局](issues/05-prototype-panes-layout.md) 跑出 11 帧、
维护者判定够用，结论见「已定的决定」。它一收口，阻塞它的
[平铺的手势与焦点](issues/07-grilling-pane-gestures-and-focus.md) 与
[顶部页签条与左栏会话页的分工](issues/09-grilling-tab-strip-and-session-page.md) 同时解锁 ——
**两张形态票都收完了，七张决策票全部在 frontier 上**（只有
[轨迹搬走之后：主列、页签条与文档](issues/12-grilling-main-column-after-move.md) 还等
[右栏的键盘与页面容器](issues/08-grilling-right-rail-keyboard.md)）。

**第三张：** [全屏唯一的东西在平铺下怎么活](issues/06-grilling-single-instance-surfaces.md)
（编号最小的 frontier 票）两轮十问收口，结论见「已定的决定」；它把
`TuiState.pending` 从单例改成每会话一份，并把票 02 表里一句初判校订掉了。frontier 现在五张：
[平铺的手势与焦点](issues/07-grilling-pane-gestures-and-focus.md)、
[右栏的键盘与页面容器](issues/08-grilling-right-rail-keyboard.md)、
[顶部页签条与左栏会话页的分工](issues/09-grilling-tab-strip-and-session-page.md)、
[每会话的视图状态与两态切换](issues/10-grilling-per-session-view-state.md)、
[上限、配置与 herdr 默认值的落点](issues/11-grilling-limits-config-and-herdr.md)。

**第四张：** [平铺的手势与焦点：谁吃键盘、怎么切](issues/07-grilling-pane-gestures-and-focus.md)
（编号最小的 frontier 票）两轮十一问收口，三种手势是维护者自己的选择（`Ctrl-R` 开平铺、
`Ctrl-1…4` 跳格、`Ctrl-左右方向`切标签），并查出一条实现前提：`map_key` 今天忽略所有修饰键。
frontier 此时四张：[右栏的键盘与页面容器](issues/08-grilling-right-rail-keyboard.md)、
[顶部页签条与左栏会话页的分工](issues/09-grilling-tab-strip-and-session-page.md)、
[每会话的视图状态与两态切换](issues/10-grilling-per-session-view-state.md)、
[上限、配置与 herdr 默认值的落点](issues/11-grilling-limits-config-and-herdr.md)。

**第五张：** [右栏的键盘与页面容器](issues/08-grilling-right-rail-keyboard.md)（它一收口，
[轨迹搬走之后](issues/12-grilling-main-column-after-move.md) 也解锁了）三轮十三问，两种维护者的
选择（`Ctrl-V` 同时开栏并交键盘、平铺态右栏照开）把票 05 的判据改了一处（平铺态 pane 宽度下限
40 → 32，票 05 里已留校订）。frontier 现在四张，**七张决策票全部可达**：
[顶部页签条与左栏会话页的分工](issues/09-grilling-tab-strip-and-session-page.md)、
[每会话的视图状态与两态切换](issues/10-grilling-per-session-view-state.md)、
[上限、配置与 herdr 默认值的落点](issues/11-grilling-limits-config-and-herdr.md)、
[轨迹搬走之后：主列、页签条与文档](issues/12-grilling-main-column-after-move.md)。

**第六张：** [顶部页签条与左栏会话页的分工](issues/09-grilling-tab-strip-and-session-page.md)
（编号最小的 frontier 票）**一轮九问全按推荐**收口：顶部列宿主表、左栏列盘上会话，状态类信息
一个不上标签，平铺态自动收起左栏；顺手按代码校订了两条既有前提（未读从不在左栏会话页、左栏
单击不切前台），票 05 与票 07 的作答各留了一行校订。frontier 现在三张：
[每会话的视图状态与两态切换](issues/10-grilling-per-session-view-state.md)、
[上限、配置与 herdr 默认值的落点](issues/11-grilling-limits-config-and-herdr.md)、
[轨迹搬走之后：主列、页签条与文档](issues/12-grilling-main-column-after-move.md)。

**第七张：** [每会话的视图状态与两态切换](issues/10-grilling-per-session-view-state.md)
（编号最小的 frontier 票）**一轮八问全按推荐**收口，产出那张 25 行的归属表：`View` 升为每会话
一份、平铺集合不允许重复所以没有新的每 pane 档、会话属性一律每会话、轨迹宽度只剩右栏一个来源。
它顺带校订了票 01 表二的四条初判（每 pane → 每会话），研究产物里留了校订段。frontier 现在两张：
[上限、配置与 herdr 默认值的落点](issues/11-grilling-limits-config-and-herdr.md)、
[轨迹搬走之后：主列、页签条与文档](issues/12-grilling-main-column-after-move.md)。

**第八张：** [上限、配置与 herdr 默认值的落点](issues/11-grilling-limits-config-and-herdr.md)
（编号最小的 frontier 票）**一轮六问全按推荐**收口：`[ui]` 加 `tile_limit` 与 `sidebar` 两个键，
herdr 只认 `HERDR_ENV=1` 且只当左栏初值，右栏让位阈值（票 05 那条越界）落成可执行的算法。
frontier 只剩一张：[轨迹搬走之后：主列、页签条与文档](issues/12-grilling-main-column-after-move.md)。

**第九张（最后一票）：** [轨迹搬走之后：主列、页签条与文档](issues/12-grilling-main-column-after-move.md)
**一轮七问全按推荐**收口：主列页签条整条拆掉、提示行改成「键盘在谁手里」两套、屏上那一块定名
**格（`Tile`）**、立两条 ADR、文档面五处收口、`architecture-deepening` 两条不做。

**图走完（2026-10-17，12/12 resolved）。** 十二张票全部收口，fog 已清空（处置见「尚未明确」），
destination 达成：可以进 `/to-spec` 把它折成构建计划。
