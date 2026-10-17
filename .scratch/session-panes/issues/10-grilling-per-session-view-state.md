# 每会话的视图状态与两态切换

Type: grilling
Status: resolved
Part of: ../map.md
Blocked by: 01

## 问题

同一个会话在标签态与平铺态之间来回切、切焦点、切标签之后，**哪些状态跟着会话走、哪些跟着 pane
走**。今天这些状态大多是单例（`TuiState` / `View` / `pane::Pane`），
[宿主表与多会话基础设施](01-research-session-infrastructure.md) 会给出「谁已经按会话存」的底账。
这一票逐条拍归属：

1. **清单**。转录的滚动与贴底跟随、轨迹的滚动 / 选中 / 过滤 / 折叠 / 搜索串 / 时间轴视口、对话与
   轨迹各自的 `pane`、输入区草稿与历史、`@` 文件索引、详情覆盖层开没开、未读计数、
   `SessionLight` 的字段 —— 一个不许漏。**另加两件票 06 定下来的**：`pending`（问卷 / 权限门，
   `tui.rs:2678` 要从单例改成**每会话一份**）与 `files: FileIndex`（票 01 表二判「要改成每 pane
   一份」，根取 `state.cwd()`）。
2. **归属**。每条给「每会话一份（切走再回来还在）」「每 pane 一份（平铺里两个 pane 看同一会话时
   不共享）」「全局一份」。今天谁重建（`rebuild`）、谁被 `stash_light` / `load_light` 搬。
3. **两态切换的保真度**。标签 ⇄ 平铺切换时滚动位置与选中要不要原样保留；宽度变了（主列与 pane 的
   内容宽不同）之后按什么重放 —— 照 `trace-ledger` 票 12 的「按行键替换任意一行」与宽度重放那条
   既有规则，还是另有取舍。
4. **轨迹搬家的后果**（冻结项 7）。轨迹的 `pane` 今天按主列宽度排版，搬进右栏后按右栏宽度排。
   同一会话在两个宽度之间来回切，谁的缓存作废、谁的选中活下来（`trace-ledger` 的「认块不认位置」
   能不能直接接上）。
5. **容量**。四个 pane 各持一份转录与轨迹的显示行缓存，`CAP = 20 000` 一档 —— 内存与重放代价要不要
   另定上限（比如只有焦点 pane 保持展开、其余懒算）。

## 作答

**（2026-10-17，一轮八问，维护者全部按推荐。第 2 条那张归属表是 `/to-spec` 直接要用的东西。）**

**一句话：`View` 从单例升为每会话一份，`SessionLight` 继续当轻状态；平铺集合不允许同一会话
重复，所以没有新的「每 pane」状态；会话属性（`cwd`、文件索引、`catalog`、`read_only`、`pending`）
一律每会话一份；两态切换走既有宽度重放；轨迹的宽度只有一个来源（右栏内宽）。**

### 1. 三档的定义

- **每会话**：切走再回来还在，键是 `SessionId`；**同一会话的多个 pane 共享**。
- **全局**：全屏 / 全进程一份。
- **每 pane**：本票结论是**没有新的每 pane 状态** —— 因为第 2 条禁止同一会话占两个 pane，
  `Viewport` 留在每会话就够了。

### 2. 归属表（逐条，一个不漏）

| # | 状态 | 今天的落点 | 归属 | 切换 / 重放时怎么处理 |
| --- | --- | --- | --- | --- |
| 1 | 转录滚动位置与贴底跟随 | `pane::Pane.top_source` / `follow`（`pane.rs:48-50`）+ `Viewport`（`pane.rs:68-77`） | 每会话 | 随 `SessionLight.conversation` 收发（`tui.rs:4249-4270`）；宽度变按 `top_source` 重定位，`follow` 照旧 |
| 2 | 轨迹滚动与贴底跟随 | `View.trace`（`tui.rs:798`）+ `SessionLight.trace`（`tui.rs:9646`） | 每会话 | 同上；**宽度只由右栏决定，两态切换不触发重放**（见第 4 条） |
| 3 | 轨迹选中 | `trace_selection`（`SessionLight`，`tui.rs:9665`） | 每会话 | 按 `BlockId` 记，宽度无关，原样保留 |
| 4 | 轨迹过滤 / 搜索串 | `trace_search`（`tui.rs:9656`）+ `trace_index`（`:9663`） | 每会话 | 同上，不随宽度变 |
| 5 | 轨迹折叠 | `folded` / `fold_before_all` / `fold_handles`（`tui.rs:9658-9661`） | 每会话 | 同上 |
| 6 | 轨迹时间轴视口 | `View.timeline`（`tui.rs:812`，宽度重放不重建 `:810`） | 每会话 | 随 `View` 走；宽度重放不复用判据之外的缓存 |
| 7 | 对话与轨迹两个 `Pane` | `View.conversation` / `View.trace`（`tui.rs:793` / `:798`） | 每会话 | 随 `View`；折行缓存在 `view()` 时按需算；`CAP = 20_000` 不动 |
| 8 | 输入区草稿 | `SessionLight.editor`（`tui.rs:9642`） | 每会话 | 已是，不动 |
| 9 | 输入区历史 | `Input.history`（`editor.rs:62-69`）随 `editor` 整份收发 | 每会话 | 已是，一行不动（`Ctrl-P` / `Ctrl-N` 翻） |
| 10 | `@` 文件索引 | `files: FileIndex`（`tui.rs:2512`），根 = `TuiState.cwd`（`:7905-7907`） | **每会话**（校订票 01 的「每 pane」） | 根 = 会话 cwd；切焦点时按新会话 cwd 重扫 |
| 11 | 详情覆盖层开没开 | `SessionLight.detail`（`tui.rs:9647`） | 每会话 | 已是，不动 |
| 12 | 详情的「打开方」 | `detail_opener`（`tui.rs:2620`）、`detail_rect`、`detail_serial` | **搬进 `SessionLight`** | 今天不进 `SessionLight`（`tui.rs:4249-4265`），切回旧会话再关可能按错档还原 —— 连同两格一起搬 |
| 13 | 未读计数 | 主循环那本账（`interactive.rs:559-569`） | 全局 | 不动；票 09 已定不上标签 |
| 14 | `SessionLight` 的 13 个字段 | `tui.rs:9640-9665` | 每会话 | 不动 |
| 15 | `pending`（问卷 / 权限门） | `TuiState.pending`（`tui.rs:2678`） | **每会话一份**（票 06 定） | 浮层矩形全局一份；全屏只画焦点会话那一份 |
| 16 | `catalog`（技能 / 命令名册） | `tui.rs:2518` | **每会话**（校订票 01 的「每 pane」） | 切焦点要重发（今天只在会话组装后发一次，`interactive.rs:419-423`） |
| 17 | `cwd` / `home` | `tui.rs:2479-2480` | **每会话**（校订表二的「每 pane」） | 切焦点跟着换；**顺带修掉 `SwitchSession` 不换 cwd 那个既有缺陷** |
| 18 | `read_only` | `tui.rs:2526` | **每会话**（校订表二的「每 pane」） | 从宿主表行取，随焦点换 |
| 19 | `View` 本体（`painted` / `block_at` / `trace_groups` / `turn_rail` / 两个宽度 / `colors` / `turn_totals`） | `tui.rs:2471`、`:780` 起 | **每会话**（`HashMap<SessionId, View>`） | 不再「切会话整块重造」；投影常驻 |
| 20 | 浮层本体（`detail` / `file_viewer` / `picker` / `notice` / `close_blocked`） | `tui.rs:2605`、`:2611`、`:2575`、`:2579`、`:2585` | 全局 | 整屏居中（票 06），不缩进 pane |
| 21 | `regions` / `screen_text` / `drag` | `tui.rs:2635` / `:2638` / `:2640` | 全局 | 指针与命中表是屏幕级的 |
| 22 | `tab` / `main_tab` / `sidebar_wanted` / `sidebar_keyboard` / `trace_keyboard` | `tui.rs:2524` / `:2563` / `:2569` / `:2560` / `:2596` | 全局 | 一块左栏、一块屏幕；**`main_tab` 随票 09 删主列页签条退场，处置归票 12** |
| 23 | 退出举手 / `background_waiting` / `background_finished` / 宽度档 / `pulse` | `tui.rs:2490-2498` / `:2528-2530` | 全局 | 一个进程一次退出；全局读数 |
| 24 | 平铺集合与两态开关 | **新值** | 全局 | **`Vec<SessionId>`**：有序、≤ 4、**不允许重复**；焦点＝宿主表前台（票 07） |
| 25 | 焦点 pane | `watch<SessionId>`（`host.rs:260`） | 全局 | 宿主表前台，票 07 已定 |

**不要用宿主表下标当集合成员**：下标会随关闭左移（`host.rs:400-405`），照抄会出别名。

### 3. 两态切换的保真度

**不新造规则，走既有的宽度重放**（`rerender_if_width_changed`，`tui.rs:4313-4367`）：

- 对话视图：宽度变了 → 按 `top_source` 保住顶端来源行，`follow` 贴底照旧；只清折行缓存重放
  那一侧（对话侧清 `conversation` / `conversation_flow` / `turn_rail`，轨迹侧走
  `clear_trace_for_replay()`）。
- 选中 / 过滤 / 折叠 / 搜索串：按 `BlockId` 记，**宽度无关，原样保留**。
- 标签 ⇄ 平铺就是一次宽度变化，不另设保真档。

### 4. 轨迹搬家后的宽度（消解票面第 4 条）

**轨迹只有一个家（右栏），不存在「两个宽度之间来回切」**：

- `trace_width` 的来源从 `conversation_width`（今天每帧赋值，`tui.rs:8554-8560`）改成**右栏内宽**；
  `trace_tier_width` 也改成右栏宽（不再是 `panes.main.width`），并**纳入重放判据**。
- **两态切换不重放轨迹** —— 右栏宽度由屏幕宽度档（40 / 28 / 隐藏）决定，与两态无关；只有屏幕
  宽度跨档才重放。
- `trace-ledger` 的「认块不认位置」直接接上：选中、过滤、折叠都按 `BlockId`，不需要额外的迁移。

### 5. 容量

**不做懒算。** `painted` 每会话常驻（它是增量记录，重放一次就回到增量更新）；`Pane` 的折行缓存
本来就在 `view()` 时按需算，非焦点 pane 不画就不算 —— 已经天然省掉一半。`CAP = 20_000` 不动。
实测真出内存问题再另开票。

### 6. 结构改动（`/to-spec` 要的落点）

1. `TuiState.view: View`（`tui.rs:2471`）→ `views: HashMap<SessionId, View>`；`SwitchSession`
   不再 `View::new` + 重放，改成「切焦点 + 该会话的 `View` 已在」。
2. `pending` 从单例改成每会话一份（票 06 已定，本票落进归属表）。
3. `cwd` / `home` / `files` / `catalog` / `read_only` 从单例改成每会话；`SwitchSession` 与
   `attach` 都要更新它们（今天一个字都不换）。
4. `detail_opener` / `detail_rect` / `detail_serial` 搬进 `SessionLight`。
5. 新增平铺集合 `Vec<SessionId>`（有序、≤ 4、去重）与两态开关。

### 移交

- `cwd` / `catalog` / 文件索引在「切焦点 / 装载 / 关闭」时的更新时序 → spec 阶段。
- `main_tab` 退场与 `docs/render.md` 的改写 → [轨迹搬走之后：主列、页签条与文档](12-grilling-main-column-after-move.md)。
- 四份 `View` 的实测内存 → 另开票（不在本图）。

**没动冻结项**：7 与 8 被落实（轨迹搬右栏、底部输入区全局一份绑焦点）。

---

**产物**：答案写进本票的 `## 作答`。第 2 条那张归属表是 `/to-spec` 直接要用的东西，要完整。
