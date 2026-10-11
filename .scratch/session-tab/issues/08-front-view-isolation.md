# grilling：前台视图的会话隔离与切换重建

Type: grilling
Status: resolved
Blocked by: 04
Part of: ../map.md

## 问题

把「切换前台时，屏幕上的东西怎么从 A 会话换成 B 会话、切回时又怎么回来」定死。这是票 04 在**渲染侧**的另一半。

**范围级决定已在 charting 冻死**（见 `../map.md` 的冻结项 6、7、8）：同进程多 Harness；切走的后台继续跑、不卸载；后台待答就地停住。本票不重开。

## 现状（charting 期已核实，别重查）

- 渲染器状态一大把且**都在同一个 `TuiState` 里**：`transcript`、`conversation` / `trace` 两个 `Pane`、`painted`、`timeline`、`block_at`、`turn_calls` / `turn_usage` / `turn_ordinal` / `turn_totals`、`panel`（数字从流上数出来）、`todo`、`turn_rail`、`trace_selection` / `trace_links` / `trace_block_ids`、`goal`。
- **没有全量重置入口**：`ConsoleRequest::Replay` 只**追加**（启动时那条一次性通道）；`Transcript` 无 clear；`painted` 只在**宽度重排**时从自己重放，不从事件流。
- 转录的来源是**事件流实时投影**（`self.transcript.push(event)`），不是"重放出来的块列表"。

## 需要定

1. **状态形状**：**每会话一份投影**（每场 host 各持一份 `TuiState` 的投影部分，前台只是把它画出来 / 拷进画布）还是**单份 + 切换时从事件流重建**。两条路的代价：内存（长会话的块与索引是几十 MB 级？）、切换延迟（重建要读多少事件）、以及哪条路在"后台继续跑"下更不容易错。给一个判断与理由，并说明被否的一条为什么否。
2. **重建入口**（若选单份）：按事件流全量重建要走哪条路 —— 复用 `apply` 一条条喂（照启动重播那条先例），还是在 `Transcript` / `Pane` 上加 reset。**不许绕过 `apply`**：`links` / `thinking_*` / `colors` / 命中索引都是它的副作用（`tui-history-replay` 已经踩过这个坑）。
3. **后台事件怎么留**：与票 04 §4 对接（别重复设计）。切回时怎么补齐到与流上一致（后台缓冲的事件 + 已重放的部分，怎么保证不重不漏、`seq` 怎么用）。
4. **切走 / 切回留住什么**：滚动位置、草稿（草稿是全局一份还是每会话一份 —— 这决定你在 A 里打了一半的字切到 B 再切回来还在不在）、页签选择与页内焦点（左栏那四签是进程级的还是会话级的）、详情覆盖层 / 问卷 / `ask_user` / `/` 菜单 / picker 这些**浮层**的归属（切走时先关掉还是留着？后台待答的问卷切过去要原样弹出）。
5. **与 `ConsoleRequest::Replay` 的关系**：复用同一条控制通道（加一个"换会话 + 重放"的变体），还是另开一条；启动时那条一次性重播要不要被切换路径吸收成同一个机制。
6. **不变量**：plain / headless 一行不动；渲染器状态仍然**不进事件流**；切会话不产生任何新事件（除非票 07 的新建/关闭路径）。

## 先读

- `../map.md`（冻结项 6、7、8）
- `04-session-host-and-concurrency.md`（**本票被它阻塞**，事件分流与它对接）
- `src/render/tui.rs` 的 `TuiState`（字段清单）、`apply`、`replay_batch` / `finish_replay`、`rerender_if_width_changed`、`click_at` 的浮层分派
- `src/render/transcript.rs`（`Transcript` 的 `new` / `push` / `flush`，**没有 clear**）
- `src/render/input.rs` 的 `ConsoleRequest::Replay` 与 `ConsoleHandle::replay`
- `src/lib.rs` 的渲染器 task（一进程一个）
- `.scratch/tui-history-replay/spec.md` §1/§2（一次性重播的接缝与代价）

## 答案落点

契约级：状态形状选择（含被否方案与理由）+ 重建入口 + 后台补齐规则 + 切走切回的保留清单 + 与 Replay 通道的关系 + 不变量。不要写实现代码。

## 作答

2026-10-12 定完：两轮 grilling（5 + 4 问），另有两份只读代码调研（`TuiState` 的 98 个字段与
归类；重放接缝与真实量级）。以下契约**自足**；范围级决定（冻结项 6、7、8）不在本票重开。

### §1 状态形状：单份投影 + 切回全量重建

**选定**：渲染器只持**一份**会话投影；切回时按目标会话的**事件流快照**全量重建。被否的两条：

- **每会话一份活投影**：与票 04 §4 直接冲突 —— 后台只送状态转换、不送 `Delta` / `Logged`，
  所以那份投影本身**不会更新**，切回仍要增量补齐；内存还要 ×N，而 `trace_index` / `painted` /
  `block_at` 三张账**覆盖整场、不受 `CAP` 裁**（`src/render/tui.rs:2305-2308`、`:825`、`:838`；
  `CAP = 20_000` 只裁 pane 的源行，`src/render/pane.rs:21`）。
- **单份 + LRU 缓存**：多一层失效规则（宽度变化、后台新事件、页内改名），且与票 04 §4 的边界
  变模糊。

**量级（真实 store 实测）**：281 场 / 74,803 事件；单场中位数 **9**、p25 5、p75 36、p90 623、
最大 **14,544**。中位数那批切回是瞬时的；长尾那几场按启动重放的实测密度（291 事件 → release
`apply` 89.4 ms，`.scratch/tui-history-replay/research/02-replay-seam-and-cost.md:169-199`）
外推是百毫秒到秒级。**实现期必须实测一场 14,544 事件的真实切换延迟**；若不可接受再回头加
缓存，本票不为它预先优化。

### §2 重建入口：投影收成一个子结构，切换时整块换

`TuiState` 现在 **98 个字段**（`src/render/tui.rs:772-1152`）：会话投影 32 / 会话级轻状态 42 /
进程级 18 / 归不准 6。**投影那 32 个字段里有 10 个完全没有清空入口**：`transcript`（`Transcript`
无 `clear`，`src/render/transcript.rs` 只有 `new` / `push` / `flush`）、`painted`（宽度重放走
`mem::take` 再原样放回，内容保留，`src/render/tui.rs:3108`、`:3132`）、`timeline`（轴域只增不减）、
`block_at`（以 `BlockId` 为键的 `HashMap`）、`panel`、`todo`、`colors`、`trace_index`、
`next_local_id`、`turn_totals`。

**契约**：

1. 把「会话投影」收成一个**独立子结构**（名字实现期定），形状 = `TuiState::new` 构造清单
   （`src/render/tui.rs:2389-2445`）里的投影部分，自带 `new(facts)` 与 `apply(event)`。
2. **切换 = `self.view = View::new(facts)` 整块换**，不写 reset 清单 —— 那 10 个字段现在就没有
   清空逻辑，靠人工清单必漏（`timeline` 与 `block_at` 最危险：轴域只增不减、块身份按 `BlockId` 存）。
3. **进程级字段留在 `TuiState` 本体、不参与切换**：`tab` / `main_tab` / `sidebar_wanted` /
   举手槽 / 脉冲 / 宽度档 / `area` / `dirty` / `quit`…
4. **`facts` 与 `SpeakerColors` 必须整份换**：`SessionFacts` 在 `TuiState::new` 注入、`apply` 不碰
   （`src/render/tui.rs:2371-2445`，配色 `:2376`）。否则切到 B 之后状态行还写着 A 的 model /
   mode / context。
5. **会话级轻状态另存一张表**（键 = `SessionId`）：草稿（`editor`）、两个 `Pane` 的视口意图
   （`follow` / `holding` / `top_source`）、转录滚动位置、浮层（`detail` / `file_viewer` /
   `picker` / `slash` / `trace_search` / `folded`）。切换 = 先收走当前那份、再装入目标那份。
   关闭会话时删那一行；宿主表无上限（票 04 §10），这张表也不另设容量规则。

### §3 切走 / 切回留住什么

| 东西 | 归属 | 依据 |
|---|---|---|
| 草稿（`editor`） | **每会话一份** | 切回来字还在；全局一份会让切到 B 时输入框里是 A 的未发送文本 |
| 转录滚动位置 + 「跟随末尾」状态 | **每会话一份** | `Pane::clear` 刻意保留视口意图（`src/render/pane.rs:179-187`），正是这份语义 |
| 详情覆盖层 / `/` 菜单 / picker / 文件查看器 / 轨迹搜索与折叠 | **每会话一份，切走留着、切回原样** | 它们都是跟会话走的视图状态 |
| 问卷 / `ask_user` | **不进这张表** | 票 04 §6：后台挂住，切过去原样弹出 |
| 左栏页签选择与页内焦点 | **进程级**（切走切回仍在同一页） | 票 04 §8 |
| 会话页的焦点行 | 按**会话 id** 保位 | 票 06 §8 |
| 终端尺寸 / 宽度档 / `Ctrl-O` 意愿 | **进程级** | 票 04 §8 |

### §4 重建走哪条通道

**复用 `ConsoleRequest`，加一个「换会话 + 重建」变体**；启动那条 `Replay`
（`src/render/input.rs:185`）**被收编成同一个机制的特例**（首帧没有旧会话可收尾）。

- 变体形状（契约级）：`SwitchSession { facts: SessionFacts, events: Vec<Event> }`。`facts` 是目标
  会话的会话事实（票 05 的形状，`facts.session_id` 就是新的前台）；`events` 是**请求那一刻**的
  流快照（`EventLog::events()` 的全量深拷贝，`src/events.rs:1047-1049`）。
- **启动门闩要松掉**：`TuiOptions.reopened` 只在 `Tui::run` 开头消费一次
  （`src/render/tui.rs:472-480`），它的作用被这个变体吸收。
- 通道够用：console 请求是**无界 mpsc**（`src/render/input.rs:339`），不经 broadcast 的 1024
  容量；`replay_batch` 用 `take()` + 放回，幂等可重复（`src/render/tui.rs:3460-3484`）。
- **在飞的旧重建先收尾**：收到新变体时先插分隔行、排空 `live_buffer`，再开新的 —— 现状是直接
  覆盖（`src/render/tui.rs:6423-6435`），旧批次永不 `finish_replay`，它缓冲的实时事件会泄到
  下一次。
- **重建期间的画面**：复用重放视觉（逐批 512 事件 / 2000 源行，`src/render/tui.rs:101`、`:105`），
  末尾 `to_bottom()`；**不插**启动那条「以下是历史」分隔行（B 的整场都是重建，不是历史）。

### §5 后台补齐规则（不重不漏）

1. **快照边界**：重建用 `events` 快照，取其中最大 `seq` 记作 `S`（`seq` 在 `apply` 里已被读，
   `src/render/tui.rs:2695-2698`）。
2. **重建期间**：目标会话新进来的 `Logged` **全部进 `live_buffer`** —— 要改现状：`live_event`
   现在在重放时**丢弃一切 `Logged`**（`src/render/tui.rs:3438-3447`），那会让重建期间的内容
   静默消失。
3. **收尾**：`finish_replay` 排空 `live_buffer` 时**只 apply `seq > S`** 的那些（快照已含 ≤ S）
   —— 于是不重（不会重复画快照里已有的块）不漏（重建期间来的都在 buffer 里）。
4. **id 过滤**：`RenderEvent` 按票 04 §4 打上 session id 之后，渲染器**只 apply「当前前台」的
   `Logged`**；后台会话的事件只用来更新会话页的状态标记与读数（票 06 §5）。
5. **切回瞬间那个窗口**由 2 + 3 覆盖：前台指针已改、`SwitchSession` 还没到的那几帧，事件进
   `live_buffer`，重建收尾时按 `seq` 补上。

### §6 触发点（票 07 §8 交来的）

| 触发 | 渲染侧动作 |
|---|---|
| 点击会话页任一行 / 新建后切过去 | 一条 `SwitchSession`：收走当前轻状态 → 装入目标的（没有就新建空的）→ 整块换投影 → 重放目标的事件快照 |
| 关闭**非前台**会话 | 前台不动；只重画会话页（票 06），并删掉轻状态表里那一行 |
| 关闭前台会话 | **禁止**（票 07 §3），没有渲染侧动作 |
| 空宿主态（关掉最后一场） | 前台置空：投影清空、主列不画内容、输入区禁用（`没有会话，先新建一场`），会话页页顶 `0 场` + `新建`（票 07 §5） |
| 目标会话已在宿主表里 | 同第一条（取事件快照即可，**绝不重走 `assemble`** —— 票 07 §7 的不变量） |

**谁发这条请求**：主循环监听宿主表的前台指针（票 04 §2 的全进程 `watch<SessionId>`），变化时
把「目标会话的 facts + 该会话的流快照」装进变体发给唯一的渲染器。

### §7 不变量

1. **plain / headless 一行不动**：`ConsoleRequest::Replay` 在 plain 前端被无视
   （`src/render/input.rs:492-494`），新变体照此；headless 那条路径根本不建 console。
2. **渲染器状态不进事件流**：本票新增的只有渲染器内部结构，事件流一个字节不动。
3. **切会话不产生任何事件**（新建 / 关闭那条路径除外 —— 那是票 07 的动作）。
4. **重建必须走 `apply`**：`links` / `thinking_*` / `colors` / 命中索引 / `painted` / `block_at` /
   `timeline` / `panel` / `todo` / `turn_*` 都是它的副作用（`tui-history-replay` 踩过的坑）。
   「整块换 + 重放」正是为了不绕过它。
5. `messages` 投影前缀与模型可见文本一个字节不动。

### §8 要改的既有行为（实现期落点）

- `ConsoleRequest` 加 `SwitchSession` 变体（`src/render/input.rs:133`），`TuiOptions.reopened`
  门闩被吸收（`src/render/tui.rs:472-480`）。
- `live_event` 在重放期间不再丢弃 `Logged`，改为压进 `live_buffer`（`src/render/tui.rs:3438-3447`）。
- 收到新重放请求时先收尾旧的（现为直接覆盖，`src/render/tui.rs:6423-6435`）。
- `TuiState` 的投影部分抽成子结构（`src/render/tui.rs:772-1152` 那 98 个字段里挑 32 个）。
- `RenderEvent` 打 session id（票 04 §4 定的，本票是它唯一的消费方，`src/render/mod.rs:86-121`）。

**交出去**：`docs/render.md` 的「一个进程一个渲染器」那句与 `docs/lifecycle.md` 的图 5 →
[票 09](09-doc-revision.md)。

**产物**：本票无独立资产；两份调研的事实已折进 §1 / §2 / §4。

**判据复核**：只产决策，无实现代码；模型可见文本与 `messages` 投影前缀一个字节不动；
plain / headless 渲染器不受影响。
