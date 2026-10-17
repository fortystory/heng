# 视图外壳与键盘：哪些件假定「全屏只有一份」

Type: research
Status: resolved
Part of: ../map.md
Blocked by: —

## 问题

[图](../map.md) 要把一屏从「一个会话」改成「最多四个 pane + 顶部页签条 + 右栏」。今天的外壳里有
一批东西是**全屏唯一**的：输入区、状态行、提示行、问卷、权限门、详情覆盖层、拖选、退出举手。
它们各自假定「只有一个会话在说话」，而冻结项 8 又要求输入区变成「底部全局一份、绑焦点 pane」、
右栏「全局一份、跟焦点会话」。这一票把那份清单与它们的归属改造点查清。

**这是只读调研，不改任何代码。** 结论一律给 `文件:行号`。

要清点六件事：

1. **键盘阶梯**。`TuiState::key` 那条自上而下的守卫阶梯有多少层、每层的判据是什么；
   `click_at` / `wheel_at` / `keyboard_in_the_input` 各自持哪一份字段子集。平铺要加「焦点 pane」
   这一层，它会插在第几层、与谁争。
2. **全屏唯一物的清单**。逐个给「它绑在哪个字段上」「它的几何从哪来」「它属于哪个会话」：
   输入区（草稿、`/` 与 `@` 浮层、记号菜单）、状态行、提示行（`KEY_HINTS` / `TRACE_HINTS`）、
   问卷、权限门、详情覆盖层、文件查看器、模型与强度选择器、拖选与 OSC 52、退出举手、`Ctrl-C`
   取消、（终端标题那一条归
   [每会话的视图状态与两态切换](10-grilling-per-session-view-state.md)）。
3. **几何与命中测试**。`layout.rs` 算出哪些分区矩形、宽度档怎么乘；`layout::Regions`（**17 个
   字段**）与 `HitAction`（**19 个变体**）覆盖哪些动作、哪些动作各自重算几何（`click_at` 里那几处）。
   多 pane 之后几何的输入（宽高）从哪来、今天是哪个字段当着「当前宽高」。
4. **单帧绘制流水**。一帧怎么合成（转录 → `pane` → 画家）：哪些步骤是单例（`CAP`、`painted`
   列表、跟随标记、时间轴缓存、轨迹的视口），它们能不能每 pane 各一份；`Pane` 的容量与
   `top_source` 的语义。
5. **键盘分层的先例**。`questionnaire-reading` / [ADR 0010](../../../docs/adr/0010-questionnaire-keys-dispatch-by-zone.md)
   的「键盘跟着当前那一层」怎么实现；轨迹账本那一层（`trace-ledger` 票 18）怎么声明「持有键盘」。
   平铺的焦点层照哪一条、`Esc` 的层序怎么排。
6. **已有的每会话状态**。`session-tab` 落地后哪些状态已经按会话存（`SessionLight` 的字段、
   `stash_light` / `load_light`）；哪些检查能直接复用成「每 pane 一份」的样板。

## 作答

**（2026-10-17，research subagent 完成，只读；产物 [`research/02-view-shell-and-keys.md`](../research/02-view-shell-and-keys.md)，285 行。）**

1. **键盘阶梯 = 17 个提前返回的守卫层 + 1 段兜底**（`src/render/tui.rs:7279`–`:7496`，兜底
   `:7497`–`:7509`），而代码注释自称「十六层阶梯」（`tui.rs:2593`）—— 两处不符，本票只报事实，
   统一归实现阶段。三套判据各持不同字段子集：`keyboard_in_the_input`（`tui.rs:14534`）只读 4 个
   字段，**漏了 `picker` 与 `replay`**，而这两者在 `key` 里都独占键盘（`tui.rs:7305` / `:7292`）。
2. **「全屏唯一」分两类**：**屏幕资源**（一块浮层、一个指针、一个进程、一把举手）本就只能一份；
   **每会话数据画在唯一矩形里**的是状态行（`tui.rs:9429`）、提示行（`tui.rs:8129`）、输入区
   （`tui.rs:2506`）—— 改造主战场是后者。逐件归属见产物 §2 那张表：输入区与 picker「全局一份、
   读焦点」（复用现成的 `stash_light` / `load_light`）；问卷与权限门「全局一份、绑**发起**会话」
   （reply 通道唯一，不能每 pane 一份）；详情覆盖层不缩进 pane（否则正文排版宽度全变）；
   拖选、OSC 52、退出举手**不动**。
3. **几何今天只有一个输入** `TuiState::area`（`tui.rs:2667`），点击链上另有 6 处拿它重跑
   `layout::plan`（`tui.rs:4995` / `:5085` / `:5199` / `:5864` / `:5982` / `:7741`）。多 pane 之后
   `layout::plan` 排的仍是「左栏 + 一列主列 + 底部」，**pane 的内接矩形要由一个新增的外层分栏供给**。
4. **焦点 pane 层插在第 7 层之前**（`sidebar_keyboard`，`tui.rs:7333`），与左栏页、轨迹页同族
   （都答「键盘在哪一块」）。但**最省的形态是这一层不持有键盘**：焦点只由页签条点击、鼠标与显式
   命令改，键盘层序一个字不动，输入区换个目标会话即可 —— 初判留给
   [平铺的手势与焦点](07-grilling-pane-gestures-and-focus.md) 收口。
5. **六个手势不带 `SessionId`**（`FrontEndEvent`，`src/render/input.rs:247`–`:282`，
   含 `Cancel`）—— 多 pane 之后 `Ctrl-C` 必须补身份（`CloseSession` / `OpenSession` /
   `SessionTitled` 三个是带的）。
6. **每 pane 一份的样板已经存在**：`SessionLight` 13 个字段的整份收发（`tui.rs:4248` / `:4268`，
   键是 `SessionId`）、`conversation` 与 `trace` 两个 `Viewport`（`pane.rs:68`）、以及按 `BlockId`
   记的过滤 / 折叠 / 选中（`tui.rs:880`–`:947`）—— 最后一类**尺寸无关**，pane 几何变了照旧有效。
7. **修正票面两处数字**（原写「18 变体」）：`layout::Regions` 是 **17 个字段**（`layout.rs:180`），
   `HitAction` 是 **19 个变体**（`tui.rs:3129`）。另：「拖拽调整 pane 边界」在代码里**没有任何
   先例**（`DRAG_THRESHOLD` 只分「点击 vs 拖选」，`tui.rs:3173`–`:3174`），归
   [平铺的几何与两态布局](05-prototype-panes-layout.md)。

**给下游的指针**：[全屏唯一的东西在平铺下怎么活](06-grilling-single-instance-surfaces.md) 用 §2
那张表；[每会话的视图状态与两态切换](10-grilling-per-session-view-state.md) 用 §6。

**2026-10-17 校订（票 06 的答案打上来的）**：§2 表里「问卷与权限门：全局一份、绑发起会话 —— reply
通道唯一，**不能**每 pane 一份」这句**只对 reply 成立，对容器不成立**。票 06 定下：平铺态允许
**接管期间切焦点、题面跟着焦点会话走**，于是 `TuiState.pending`（`tui.rs:2678`）要从**单例**改成
**每会话一份**（每份自带自己的 reply），全屏同一时刻只显示焦点会话的那一份。表里其余各行不受影响。

---

**产物**：`.scratch/session-panes/research/02-view-shell-and-keys.md`，含一张
「全屏唯一物 × 归属改造」的表：每一行给**它今天绑在哪**、**它属于哪个会话**、**改造的初判**
（每 pane 一份 / 全局一份读焦点 / 不动）。
