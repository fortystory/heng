# 视图外壳与键盘：哪些件假定「全屏只有一份」

Type: research
Status: done
Part of: ../map.md
Blocked by: —

**来源**：本仓库自己的代码，引用一律 `路径:行号`。日期 2026-10-17。只读调研，未改任何文件。

本票回答 [图](../map.md) 的 [子票](../issues/02-research-view-shell-and-keys.md) 那六件事。冻结项 8
（底部全局一份输入区、绑焦点 pane；右栏全局一份、跟焦点会话）是判据，不是待议项。

## 摘要

1. **键盘阶梯不是一个数组，是一条手写的 `if` 长链**：`TuiState::key`（`src/render/tui.rs:7269`）
   在兜底那一段之前有 **17 个提前返回的守卫层**（`src/render/tui.rs:7279`–`src/render/tui.rs:7496`）。
   代码注释自称「十六层阶梯」（`src/render/tui.rs:2593`），与今天的逐块计数不符。
2. **三套指针/键盘判据各持一份不同的字段子集**：`keyboard_in_the_input`
   （`src/render/tui.rs:14534`）只读 4 个字段，且**漏了 `picker` 与 `replay`**，而 `key` 里这两者都
   独占键盘（`src/render/tui.rs:7305`、`src/render/tui.rs:7292`）。
3. **「全屏唯一」的其实是两类东西**：一类是**屏幕资源**（一块浮层、一个指针、一个进程、一把举手），
   本来就只能一份；另一类是**每会话的数据画在唯一的矩形里**（状态行 `src/render/tui.rs:9429`、
   提示行 `src/render/tui.rs:8129`、输入区 `src/render/tui.rs:2506`），它们的数据已经在 `View` 或
   `SessionLight` 里按会话存着，只是共享一块几何。第二类是这次改造的主要工作量。
4. **几何今天只有一个输入**：`TuiState::area`（`src/render/tui.rs:2667`，每帧由 `draw_frame` 写
   `src/render/tui.rs:8515`），点击路径上另有 6 处拿它重跑 `layout::plan`
   （`src/render/tui.rs:4995`、`:5085`、`:5199`、`:5864`、`:5982`、`:7741`）。多 pane 之后
   `layout::plan` 排的仍然是「左栏 + 一列主列 + 底部」，pane 的内接矩形得由一个新的外层分栏供给。
5. **每会话一份的样板已经存在**：`SessionLight`（`src/render/tui.rs:9640`，13 个字段）与
   `stash_light` / `load_light`（`src/render/tui.rs:4248` / `:4268`）。其中
   `conversation` / `trace` 两个 `Viewport`（`src/render/pane.rs:68`）正是「每个视图各一份视口意图」
   的现成做法，扩成每 pane 一份的代价最小。

## 1 键盘阶梯

### 1.1 层与判据

`TuiState::key`（`src/render/tui.rs:7269`）是一条自上而下的守卫链，每一层「认了就走」。
按代码顺序：

| # | 行号 | 判据 | 认了之后的去处 |
| --- | --- | --- | --- |
| 0 | `tui.rs:7270`–`:7275` | 无（入口副作用） | `dirty`、`note_attention_seen`（`:4108`）、`sync_tokens`（`:7881`） |
| 1 | `tui.rs:7279` | `key == CtrlZ` | `suspend = true`，终端层手势，拦一切 |
| 2 | `tui.rs:7285` | `key == Esc && drag.is_some()` | 取消一次还没落地的拖选 |
| 3 | `tui.rs:7292` | `replay.is_some()` | `replay_key`（`:4526`） |
| 4 | `tui.rs:7299` | `file_viewer.is_some() && viewer_key(key)` | nvim 浮层独占（`Ctrl-C` 是唯一出口，`:5133`） |
| 5 | `tui.rs:7305` | `picker.is_some()` | `picker_key`（`:6055`），其余可打印字符不落草稿 |
| 6 | `tui.rs:7313` | `detail_open()`（`:14096`） | 覆盖层的键（`Esc`/`Ctrl-D` 关、`Tab` 换面） |
| 7 | `tui.rs:7333` | `sidebar_keyboard && sidebar_key(key)` | 会话/文件/改动页（`:6095`） |
| 8 | `tui.rs:7349` | `main_tab == Trace && trace_keyboard && pending.is_none() && trace_key(key)` | 轨迹页**部分持有**（`:5218`，未收的键穿透） |
| 9 | `tui.rs:7364` | `key == CtrlT` | `ask_picker(Model)`（`:5997`） |
| 10 | `tui.rs:7369` | `key == CtrlO` | 左栏意愿翻转 |
| 11 | `tui.rs:7381` | `key == CtrlV` | `switch_main_tab`（`:5968`） |
| 12 | `tui.rs:7389` | 非 `Ctrl-D`、非问卷 `Esc` | `expire_exit_gesture`（副作用，不返回） |
| 13 | `tui.rs:7396` | `CtrlC` 或 `CtrlD` | `exit_key`（`:4128`） |
| 14 | `tui.rs:7400` | `key == Esc` | 全局 `Esc` 层序（下详） |
| 15 | `tui.rs:7439` | `pending.is_some()` | 问卷 `questionnaire_key`（`:7256`）或 `answer_key`（`:8062`） |
| 16 | `tui.rs:7455` | `muted && (Char|Enter|Tab)` | 输入区禁言（`goal-loop`） |
| 17 | `tui.rs:7458` | `key == BackTab` | 模式循环 `Mode::next` |
| 18 | `tui.rs:7470` | `token_menu().is_some()`（`:7916`） | `/` `@` 菜单吃 `↑`/`↓`/`Tab`/`Enter` |
| 兜底 | `tui.rs:7497`–`:7509` | 其余一切 | `Enter` 提交、翻页三键按当前页（`page_current`，`:4592`）、`editor_key`（`:4548`），末尾 `sync_menu`（`:8045`） |

第 12 层不是守卫（它不返回），这是上面数出 17 层提前返回的原因。文件重放那一层内部还有自己的
守卫（`replay_key` 的边界是它自己的，`tui.rs:4526`）。

`Esc` 的内部层序（`tui.rs:7400`–`:7438`）：问卷 `questionnaire_escape`（`:7405`）→ 忙时取消
`FrontEndEvent::Cancel`，禁言时 `GoalStop` 举手（`:7408`）→ 有 `pending` 则 `decline`（`:7423`）
→ 记号菜单 `slash.dismissed`（`:7429`）→ 多行草稿先问 `Pending::ClearDraft`（`:7433`）→ 清草稿。

### 1.2 三套指针判据各持哪份字段

- **`click_at`**（`src/render/tui.rs:4771`）读：`picker` + `picker.rect`（`:4774`、`picker_click` 在
  `:6014`）、`detail_open()` / `detail_rect`（`:4777`、`:4782`）、`regions.action_at`
  （`:4792`、`:4842`）、`questionnaire()` / `questionnaire_bottom` / `pending` + `modal_rect`
  （`:4806`–`:4818`）、`take_input_keyboard`（`:4824` → `:5982`）、`trace_rect`（`:4831`）、
  `take_sidebar_keyboard`（`:4840` → `tabs_rect` 与 `sidebar_page_rect`，`:5883`/`:7868`）、
  `files_page.rect` / `changes` / `view.todo`（`:4909`–`:4919`）、`screen_text`（在
  `press_at`/`release_at`，`:4697`/`:4719`）。
- **`wheel_at`**（`src/render/tui.rs:4639`）读：`detail_open()`（`:4640`）、`pending` +
  `questionnaire_bottom` / `modal_rect`（`:4644`–`:4656`）、`sidebar_page_rect` + `tab`
  （`:4667`–`:4676`）、`wheel_current`（`:4584`，按 `main_tab` 分派）。
- **`keyboard_in_the_input`**（`src/render/tui.rs:14534`）只读 4 个：
  `pending.is_none()`、`!sidebar_keyboard`、`detail.is_none()`、`file_viewer.is_none()`。

**子集不一致的两处实例**：`picker` 与 `replay` 立着时键盘归它们（`tui.rs:7305`、`:7292`），而
`keyboard_in_the_input` 对这两者全无所知，于是输入区光标照旧露面（`tui.rs:10948` 与
`tui.rs:8609` 都读它）。`tui.rs:15699`–`:15712` 的测试只覆盖了「中间的模态 / 详情 / 查看器」，
没有覆盖 picker 与 replay。

### 1.3 平铺的「焦点 pane」层插在哪

**初判**：插在第 7 层（`sidebar_keyboard`，`tui.rs:7333`）**之前**，与第 7、8 层同族 —— 这三者
答的是同一句话「键盘现在在屏幕的哪一块」。理由是 pane 在几何上包着主列，比左栏/主列更外层。

**与谁争**：若焦点 pane 也想用 `↑`/`↓`/`j`/`k` 切焦点，会与轨迹页那一层（
`tui.rs:5225`–`:5226` 已收编这四键）和左栏两页（`files_key`，`tui.rs:6406`–`:6407`）正面撞。
冻结项 8 把输入区定成「全局一份、绑焦点 pane」，所以**最省的形态是这一层不持有键盘**：
pane 焦点只由页签条点击、鼠标与显式命令改，键盘层序一个字不动，输入区换个目标会话即可。
这是初判，留给 [平铺的手势与焦点](../issues/07-grilling-pane-gestures-and-focus.md) 收口。

## 2 全屏唯一物 × 归属改造

| 物件 | 今天绑在哪（字段:行号） | 几何从哪来 | 属于哪个会话 | 改造初判 |
| --- | --- | --- | --- | --- |
| 输入区草稿 | `TuiState::editor`（`tui.rs:2506`）；每会话副本在 `SessionLight::editor`（`tui.rs:9642`），切换走 `stash_light`/`load_light`（`tui.rs:4250`/`:4270`） | `layout::content_width`（`layout.rs:443`）+ `Regions::input`（`layout.rs:210`、`layout.rs:534`）；行数由 `bottom_rows`（`tui.rs:8215`） | 每会话（`light` 表按 `SessionId`，`tui.rs:2535`） | **全局一份、读焦点**（冻结项 8）；换 pane 复用现成的 stash/load |
| `/` 与 `@` 浮层 | `TuiState::slash`（`tui.rs:2521`，`MenuSelection` `tui.rs:3253`）、`sync_menu`（`tui.rs:8045`）、`token_menu`（`tui.rs:7916`）；候选来自 `catalog`（`tui.rs:2518`）与 `files`（`tui.rs:2512`） | `Regions::menu` / `menu_width` / `menu_room`（`layout.rs:417`/`:404`/`:388`）；锚是 `draw_bottom` 回的光标（`tui.rs:11014`，接线在 `tui.rs:8574`–`:8579`） | 草稿那部分每会话（`SessionLight::slash` `tui.rs:9654`）；`catalog` 进程级 | **全局一份、读焦点**（跟着输入区走） |
| 记号菜单的键位 | 第 18 层（`tui.rs:7470`），`↑`/`↓`/`Tab`/`Enter` 四键 | 同上一行 | 同上一行 | **全局一份、读焦点** |
| 状态行 | 无自有字段：`draw_status`（`tui.rs:9429`）读 `view.facts.model`、`view.panel`（`tui.rs:854`）、`view.mode`、`effort_label`（`tui.rs:8183`） | `Regions::status`（`layout.rs:208`、`layout.rs:533`） | 每会话（全是 `View` 字段） | **全局一份、读焦点**；若要每 pane 各一条状态行，则几何也要每 pane 一份（冻结项 8 未表态，交给 [票 06](../issues/06-grilling-single-instance-surfaces.md)） |
| 提示行 `KEY_HINTS` / `TRACE_HINTS` | `hint_line`（`tui.rs:8129`）读 `main_tab`（`tui.rs:2563`）、`replay`（`tui.rs:2687`）、举手（`:8132`）、四份回执 `notice`/`opened`/`copied`/`close_blocked`（`tui.rs:2579`/`:2652`/`:2643`/`:2585`）；两套表在 `wording.rs:1658` / `wording.rs:1683`，阶梯在 `wording.rs:1823` | `Regions::hints`（`layout.rs:212`、`layout.rs:539`） | 进程级（`main_tab` 与举手都是进程级） | **全局一份**；文案要按焦点会话的 `busy()`（`tui.rs:7526`）与当前页取（初判） |
| 问卷 | `TuiState::pending`（`tui.rs:2678`）里的 `Pending::Questionnaire`（`tui.rs:2725`）；`Questionnaire`（`tui.rs:2744`）含唯一 reply（`tui.rs:2749`） | `layout::plan_questionnaire`（`layout.rs:468`）与 `questionnaire_input_rows`（`layout.rs:709`）；命中块 `questionnaire_bottom`（`tui.rs:2633`，每帧写 `tui.rs:8563`） | 发起它的那个会话（模型发起） | **全局一份、绑发起会话**；reply 通道唯一，**不能**每 pane 一份 |
| 权限门（循环的问题） | `Pending::Loop { request, reply }`（`tui.rs:2706`） | `Regions::modal` / `modal_sized`（`layout.rs:365`/`:371`）；命中块 `modal_rect`（`tui.rs:2630`） | 发起它的会话 | **全局一份、绑发起会话**（与问卷同一条） |
| 详情覆盖层 | `detail`（`tui.rs:2605`）、`detail_rect`（`tui.rs:2623`）、`detail_opener`（`tui.rs:2620`）、`detail_serial`（`tui.rs:2626`）；`DetailView`（`tui.rs:13595`） | `Regions::detail`（`layout.rs:321`）/ `overlay_area`（`layout.rs:343`）：在**整屏**居中，问卷立着时下移到 `overlay_floor`（`layout.rs:230`、`layout.rs:580`） | 打开它的那个会话（`opener` 记着关掉时归还给谁，`tui.rs:2618`） | **全局一份、绑发起会话**；浮层不缩进 pane，否则正文排版宽度全变 |
| 文件查看器 | `file_viewer`（`tui.rs:2611`）、`file_viewer_rect`（`tui.rs:2614`）；`FileViewerPane`（`tui.rs:13575`）持一条 pty | `viewer_width`（`layout.rs:330`）+ `overlay_area`（`layout.rs:343`），打开时定网格（`tui.rs:5085`–`:5103`） | 打开它的那个会话 | **不动**（进程唯一）；只把归属绑到发起会话 |
| 模型与强度选择器 | `picker`（`tui.rs:2575`）；`Picker`（`tui.rs:9557`）含唯一 reply（`tui.rs:9561`）；每会话副本 `SessionLight::picker`（`tui.rs:9652`） | `draw_picker`（`tui.rs:14548`）里 `modal_sized`（`tui.rs:14570`）；命中块 `picker.rect` / `picker.rows`（`tui.rs:9568`/`:9572`） | 每会话（`facts.switchable` / `fixed_effort`，`tui.rs:5998`/`:6004`） | **全局一份、绑焦点**（冻结项 8 的同类） |
| 拖选 | `drag`（`tui.rs:2640`）、`screen_text`（`tui.rs:2638`）；`ScreenText`/`Drag` 在 `selection.rs:65`/`:102`，每帧由 `note_rows`（`tui.rs:10703`）填 | 来自上一帧真画出来的行（`selection.rs:184` 复白画在最上层，`tui.rs:8594`） | 屏幕（指针只有一个） | **不动**：全局一份，指针语义天然唯一 |
| OSC 52 剪贴板 | `copied`（`tui.rs:2643`）、`clipboard`（`tui.rs:2648`）；算出在 `copy`（`tui.rs:4735`），写出在循环（`tui.rs:595`） | 无几何 | 屏幕 | **不动** |
| 退出举手 | `exit_deadline`（`tui.rs:2490`）、`exit_gesture`（`tui.rs:2492`）、`exit_gesture_busy`（`tui.rs:2498`）；出口文案在 `tui.rs:6387`/`wording.rs:1808` | 占提示行的出口段（`tui.rs:8132`） | 进程级 | **不动**（一次进程只退一次；槽位唯一是互斥的结构性保证） |
| `Ctrl-C` 取消 | `exit_key`（`tui.rs:4128`）→ `FrontEndEvent::Cancel`（`src/render/input.rs:249`） | 无 | 今天是「前台」（`tui.rs:2475`） | **要补身份**：`Cancel`/`Quit`/`CycleMode`/`NewSession`/`OpenPicker`/`AttentionSeen` 六个手势都不带 `SessionId`（`src/render/input.rs:247`–`:282`），只有 `CloseSession`/`OpenSession`/`SessionTitled` 带 |
| 左栏（会话 / 调用量 / todo / 文件 / 改动） | `tab`（`tui.rs:2524`）、`sessions`（`tui.rs:2538`）、`files_page`（`tui.rs:2543`）、`changes`（`tui.rs:2548`）、`sidebar_keyboard`（`tui.rs:2560`）、`tabs_rect`（`tui.rs:2554`）；四页枚举 `Tab`（`tui.rs:9875`） | `Regions::sidebar` / `tabs` / `sidebar_page` / `divide`（`layout.rs:214`–`:222`、`layout.rs:498`–`:575`） | 进程级（同一份左栏） | **不动**（冻结项 5 保留左栏会话页） |
| 主列页签条 | `main_tab`（`tui.rs:2563`）；`MainTab`（`tui.rs:9902`）；画在 `draw_main_tab_bar`（`tui.rs:9353`） | `Regions::main_tabs`（`layout.rs:188`、`layout.rs:517`）；占 `MAIN_TABS_ROWS = 2`（`layout.rs:53`） | 进程级 | **删掉**（冻结项 7），腾出的两行还给转录 |
| 右栏 | 今天不存在 | 无 | — | **新建**：全局一份、跟焦点会话（冻结项 6、8） |
| 终端标题 | `last_title`（`tui.rs:2483`）、`title`/`sync_title`（`tui.rs:4158`）、`set_terminal_title`（`tui.rs:648`） | 无（窗口属性） | 会话（按 cwd 与会话事实算） | 归 [票 10](../issues/10-grilling-per-session-view-state.md)，本票不判 |

## 3 几何与命中测试

### 3.1 `layout.rs` 排出的分区

`layout::Regions`（`layout.rs:180`）有 **17 个字段**（票面写「18 变体」，与实际差一个）：

`screen` `layout.rs:183`、`main` `:185`、`main_tabs` `:188`、`transcript` `:193`、`band` `:197`、
`band_kind` `:199`、`trace` `:204`、`rail` `:206`、`status` `:208`、`input` `:210`、`hints` `:212`、
`sidebar` `:214`、`tabs` `:216`、`sidebar_page` `:220`、`divide` `:222`、`sidebar_kind` `:224`、
`overlay_floor` `:230`。

派生矩形另算方法：`transcript_text` `:242`、`trace_text` `:252`、`scrollbar` `:262`、
`trace_scrollbar` `:273`、`detail_text_width` `:308`、`detail` `:321`、`overlay_area` `:343`、
`modal` `:365`、`modal_sized` `:371`、`menu_room` `:388`、`menu_width` `:404`、`menu` `:417`。

### 3.2 宽度档怎么乘

左栏档位由**宽度 × 意愿**相乘（`layout.rs:619` 的 `sidebar_tier`，注释在 `layout.rs:614`）：
`≥120` → 40 列（`SIDEBAR_WIDE`，`layout.rs:86`、`SIDEBAR_WIDE_FROM` `:90`）；
`≥80` → 28 列（`:87`、`:93`）；`<80` 或意愿为假 → 隐藏。主列宽 = 整屏减去「档宽 + 1 列分隔线」
（`main_width`，`layout.rs:636`）。转录行 = 高 − `CHROME`(6) − 输入行数（`layout.rs:34`、`:487`），
轨迹页再多扣 `BAND_ROWS`(3)（`layout.rs:38`、`:44`）。输入区行数在
`MIN_INPUT_ROWS`(3) 与 `max_input_rows`（`layout.rs:695`，封顶 `MAX_INPUT_ROWS = 10`）之间；
问卷走 `questionnaire_input_rows`（`layout.rs:709`）。

### 3.3 `HitAction`（票价说的 18 变体）

`HitAction`（`src/render/tui.rs:3129`）实际是 **19 个变体**：`Answer` `:3131`、`Paste` `:3133`、
`ClearDraft` `:3135`、`StopGoal` `:3137`、`Dismiss` `:3139`、`Previous` `:3141`、`Next` `:3143`、
`Submit` `:3145`、`SwitchTab` `:3147`、`SessionRow` `:3152`、`SessionNew` `:3154`、
`SessionClose` `:3156`、`CloseBlockedSwitch` `:3159`、`SwitchMainTab` `:3161`、
`TurnRailUnit` `:3163`、`SwitchModel` `:3165`、`SwitchEffort` `:3167`、`DetailFace` `:3169`、
`Fold` `:3175`。命中表本体是 `Regions`（`tui.rs:3191`，三个字段：`cells`/`options`/`custom`），
每帧 `clear`（`tui.rs:3207`）后由画家填。指针侧只读 `action_at`（`tui.rs:3228`）与
`option_at`/`custom_at`（`tui.rs:3215`/`:3223`）。

### 3.4 哪些动作各自重算几何

`click_at` 自己不排几何，但它的分派链里踩到 6 处用上一帧 `area` 重跑 `layout::plan` 的地方：

- `tui.rs:5982` `take_input_keyboard`（点回输入区）
- `tui.rs:5864` `open_trace_detail`（点轨迹行 / `Enter` 开详情）
- `tui.rs:4995` `open_todo_detail`
- `tui.rs:5085` `open_file_viewer`
- `tui.rs:5199`（覆盖层那条路上的正文宽度）
- `tui.rs:7741` `open_changes_detail`

另有 `tui.rs:8220`/`:8226` 用 `layout::content_width` 量草稿与问卷的宽度。

### 3.5 今天的「当前宽高」是哪个字段

`TuiState::area`（`src/render/tui.rs:2667`）。每帧由 `draw_frame` 从 `frame.area()` 写进
（`tui.rs:8510`、`tui.rs:8515`），绘制路径直接用 `frame.area()`；点击与滚轮路径读 `self.area`
（上面那 6 处）。所以**几何的输入是整屏**，没有「pane 的矩形」这一层概念。
多 pane 之后需要一个新的外层分栏把整屏切成最多四块，再把**每块矩形**喂给
`layout::plan` —— 今天 `plan` 的第一个参数是 `area`（`layout.rs:459`），它排的仍然是一条左栏
加一条主列加底部三件，pane 只是「主列」这一个角色。

## 4 单帧绘制流水

### 4.1 一帧的合成顺序

入口 `paint_frame`（`tui.rs:8604`，收光标可见性）→ `draw_frame`（`tui.rs:8509`）：

1. `settle_trace_fresh`（`:8513`）→ 写 `area`（`:8515`）→ 清单例：`regions.clear`（`:8518`）、
   `screen_text.clear`（`:8521`）、`modal_rect`/`questionnaire_bottom`/`trace_rect`/
   `conversation_rect`/`trace_indicator`/`indicator`（`:8524`–`:8532`）。
2. 太小的终端：关查看器、画 `too_small`、返回（`:8533`–`:8541`）。
3. 排版：`bottom_rows`（`:8546`）→ `layout::plan_questionnaire` 或 `layout::plan`（`:8547`–`:8551`）。
4. 两个视图宽度同源（`:8555`–`:8556`），写 `trace_tier_width`（`:8559`），
   `rerender_if_width_changed`（`:8560` → `:4313`：取走整份 `painted`、逐条 `emit_painted` 重推给
   `Targets { conversation, trace }`，`:4342`）。
5. 画：`draw_shell` `:8571` → `draw_transcript` `:8572` → `draw_status` `:8573` →
   `draw_bottom` `:8574`（返回光标锚点）→ `draw_menu` `:8578` → `draw_modal` `:8581` →
   `draw_picker` `:8585` → `draw_detail` `:8590` → `draw_file_viewer` `:8591` →
   `selection::paint` `:8594`（最后，只碰缓冲）。

**「转录 → pane → 画家」这一段的实际形态**：`draw_transcript`（`tui.rs:10319`）按 `main_tab`
分支，各自调 `Pane::view`（`pane.rs:272`）拿显示行，再 `Paragraph` 画进 `transcript_text()` /
`trace_text()`；画家从不自己推尺寸（`layout.rs:6`–`:8` 的纪律）。

### 4.2 哪些是单例，能不能每 pane 一份

| 步骤 | 今天在哪 | 是不是单例 | 能不能每 pane 一份 |
| --- | --- | --- | --- |
| `CAP` 容量 | `pane.rs:21`（常量 20 000 来源行） | 常量，但**每个 `Pane` 各持一份缓冲**（`pane.rs:32`） | 能，且已经是「每个视图一份」；代价是每份最多 20 000 来源行，四 pane 就是四份内存 |
| 折行缓存/起点表 | `pane.rs:34`–`:38` | 每 `Pane` 一份 | 能（同上） |
| 跟随标记 `follow` | `pane.rs:50`；`fresh` `pane.rs:440` | 每 `Pane` 一份 | 能，已在 `Viewport` 里随会话存（`pane.rs:74`） |
| `painted` 列表 | `View::painted`（`tui.rs:806`） | **每会话一份** | 会话级共享源；多 pane 各显示不同会话时应每会话一份（**不是**每 pane 一份） |
| 时间轴缓存 | `View::timeline`（`tui.rs:812`）+ `View::block_at`（`tui.rs:819`）+ `View::trace_groups`（`tui.rs:914`） | 每会话一份，宽度重放不重建（`:810`） | 随会话走；同一会话的多个 pane 共用一份 |
| 轨迹视口 | `View::trace`（`tui.rs:798`）；命中/链接的平行表 `trace_drawn` `:921`、`trace_links` `:894`、`trace_block_ids` `:903`、`anchors` `:909`、`folded` `:947`、`trace_index` `:941`、`trace_selection` `:888` | 每会话一份 | 随会话走；右栏轨迹页与主列轨迹页若同时存在，需要两份 `Viewport`（`pane.rs:68` 的形状已经支持） |
| `regions` / `screen_text` / `drag` | `tui.rs:2635`/`:2638`/`:2640` | **进程唯一** | 不能（指针与命中表是屏幕级的），只在别的 pane 覆盖到的地方相应清空 |

### 4.3 `Pane` 的容量与 `top_source` 语义

- 容量 `CAP = 20_000`（`pane.rs:21`）数的是**来源行**，不是显示行（`pane.rs:9`–`:11`）；超限时
  `evict`（`pane.rs:504`）丢最旧并**报出丢了几条**（`pane.rs:124`），这个返回数是平行表裁剪的
  唯一权威（`pane.rs:121`–`:123`）。
- `top_source`（`pane.rs:48`）记的是**视口顶端所在的来源行下标**，是宽度重排后活下来的那个坐标
  （`pane.rs:463`–`:484` 的 `ensure` 用它把 `top` 重新落到显示行上）；`sync_top_source`
  （`pane.rs:538`）从 `starts` 二分回来。它随 `Viewport`（`pane.rs:68`）在会话切换时收发
  （`tui.rs:4251`/`:4271`），所以「切换不丢阅读位置」靠的就是它。

## 5 键盘分层的先例

- **`questionnaire-reading` 的「键盘跟着当前那一层」**：`.scratch/questionnaire-reading/spec.md:30`
  ——没有浮层时归问卷，详情覆盖层与文件查看器立着时归浮层，**关掉之后原样还给问卷**（作答、
  焦点、翻到第几题都留着）。实现上就是守卫顺序：浮层那两层（`tui.rs:7299`、`:7313`）排在
  `pending`（`tui.rs:7439`）之前，提前返回即可，问卷的状态一个字节都不用改。
- **`trace-ledger` 票 18 怎么声明「持有键盘」**：`.scratch/trace-ledger/spec.md:291`–`:301`
  ——判据两半（当前显示页是轨迹 **且** 没交还）、位置在详情覆盖层之下与记号菜单之上、
  **焦点只有一个布尔状态**（`TuiState::trace_keyboard`，`tui.rs:2596`，默认持有，`Esc` 退完三层
  交还，点账本任意位置拿回，重新成为当前页自动回到持有）。它是**部分持有**的实现样板：
  `trace_key`（`tui.rs:5218`）只认表上那几个键，其余 `return false` 穿透（`tui.rs:5251`），
  所以打字、`Backspace`、翻页与所有 `Ctrl-*` 照旧落到输入区。
- **`Esc` 层序的三种排法**：问卷（`tui.rs:7404` 单层退出询问）→ 轨迹页（`trace_escape`
  `tui.rs:5298` 四层：搜索 → 过滤 → 选中 → 交还键盘）→ 左栏两页（`files_key` 的 `Esc`
  `tui.rs:6405` 只还键盘，一次手势一层）。`docs/render.md:607` 是轨迹页那四层的权威表述，
  `docs/render.md:578`–`585` 是左栏那一套。
- **平铺的焦点层照哪一条（初判）**：照 `sidebar_keyboard` 那条更近 —— 「点一下交给我、
  `Esc` 还回去」（`tui.rs:5883`/`:5931`），而不是轨迹页的「默认持有」。理由是默认持有键盘的
  东西今天只有输入区一个，而冻结项 8 要输入区保持全局一份；焦点 pane 若也默认持有，两者会
  同时主张 `↑`/`↓`。

## 6 已有的每会话状态

`SessionLight`（`src/render/tui.rs:9640`）今天有 **13 个字段**，全部经 `stash_light`
（`tui.rs:4248`）收、`load_light`（`tui.rs:4268`）装，键是 `SessionId`（表在 `tui.rs:2535`）：

`editor` `:9642`、`conversation`（`Viewport`）`:9644`、`trace`（`Viewport`）`:9646`、
`detail` `:9648`、`file_viewer` `:9650`、`picker` `:9652`、`slash` `:9654`、`trace_search` `:9656`、
`folded` `:9658`、`fold_before_all` `:9659`、`fold_handles` `:9661`、`trace_index` `:9663`、
`trace_selection` `:9665`。

进程级、**不在**这张表里的（`tui.rs:9637`–`:9638` 与 `.scratch/session-tab/spec.md:199`–`:202`）：
`tab`（左栏页）、`main_tab`（主列页）、`sidebar_wanted`、举手槽、`pulse`、宽度档、页内焦点、
`sessions` 页的过滤态。另有一张按会话索引的三态表 `states`（`tui.rs:2532`，键同为 `SessionId`），
`busy()` 就读它（`tui.rs:7526`）。

**可直接复用成「每 pane 一份」的样板**：

1. `Viewport`（`pane.rs:68`）＋ `restore_viewport`（`pane.rs:94`）—— 「只装意图、折行与行数是
   下一次 `view()` 的事」，天然支持同一个 `Pane` 在不同宽度下重定位。
2. `SessionLight` 的**整份收发**写法（`tui.rs:4248`/`:4268` 用 `mem::take`），以及
   `ConsoleRequest::SwitchSession`（`tui.rs:6987`）那条「整块换投影 + 先收后装」的路径。
3. `View` 里**每视图一份**的既有先例：`conversation` 与 `trace` 两个 `Pane`（`tui.rs:793`/`:798`）
   共用一份 `painted` 共享源（`tui.rs:806`、`Targets` 在 `tui.rs:4342`）—— 这正是「多 pane
   共享同一份会话记录、各自持有视口」所要的形状。
4. 尺寸无关的键索引纪律：轨迹页的过滤、折叠、选中都**按 `BlockId` 记**（`tui.rs:880`–`:947`），
   不存屏幕位置。pane 的几何一变，这些集合照旧有效 —— 多 pane 改造不必动它们。

## 与票面数字不符 / 未查到

- 票面说 `Regions` 有 **18 变体**：`layout::Regions`（`layout.rs:180`）是 **17 个字段**；
  `HitAction`（`tui.rs:3129`）是 **19 个变体**。两处都不是 18。
- 票面问「`TuiState::key` 那条阶梯有多少层」：代码注释自称「十六层阶梯」（`tui.rs:2593`），
  今天逐块数是 **17 个提前返回的守卫层 + 1 个兜底段**。注释与实际不一致，本票只报事实，
  不做统一（那是实现阶段的活）。
- 「拖拽调整 pane 边界」的几何：今天 `DRAG_THRESHOLD` 只分「点击 vs 拖选」
  （`tui.rs:3173`–`:3174` 的注），没有任何调整边界的代码。查不到可复用的先例，归
  [平铺的几何与两态布局](../issues/05-prototype-panes-layout.md)。
