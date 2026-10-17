# research：宿主表与多会话基础设施的底账

Part of: ../map.md
Ticket: ../issues/01-research-session-infrastructure.md
Status: resolved（本文件只报事实与清点，不选方案）

被测仓库 HEAD：`ad5aaf6`（`refactor(cli): cli.rs 拆成 module，一条子命令一个文件`）。行号以本文件
写就时的文件为准；引用处都带符号名，行号漂移时按名字重新定位。

## 问题

见 [图](../map.md) 与 [本票](../issues/01-research-session-infrastructure.md)：平铺是同进程多会话，
本票清点「今天一个进程里几个会话的设施里，哪些已经按会话分开、哪些是全进程一份」。

## 口径

只读调研，来源是仓库自身代码，引用一律 `路径:行号`。**票面把 `src/render/tui.rs` 写成 4000+ 行，
实测 `wc -l` 是 17964 行**，所以下面每一条都先用 `rg` 定位再读片段。

一处术语要在开头钉死：`src/render/pane.rs` 的 `Pane` 是**转录滚动缓冲**（`pane.rs:30`），一个会话里
有两个实例（对话与轨迹，`tui.rs:793` 与 `tui.rs:798`）；它**不是**屏上的一块 pane。本图说的「pane」
在今天的代码里没有任何对应类型，最接近的是 `layout::Regions` 那一份整屏几何（`layout.rs:180`）。

另有一处票面假设不成立：`src/render/tui.rs` 里**没有名为 `rebuild` 的函数**（`rg rebuild src/` 只命中两个
测试名 `tui.rs:15947`、`tui.rs:16192`）。切换与重建的实际路径是 `View::new` + `SessionLight` 收装 +
整条流重放，见下面第五节。

---

## 表一：已按会话分开的东西

「已按会话分开」= 同一个进程里两个会话各有一份，互不覆盖。括号里是它今天的**寿命**。

| 东西 | 落点 | 说明 |
|---|---|---|
| `SessionId` | `events.rs:76` | newtype，会话身份 |
| `Harness` | `lib.rs:150` | 一个会话的运行件：`session`、`provider`、`speaker`、`cancel`、`render` 句柄（`lib.rs:156-168`） |
| `Session` | `session/mod.rs:84` | 持有 `EventLog`、工具表、锁、policy、读集合、身份 |
| `SessionParts` | `session/mod.rs:53` | 组装输入；工具表与锁是**共享句柄**，不是每会话一份 |
| `EventLog` | `events.rs:1028` | 每会话一个 `log.jsonl`；克隆共享同一 writer / 缓存 / `next_seq`（`events.rs:1020-1026`） |
| `CancelSignal` | `agent/cancel.rs:24` | 每个会话一个（`host.rs:251`、`host.rs:312` 取自 harness），一次取消只停这个会话（`host.rs:138-139`） |
| 命令通道 `mpsc::Sender<C>` | `host.rs:247`，建成于 `host.rs:315` | 每会话一条，容量 **8** |
| 状态回传 `watch<SessionState>` | `host.rs:242-244`，建成于 `host.rs:313` | 每会话一对，容量 1（保最新） |
| `HostEntry` | `host.rs:237` | 表的一行：`id`/`cwd`/`state`/`cmd`/`read_only`/`cancel`/`join` |
| 会话的 `cwd` 与 `read_only` | `host.rs:240`、`host.rs:249` | 表行上的两格，按会话 |
| `FrontFacts` | `host.rs:58` | 前台会话的只读快照（模型、档位、模式、技能、目标） |
| `SessionAsker` | `asker.rs:19` | 每个会话包一层：「不是前台就不送请求」（`asker.rs:62-73`） |
| `RenderHandle` 的身份 | `render/mod.rs:201-206` | 每个会话拿到带自己 `SessionId` 的写端（`render/mod.rs:392-401`） |
| `RenderEvent` 的 `session` 字段 | `render/mod.rs:86-139`，取用处 `render/mod.rs:146-156` | 票 14 已落地：每一条都带会话身份 |
| `TuiState.states` | `tui.rs:2532` | `HashMap<SessionId, SessionState>`，切走切回都在 |
| `TuiState.light` / `SessionLight` | `tui.rs:2535`、`tui.rs:9640-9666` | 每会话一份轻状态：草稿、两个视口意图、浮层、折叠与轨迹索引 |
| `View` | `tui.rs:780` | 每会话的投影（转录、两个 `Pane`、timeline、宽度、选中块）——但**同一时刻只有一份活着**，见第五节 |
| `pane::Pane` 的两个实例 + `Viewport` | `tui.rs:793`、`tui.rs:798`、`pane.rs:68-77` | 视口意图按会话存取（`pane.rs:64-66`） |
| 宿主表的行集合 | `host.rs:258` | `Vec<HostEntry<C>>`，**无上限**——4 这个数今天不存在于这一层 |

## 表二：全屏 / 全进程唯一的东西

「全屏唯一」= 整个进程恰好一份。初判只有两种：**要改成每 pane 一份**，或**只需要一个焦点投影**
（即保留一份，但把「读哪个数据」改成读焦点 pane）。

> **2026-10-17 校订（票 10 的答案打上来的）**：下表里 `files: FileIndex`、`catalog`、
> `cwd` / `home`、`read_only` 四条初判的「要改成每 pane 一份」**改判为每会话一份** —— 它们是
> 会话的属性（索引的根是会话 cwd、名册在会话组装时推、只读档在宿主表行上），同一会话的多个 pane
> 共享。完整归属表与理由见
> [每会话的视图状态与两态切换](../../issues/10-grilling-per-session-view-state.md) 第 2 条。

| 全屏 / 全进程唯一的东西 | 落点 | 初判 |
|---|---|---|
| 渲染器本身（占终端与键盘） | `render/mod.rs:371-381`、`interactive.rs:289` | 只需要一个焦点投影（一个渲染器画 N 块 pane，不违反「渲染器只能起一次」） |
| 渲染通道 `broadcast`（容量 1024） | `render/mod.rs:75`、`render/mod.rs:339`、`render/mod.rs:372` | 只需要一个焦点投影（通道已带 `session`，缺的是消费端的路由） |
| 前台指针 `watch<SessionId>` | `host.rs:260`、`host.rs:290-297` | 只需要一个焦点投影（它就是「焦点 pane」的天然载体；见第六节） |
| `TuiState.front` | `tui.rs:2475`、`tui.rs:6993` | 只需要一个焦点投影 |
| `live_event` 的前台过滤 | `tui.rs:4440-4450` | **要改成每 pane 一份**（今天非前台事件直接 `return`，投影只有一份） |
| `ConsoleRequest::SwitchSession` | `input.rs:161-168`、`tui.rs:6987-7018` | 要改成每 pane 一份（今天「切会话 = 整块换投影 + 单份重放」） |
| `TuiState.cwd` / `home` | `tui.rs:2479-2480`、`tui.rs:3955-3963` | 要改成每 pane 一份；`SwitchSession` **不更新它**（`tui.rs:6987-7018`），终端标题（`tui.rs:4042-4055`）与文件索引的根（`tui.rs:500-519`、`tui.rs:7905`）今天都钉在启动那个会话的 cwd 上 |
| 终端标题与 `last_title` | `tui.rs:2483`、`tui.rs:4042-4055` | 只需要一个焦点投影（一条终端标题；读焦点会话 + 全局后台计数） |
| 退出举手槽（`exit_deadline`/`exit_gesture`/`exit_gesture_busy`） | `tui.rs:2490-2498`、`tui.rs:4114-4149` | 只需要一个焦点投影（一个进程一次退出，`Ctrl-D` 双击语义不变） |
| 退出闸门 `exit_gate` | `host.rs:211-225`、`interactive.rs:640-653` | 只需要一个焦点投影（它已读全表，与 pane 数无关） |
| 输入区 `editor` | `tui.rs:2506`、`tui.rs:4248-4283` | 只需要一个焦点投影——但**收装语义要重定**：今天「切会话 = 草稿跟着走」，冻结项 8 说输入区全局一份绑焦点 pane |
| 输入/手势通道 `ConsoleHandle` | `input.rs:299-301`、`input.rs:133-243` | 只需要一个焦点投影（输入归主循环，`interactive.rs:477-479`） |
| `TuiState.dirty` 与绘制循环 | `tui.rs:2504`、`tui.rs:8509` | 只需要一个焦点投影（一帧画全屏） |
| 整屏几何 `layout::plan` / `Regions` | `layout.rs:459-468`、`layout.rs:180-231` | 要改成每 pane 一份（今天是「主列 + 左栏」两块矩形，没有 pane 维度） |
| `files: FileIndex` | `tui.rs:2512`、`tui.rs:2515` | 要改成每 pane 一份（注释说它是会话级的值，但存在单例里、根取 `state.cwd()`） |
| `changes: ChangesPage` / `files_page` / `sessions: SessionsPage` | `tui.rs:2548`、`tui.rs:2543`、`tui.rs:2538` | 只需要一个焦点投影（左栏一栏一页；会话页本身是跨会话的） |
| 页签选择 `tab` / `main_tab` / `sidebar_wanted` / `sidebar_keyboard` / `trace_keyboard` | `tui.rs:2524`、`tui.rs:2563`、`tui.rs:2569`、`tui.rs:2560`、`tui.rs:2596` | 只需要一个焦点投影（前四个是「一块左栏、一块屏幕」的事实）；`trace_keyboard` 随轨迹搬进右栏，归右栏那一层 |
| 浮层：`detail` / `file_viewer` / `picker` / `notice` / `close_blocked` | `tui.rs:2605`、`tui.rs:2611`、`tui.rs:2575`、`tui.rs:2579`、`tui.rs:2585` | 只需要一个焦点投影（浮层全屏居中是既有决定） |
| `catalog: Vec<CatalogEntry>` | `tui.rs:2518` | 要改成每 pane 一份（技能名册是会话的，`interactive.rs:419-423` 推一次） |
| `background_waiting` / `background_finished` | `tui.rs:2528-2530`，由 `interactive.rs:578-588` 推 | 只需要一个焦点投影（本来就是「除焦点外还有几个」的全局读数） |
| `read_only` | `tui.rs:2526`、`interactive.rs:433-435` | 要改成每 pane 一份（今天它按进程一档；只读预览是**会话**的属性，`host.rs:249`） |
| `SessionHost` 表本身 | `host.rs:257` | 只需要一个焦点投影（表就是平铺的集合来源；上限 4 是装载策略，不在这层） |
| `PathLockTable` / `PathLocks` | `interactive.rs:288`、`tools/paths.rs:287-301` | 只需要一个焦点投影（**必须**全进程一份，否则写互斥按会话算，`session/mod.rs:60`） |
| `Config` / `McpService` / `SessionStore` / `GoalSetup` / `AssemblyKit` | `loading.rs:12-30`、`interactive.rs:303-322` | 只需要一个焦点投影（组装材料，进程级） |
| `Muted` | `input.rs:223-228`、`tui.rs:7092` | 只需要一个焦点投影（空宿主时禁言输入区） |
| `HostEntry.cancel` | `host.rs:253` | 每会话一份，但生产代码里**没有任何读取点**（取消走命令通道）；平铺后仍不需要它 |

---

## 一、宿主表本身

**形状。** `SessionState` 是三态 `Idle` / `RunningTurn` / `AwaitingHuman`（`host.rs:28-37`）。
`HostEntry` 是 8 格：`id`、`cwd`、`state`（`watch::Receiver`）、`state_tx`、`cmd`（`mpsc::Sender<C>`）、
`read_only`、`cancel`、`join`（`host.rs:237-254`）。表是 `Vec<HostEntry<C>>` 加一个
`watch::Sender<SessionId>` 的前台指针，另留一个接收端保住通道（`host.rs:257-267`，理由见
`host.rs:261-266`：`watch::Sender::send` 在没有接收者时失败，少了它 `switch` 会静默失效）。

**容量。** 宿主表**没有容量上限**——`adopt_with` 只查重（`host.rs:309-311`），不查数量。
三个通道的容量各有出处：命令 `mpsc::channel(8)`（`host.rs:315`）、状态 `watch` 容量 1
（`host.rs:313`）、渲染 `broadcast` 容量 `RENDER_CHANNEL_CAPACITY = 1024`（`render/mod.rs:75`）。
另有会话间的 unbounded 请求通道 `CliRequest`（`interactive.rs:444`）与 `ConsoleRequest`
（`input.rs:300`）。

**`adopt` / `adopt_read_only` / `adopt_with`。** `adopt` 是 `adopt_with(harness, cwd, false, TurnActor)`
（`host.rs:274-276`）；`adopt_read_only` 只把第三格置真（`host.rs:282-284`）。`adopt_with` 做四件事：
查重、取 `cancel`、建 `state` 的 `watch`、建 `cmd` 的 `mpsc`，然后 `tokio::spawn` 一个 task
（`host.rs:301-341`）。那个 task 的循环是「收一条命令 → 置 `RunningTurn` → 交给 actor → 置 `Idle`」
（`host.rs:316-329`），**三态的发送端归宿主，actor 只负责办**（`host.rs:319-324`）。

**`switch`。** 只写一次 `foreground.send(id.clone())`，**忙闲不阻挡**（`host.rs:353-358`）。
它不改表、不装载；「不在表里就先装载」在 `cli` 那一层（`loading.rs:572-578` 的 `SwitchOnly` 分支）。

**`close_check`。** 三条拒绝（`host.rs:175-191`）：不在表 → `NotHosted`；**前台且表里不止它一个**
→ `Foreground`；`RunningTurn` → `Running`。前台优先于忙闲，唯一的例外是「只剩它一个」
（`host.rs:182-186`）。真正的卸载是 `close_with`：从 `Vec` 里 `remove`、`send(shutdown).await`、
`join.await`（`host.rs:399-409`）；`close` 是缺省命令型的包装（`host.rs:417-419`）。
**不写盘、不删盘**（`host.rs:398`）。

**通道满了会怎样。** 三种命运各不相同：

* 命令 `mpsc(8)`：提交路径用 `send(...).await`（`interactive.rs:542-548`），**满则反压主循环**；
  `close_with` 也是 `send(...).await`（`host.rs:406`）。
* 手势路径用 `try_send`（`interactive.rs:1150`），**满即丢弃**——一次取消/模式循环在手势洪峰里会掉。
* 状态 `watch`：**保最新、不丢**，这是它被选中的理由（`host.rs:7-9`）。
* 渲染 `broadcast(1024)`：来不及消费就 `Lagged`，**静默丢**，TUI 只补画一条诊断
  （`tui.rs:574-579`、`tui.rs:7180-7187`）。

**一条硬不变量。** 同一个 `SessionId` 在表里只能有一行（`host.rs:12-13`），理由是同一路径两份
`EventLog` 会各自维护 `next_seq`、写出重复 `seq` 且互相不可见。落地处是 `attach::check` 的
`AlreadyHosted`（`attach.rs:100-103`）与 `attach_plan` 的 `SwitchOnly`（`loading.rs:750-753`）。

## 二、前台指针

**唯一的定义与订阅者。** 指针在 `host.rs:260`。`follow_foreground`（`host.rs:349-351`）**在生产代码里
没有任何调用点**（只有 `tests/session_host.rs:194`）；`subscribe_foreground`（`host.rs:381-383`）只有
一个调用点 `interactive.rs:293`，那一份被三处用：主循环自己留一份、喂给第一个会话的
`SessionAsker`（`interactive.rs:295-299`）、塞进 `AssemblyKit.foreground`（`interactive.rs:311`、
`loading.rs:24`），后者给每一个新装载的会话的 asker（`loading.rs:36-42`）。

所以前台指针的**真实订阅者只有一个**：`SessionAsker`，它拿 `wait_for` 把后台会话的问题挂住
（`asker.rs:62-73`）。

**渲染器拿到前台的路径不是订阅，是推。** 循环在装载/切换时调 `console.switch_session(...)`
（`loading.rs:254`、`loading.rs:303`、`loading.rs:625`），带的是那个会话的 `SessionFacts` 与事件流快照
（`input.rs:161-168`）；渲染器把 `self.front` 改成新的（`tui.rs:6993`）。渲染器**从不读**宿主表的
前台指针。切换前台因此只是「换一个命令的收件人 + 换一份投影」。

**`Ctrl-C` 找到「前台那个会话」的路。** `tui.rs:4130-4133`（`Key::CtrlC`）推 `FrontEndEvent::Cancel`
→ 主循环 `interactive.rs:659-661` → `forward_gesture(&host, ...)`（`interactive.rs:1147-1152`）
——它取 `host.get(&host.foreground())` 的 `cmd`，`try_send` 一条 `CliCommand::Gesture`
→ `CliActor::gesture` 命中 `FrontEndEvent::Cancel`（`interactive.rs:1227`）→
`harness.cancel_signal().cancel()`。信号是每会话一份（`host.rs:251`），所以这一路本身已经
「只取消前台」。**收件人由 `host.foreground()` 这一个值决定**。

**退出举手与终端标题读谁。** 举手槽是渲染器状态（`tui.rs:2490-2498`），`Ctrl-D` 走双击
（`tui.rs:4134-4149`），闸门在循环里（`interactive.rs:640-653`），判据是 `exit_gate`（`host.rs:211-225`，
「不在 `Idle` 的都算没收尾」）。终端标题是 `TuiState::title`（`tui.rs:4042-4055`），读四样：
`self.cwd`（启动那个会话的目录）、`background_waiting`/`background_finished`（循环每 80ms 推，
`interactive.rs:576-588`）、`replay`/`pending`、以及 `busy()`（`tui.rs:7527` =
`self.running || self.states.contains_key(&self.front)`）。

**做「焦点 pane」够不够。** 够一半。缺的是三件：其一，`front` 是**单值**，没有「平铺集合」
（表里有几行、屏上画几块）这个概念——今天 `Hosted`/`HostCount` 只把它当读数推给会话页
（`input.rs:177-183`）；其二，渲染投影只有一份（`tui.rs:4441-4443` 把非前台事件直接丢掉），
「焦点」今天等于「唯一被画的那个」，改成 pane 之后需要「N 份投影 + 一个焦点」；其三，
输入区的草稿随会话走（`SessionLight.editor`，`tui.rs:9642`），而冻结项 8 要它全局一份绑焦点——
这两条得二选一。

## 三、一个会话的运行件

**组装件。** `SessionParts`（`session/mod.rs:53-82`）是注入的：`log`、`config`、`tools`、`locks`、
`outputs_dir`、`policy`、`asker`、`questions`、`hook`、`home`、`skills`、`identity`。
其中 `tools` 与 `policy` 是 `Arc`、`locks` 是克隆即共享——**同一张路径锁表必须到达每个执行者**
（`session/mod.rs:60`）。`Harness`（`lib.rs:150-169`）持有 `opened`（脚手架）、`session`、
`provider`、`speaker`、`cancel`、`render`（会话那一份写端）、`channel`（进程那一份，只为收尾）。

**一次 `Turn` 到事件流。** `Harness::run_turn`（`lib.rs:721-724`）先记一条用户消息，再进
`drive_turn`（`lib.rs:730-745`）——那里 `cancel.reset()` 起一次运行的手势范围，然后
`agent::run_turn`（`agent.rs:574-581`）。写入只有一条路径：`append_event`（`agent.rs:2383-2395`）
做「打码 → `log.append` → `render.logged(&event)`」。`render.logged` 发出的就是带 `session` 的
`RenderEvent::Logged`（`render/mod.rs:232-237`）。

**`EventLog` / `next_seq` 的归属。** `EventLog` 是 `Arc<Mutex<Inner>>`，`Inner` 里同时装着 writer、
内存缓存与 `next_seq`（`events.rs:1027-1038`）。**同一个会话的多个克隆共享一个 `next_seq`**
（`events.rs:1020-1026`）；`seq` 就是行号（`events.rs:1126-1139`）。

**写同一个 `log.jsonl` 的并发约束。** 三条：进程内由宿主表不变量保证「一个 `SessionId` 只有一份
日志」（`host.rs:12-13`）；讨论者/执行者共用**同一个** `EventLog` 克隆，靠 `Mutex` 保证追加原子
（`events.rs:1020-1026`）；跨进程**没有任何文件锁**——`EventLog::open` 直接以 append 打开
（`events.rs:1072-1084`），`SessionStore` 里也没有 flock。所以「同一个会话被两个进程同时打开」
今天不被阻止，而 `attach::check` 只看**本进程**的宿主表（`attach.rs:100-103`）。

## 四、渲染通道的扇出

**事件带 `SessionId`——票 14 已落地。** 七种 `RenderEvent` 每一种都有 `session: SessionId`
（`render/mod.rs:86-139`），并由 `RenderEvent::session()` 统一取（`render/mod.rs:141-156`）。
发端是带身份的 `RenderHandle`（`render/mod.rs:201-206`），身份在 `RenderChannel::handle` 时定
（`render/mod.rs:392-401`）。

**订阅端只有一个。** `Render` trait 的三个实现（`render/mod.rs:282-284`：headless / plain / tui）
由 `Renderer::spawn` 起**恰好一个**（`render/mod.rs:313-333`），进程级的那一份由
`RenderChannel::start` 建（`render/mod.rs:370-381`），注释明说「一个会话从它这里取一个带自己身份的
`RenderHandle`，多个会话因此共用同一个渲染器与同一块屏幕」（`render/mod.rs:343-348`）。

**多份流同时活着时，画家靠什么知道画谁：靠 `self.front` 过滤。** `live_event`
（`tui.rs:4440-4450`）第一句就是 `if event.session() != &self.front { return; }`——**非前台的事件
进不了投影**，只用来更新状态读数（那还是主循环经 `ConsoleRequest` 推的，见
`interactive.rs:553-596`）。重放路径同样只有一份（`tui.rs:4463-4490`，`Replay` 里那份事件流就是
切换时收到的快照）。顺带一个细节：`RenderEvent::SessionState`（`render/mod.rs:128-134`）在 TUI 侧
只被用来取时刻（`tui.rs:11891`），真正的忙闲走 `ConsoleRequest::SessionState`
（`input.rs:201-205`、`tui.rs:7066-7073`）——**同一件事有两条通道，pane 化时要收成一条**。

## 五、视图状态的所有者

**`TuiState` 的字段分三类**（`tui.rs:2469` 起）：

* **每会话一份**：`states`（`tui.rs:2532`）、`light`（`tui.rs:2535`）、`sessions.hosted`
  （`tui.rs:9690`）。
* **整块换的投影**：`view: View`（`tui.rs:2471`）——`View` 自己持有 `facts`、`goal`、`mode`、
  `conversation: Pane`、`trace: Pane`、`live`、`painted`、`timeline`、两个宽度、`turn_rail`、
  选中块、`trace_links`（`tui.rs:780-900` 起）。
* **单例**：`front`、`cwd`/`home`、`last_title`、退出举手三格、`suspend`、`dirty`、`editor`、
  `files`、`catalog`、`slash`、`tab`、`read_only`、`main_tab`、`sidebar_wanted`、
  `sidebar_keyboard`、`trace_keyboard`、`picker`、`notice`、`close_blocked`、各种浮层矩形
  （`tui.rs:2469` 起，上面逐条给了行号）。

**切换搬了什么、丢了什么。** `stash_light`（`tui.rs:4248-4265`）用 `mem::take` 收走十三样
（草稿、两个视口意图、详情、查看器、选择器、`/` 菜单、轨迹搜索、折叠集、折叠把手、轨迹索引、
选中块），存进 `light[front]`；`load_light`（`tui.rs:4268-4283`）反向装回。`SwitchSession`
（`tui.rs:6987-7018`）的顺序是：收尾在飞的旧重放 → `stash_light` → 改 `front` → `View::new(facts)`
→ `load_light` → 清 `live_buffer` → 用快照开一次重放。

所以**丢的是整个投影**（`painted`、`timeline`、`turn_rail`、`colors`、`turn_totals`……），
靠重新铺一遍那条流恢复；**保的是**轻状态与 `states`。会话页那行 `▍` 是就地改的
（`tui.rs:7000-7002`，理由见 `tui.rs:6996-6999`）。宽度变化走另一条路
`rerender_if_width_changed`（`tui.rs:4313-4367`），只重放宽度真变了的那一个视口。

## 六、装载与关闭

**`attach` 的输入与产出。** 生产路径是 `loading.rs:520-637`：输入 `&mut SessionHost<CliCommand>`、
`&AssemblyKit`、`&ConsoleHandle`、`&UnboundedSender<CliRequest>`、`AttachTarget`
（`Existing(StoredSession)` 或 `New { cwd, inherit, rollover }`），产出 `Result<SessionId, ()>`
（`loading.rs:520-526`）。顺序是：归一化（读流、取 cwd、`attach::check`）→ `attach_plan`
（纯函数，`loading.rs:747-790`）→ 三选一：`SwitchOnly`（零延迟，`loading.rs:572-578`）、
`Refuse`（各回一句，`loading.rs:579-582`）、`Attach`（真装载，`loading.rs:583-635`）。
`Attach` 那支里的六步是：说通知 → 包 asker → `kit.load(...)` → 造 `CliActor` →
`host.adopt_with(...)` → 接 `state_tx`（`loading.rs:620-623`）。最后一定 `host.switch(&id)`
（`loading.rs:624`）——**装载天然改前台**。

**装载路径依赖的单例。** `AssemblyKit`（`loading.rs:12-30`）本身就是一张单例清单：`config: Arc<Config>`、
`model`、`speaker`、`store`、`locks: PathLocks`、`channel: RenderChannel`、`console_asker`、
`foreground`、`questions`、`mcp`、`home`、`mode`、`goals`。进程级那张路径锁表在
`interactive.rs:288` 建一次（`tools/paths.rs:287-301`，同 cwd 共享一把）。

票面问的「driver」在源码里查不到这个词（`rg -i driver src/` 无命中）：最接近的两样都是**每会话
一份**而不是单例——`Harness.provider`（`lib.rs:160`）与驱动那个会话的 `CliActor`
（`interactive.rs:1104-1118`，一个会话一个，由 `adopt_with` 交给它自己的 task）。

**一处债务要记账**：`loading.rs` 里有**两份同形的 `attach`**——`impl AssemblyKit` 内的一份
（`loading.rs:194-315`）与自由函数一份（`loading.rs:520-637`），`attach_plan`（`loading.rs:427` /
`loading.rs:747`）、`open_new`（`loading.rs:323` / `loading.rs:645`）与 `open_session`
（`loading.rs:472` / `loading.rs:792`）同样成对。
生产调用点用的是自由函数（`interactive.rs:601`、`interactive.rs:608`、`interactive.rs:620`），
所以 `impl` 里那一套是死代码。

**`close_check` / `exit_gate` 的判据**见第一节与第二节。关闭的编排在
`close_session`（`interactive.rs:1065-1094`）：被拒时前台那条走 `console.close_blocked`
（`interactive.rs:1075-1077`），其余进转录；成功后推 `session_closed` 与 `host_count`
（`interactive.rs:1089-1090`）。

**同屏多会话与「渲染器唯一」「一个进程一个终端」冲突吗：不冲突。** 事件已经带
`SessionId`（`render/mod.rs:86-139`），通道是广播、多写端少消费端（`render/mod.rs:343-348`），
而 TUI 已经在同一帧里同时持有「前台会话的投影 + 全部会话的状态读数」
（`tui.rs:2532` 与 `interactive.rs:553-596`）。冲突只在**投影只有一份**这一处（`tui.rs:4441`）与
**几何只有一份**这一处（`layout.rs:459`）。

## 七、可复用的先例：讨论与执行者

**它们不是「多份流」，是「一份流、多个发言者」。** 三条证据：

* 执行者：`ExecutorPort` 从父会话克隆 `log`、`session_id`、`locks`、`render`
  （`executor.rs:121-140`），其中 `session_id: session.id().clone()`（`executor.rs:126`）与父会话
  **同一个 `SessionId`**；它自带 `RenderHandle` 也是 `render.clone()`
  （`executor.rs:137`，来自 `executor.rs:93-96`）。区分靠 `SpeakerId::Executor`（`executor.rs:144`）。
* 讨论者：走 `Session::fork`（`session/mod.rs:298-316`），共享 `log`、`tools`、`locks`、`policy`、
  `asker`、`questions`、`hook`、`skills`，只换 `config` 与 `identity`，而 `id: self.id.clone()`
  （`session/mod.rs:301`）——**同一个会话身份**。
* 组装：`assemble_discussion` 只取**一个** `render.handle(scaffold.session_id.clone())`
  （`lib.rs:694`），给整场讨论用。

**权限委派链**是另一条不同的先例：讨论者**共享同一个** `Arc<Mutex<Policy>>`（`session/mod.rs:308`），
而执行者拿的是**新造的值**——`Policy::for_mode(...)` 加可传播规则（`executor.rs:107-111`、
`executor.rs:171`），「放行从不随之下行」（`executor.rs:98-101`）。

**能照搬什么。** 能搬的是组装法：共享句柄（日志之外的锁、工具表、asker、questions、hook）
以克隆注入、按 agent 分的东西新建——这正是第二个会话该复用的形状（`loading.rs:45-96` 已经在做）。
**不能搬的是分派机制**：它们靠 `SpeakerId` 在同一条流里区分说话人，而渲染器分派靠
`SessionId`（`tui.rs:4441`）；平铺要的是多个 `SessionId` 各自一份 `EventLog`，那正是执行者/讨论者
**刻意不做**的事。按图的冻结项 10，这是 `multi-role-view` 那一类，不是本图那一类。

---

## 查不到

* `src/render/tui.rs` 里没有 `rebuild` 函数（只有两个测试名引用它）。
* `HostEntry.cancel`（`host.rs:253`）在全仓没有生产读取点（`rg 'entry\.cancel' src/` 无命中）。
* 跨进程的会话独占锁：查不到（`src/session/store.rs` 无 flock；`EventLog::open` 直接 append）。
* 「平铺集合」「pane 数上限」「焦点 pane」这三个概念在今天的代码里都没有对应类型或字段。
