# grilling：多会话宿主与后台并发

Type: grilling
Status: resolved
Blocked by: —
Part of: ../map.md

## 问题

把「一个进程里同时活着多场会话，切走的照跑」定到可实现级 —— 这是本图最重的一张票，也是「切走不卸载」的全部成本所在。

**范围级决定已在 charting 冻死**（见 `../map.md` 的冻结项 6、7、8）：宿主 = **同进程多 Harness**（不引入 IPC、不做独立进程 daemon）；切走的会话**后台继续跑回合**且**不卸载**；后台会话跑出需人答的东西就**停在那一步**并标记。本票只定「怎么装」。

## 现状（charting 期已核实，别重查）

一进程一会话是硬编码的：`Harness` 绑 cwd、沙箱探测（`OnceLock`）、权限策略、技能、`agents_md`、工具表（`src/lib.rs:207-228`）；渲染器一进程一个、绝不重建（`src/lib.rs:156-157`）；`ConsoleRequest` 是循环 ↔ 前端的那条通道。`Harness::rollover` 已经是"同进程、同渲染器、只换 log 与会话身份"，但只能 `EventLog::create`（开**新**会话），且不换 cwd / 沙箱 / 技能 / 权限策略。

## 需要定

1. **宿主与所有权**：会话表由谁持有（CLI 组装点的一个新类型？`Harness` 之上的一层？），每场 host 持什么、生命周期怎么表达；**一次会话一份**的东西列全（cwd、沙箱探测、权限策略、技能与 `agents_md`、工具表、`EventLog`、投影 / `Agent` 状态）。给一个新词（暂定**会话宿主**）写进 `CONTEXT.md` —— 或论证不需要新词。
2. **前台的切换动作**：点击到切换完成之间的序列（检查空闲 → 装载/取回 host → 换前台 → 视图重建交给票 08）；忙时点击给什么回执；目标 host 已在内存 vs 首次从磁盘装载（票 05 的装载路径）两条分支。
3. **后台回合怎么被推进**：现在是单会话的一条 `select!` 主循环。多个 host 各自驱动自己的 future，还是共用一个驱动循环？谁 poll 谁、前台切换如何不打断后台（tokio task 归属、取消语义、`SessionCancel` 一类的既有机制）。
4. **事件分流**：渲染通道是进程级一条。后台会话的事件去哪（丢弃 / 按会话缓冲 / 只记计数）？前台只画前台的事件；**切回时怎么补齐**（与票 08 对接，别在两票里各设计一半）。
5. **并发预算**：默认允许多少场同时跑回合；与厂商配额窗口（`src/session/ledger.rs` 的跨工作区日账本）、`budget_limit`（goal-loop 的额度是**跨会话**的，见 `CONTEXT.md` 的**累计 token 上限**）怎么交互；撞顶时哪场降级收尾。
6. **需人答的暂停**（冻结项 8 的落地）：权限询问 / 问卷 / `ask_user` 在后台时的挂起点在哪、标记怎么送到列表与提示行、切过去后原样弹出的去向（`ConsoleRequest` 的哪条通道）。
7. **中断与退出**：`Ctrl-C`（取消 / 退出 / 重播打断）在"前台忙、后台也忙"时归谁；`Ctrl-D` 退出确认；**退出时后台会话怎么写收尾**（不写 / 写 `SessionEnded` / 提示有几场在跑）；进程被杀时多场会话的恢复语义（既有 `recover_pending_calls` 是单会话的）。
8. **进程级手势与会话级手势**：`Ctrl-O`（左栏）、`/model`、`/effort`、`Shift+Tab`（模式）落前台还是会话各自；`/clear`（既有 `rollover`）与票 07 的"新建会话"是什么关系。
9. **工具副作用**：两场会话同 cwd 时并行写文件的互斥（既有 `PathLocks` 的粒度）；不同 cwd（各 worktree）时沙箱与权限策略各归各的确认。
10. **上限与卸载**：宿主里最多留几场（fog 里有这条，但至少要给默认值：无上限 + 手动关闭？）；关闭的语义在票 07。

## 先读

- `../map.md`（冻结项 6、7、8、11）
- `src/lib.rs` 的 `OpenedSession` / `assemble` / `rollover` / 沙箱探测 / `recover_pending_calls`
- `src/cli.rs:251-451`（interactive 的组装与主循环）与 `choose_session`
- `src/render/mod.rs` 的 `channel` / `RenderHandle`；`src/render/input.rs` 的 `ConsoleRequest`
- `.scratch/goal-loop/spec.md`（`rollover`、额度跨会话、翻页）
- `.scratch/tui-history-replay/spec.md` §1/§2（`ConsoleRequest::Replay` 这条一次性通道的先例）
- `08-front-view-isolation.md`（视图侧的分工，别在两票里重复设计）

## 答案落点

契约级：宿主形状与所有权 + 切换序列 + 后台推进与事件分流 + 预算规则 + 暂停/中断/退出语义表 + 上限。不要写实现代码。

## 作答

2026-10-12，六轮 grilling（17 问）定完。以下契约**自足**：`/to-spec` 之后实现会在别的会话里读它，看不到本图与 charting 对话。范围级决定（冻结项 6 / 7 / 8 / 11）不在本票重开。

### §1 宿主与所有权

**`Harness` 就是一场会话的进程内实例（会话宿主），不造新词。** 依据：`assemble` 一次产出一个 `Harness { opened, session, provider, speaker, cancel }`（`src/lib.rs:152-167`），而 `OpenedSession` 的 17 个字段里会话级只有 5 个（`id` / `log` / `cwd` / `outputs_dir` / `resuming`），其余（工具表、锁表、权限策略、`asker`、问卷、hook、技能、`agents_md`、沙箱 `OnceLock`）本来就是「一次组装一份」—— 正是冻结项 6 要「一场一份」的那套。新造一层 `SessionHost` 只是把同样的字段换个壳。

**要补的一个真缺口**：`OpenedSession` 里的 `render: RenderHandle` 与 `render_task: JoinHandle<()>`（`src/lib.rs:229-230`）是**进程级**的（一条 broadcast + 一个渲染器任务，`src/render/mod.rs:161-164`、`253-268`），必须抽出来做成全进程一份，在 `assemble` 时注入。抽出去之前，一场会话一个 host 会各持一份渲染器句柄，那是错的。

**新增宿主表**（新词，写进 `CONTEXT.md`）：位于 CLI 组装点之上、由主循环持有（现状 `interactive_loop(&mut harness, ...)`，`src/cli.rs:1490`）。每行：

- `id: SessionId`
- `cwd: PathBuf`
- `state: watch::Receiver<SessionState>`（三态，见 §3）
- `cmd: mpsc::Sender<SessionCommand>`（见 §3）
- `cancel: CancelSignal`（克隆句柄，见 §3）
- `join: JoinHandle<()>`

表级另有一个 `foreground: watch::Sender<SessionId>`（见 §2）。

**生命周期**：`assemble` 产出 `Harness` → 起 task 拥有它 → 入表 → 切前台；关闭时发 `Close`、await `join`、从表移除。

**不变量**：同一个 `SessionId` 在宿主表里只能有一行。否则同一路径两份 `EventLog` 会各自维护 `next_seq`、写出重复 `seq` 且互相不可见（`src/events.rs:1022`）。

### §2 前台切换

**前台指针 = 全进程一个 `watch<SessionId>`**，宿主表的 `foreground` 就是它的发送端，每个 task 持接收端。于是：

- **切换 = 改一次 watch**，所有 task 自己反应：待答的 asker 因此放行（§6），「前台全量 / 后台只送状态」（§4）也由它翻转。
- 已在宿主表里 → **零延迟**，没有要等的东西。
- 不在宿主表里 → 装载 task，见 §9。

**忙闲与切换无关**：目标正在跑回合**不阻挡**切换（切走照跑正是本图的目的）。票面原写的「检查空闲 → 换前台」里的检查空闲**被取消**。

**票 02 已定的三类回执照旧**（不重开）：已是前台 → 只交键盘 + 落焦点；cwd 目录不存在 / 流损坏 → 各回一句提示行。

**视图重建的出口**：切换完成后给渲染侧一条控制请求；请求形状与重建规则由票 08 定，本票只保证「切换动作有一个明确的渲染侧出口」。

### §3 task 协议与后台推进

**每场一个 tokio task，拥有自己的 `Harness`。** 由此新增一条约束：`Harness` 必须满足 `Send`（现在没有这个编译期断言；回合 future 只被 `Box::pin` + `select!` poll，`src/cli.rs:2758`，`agent.rs` 里没有 `tokio::spawn`，而 runtime 是 multi_thread，`src/cli.rs:93-96`）。

**命令**：每场一条 `mpsc<SessionCommand>`，宿主表持发送端、task 持接收端。变体只有三个：

- `Submit(Submission)` —— 把前台输入区的一次提交交给这场会话（task 拥有 `Harness`，主循环不能直接调 `harness.run_turn`）
- `Cancel` —— 取消这场会话当前在跑的动作
- `Close` —— 关闭并结束 task

**状态**：每场一个 `watch<SessionState>`，三态 `Idle` / `RunningTurn` / `AwaitingHuman`。task 是发送端、宿主表持接收端。选 `watch` 的理由：它「保最新不丢」，而渲染通道是 `broadcast`、满了**静默丢弃**（`src/render/mod.rs:73-75`）—— 宿主表的状态快照不能被丢。宿主表同时在每个状态变更点往渲染通道发一条 `RenderEvent::SessionState { id, state }`，那是给渲染器画标记列的（§4）。

**取消**：每场一份既有 `CancelSignal`（内含 `watch::Sender<bool>`，`src/agent/cancel.rs:24`），宿主表持一份句柄、`Cancel` 命令走它。现状「一个信号管一场会话、每次运行开头 `reset()`」（`src/lib.rs:688`、`735`、`1056`）的限制正好因为每场一份而消失 —— 信号不再跨会话共享。

**退出**：见 §7。

### §4 事件分流

`RenderEvent`（`src/render/mod.rs:86-121`）**打上 session id**（现状只有 speaker，`events.rs:828-832`，多场共流时前端分不出谁说的）。

- **前台会话**：送**全量**，含 `Delta` 增量文本。
- **后台会话**：只送**状态转换**（§3 的 `RenderEvent::SessionState`）与计数；**不送** `Delta` / `Logged` / `Notice`。

理由：通道是 `broadcast`、容量 1024、`send` 失败被忽略（`src/render/mod.rs:75`、`167-213`）—— 多场全量合流会先把**前台的**增量文本丢在容量上限上，而用户正盯着那些。后台只送状态，省通道且刚好够画票 02 的标记列（`▍` / `?` / `●`）。

**切回时怎么补齐，不在本票定，交票 08**（两边只对接接口：本票保证「后台事件可按会话归属 + 状态随时可读」，票 08 定重建规则）。

### §5 并发预算

**不设并发上限**，不新增进程级闸门。

理由（charting 期事实）：没有现成的并发闸门可复用 —— `DayLedger`（`src/session/ledger.rs:25-34`）只是按 UTC 日的**记账**且尚未接线到任何视图，`budget_limit` 只用于显示（`src/cli.rs:383`、`744`、`src/render/panel.rs:101`），撞限流的既有兜底是 `provider_retries`（缺省 2）。所以「加上限」= 新机制，而自用工具的并发数由人自己控制（开几场是人的手势）。

**防烧钱仍靠既有机制**：每场自己的额度闸门（回合循环 `src/agent.rs:563-577`、预检 `615-627`、派发新执行者 `1130-1143`）。本票**不改**跨会话额度求和 —— 那是 goal-loop 在有目标时的语义（见 `CONTEXT.md` 的**累计 token 上限**）。

**界面给一个并发读数**（「几场在跑回合」），由宿主表的状态快照派生（数 `RunningTurn`）；**落在哪由票 06 定**。

### §6 需人答的暂停

冻结项 8 的落地：**asker 按会话包装**（`Asker` / `UserQuestions` 的会话级实现，持 `SessionId` + 一个读前台指针的 `watch<SessionId>` 接收端）。

- `put()` / `ask()` 先读「我是不是前台」：
  - **不是前台** → **不往前端送请求**，把 future **挂在 `watch` 的变化上**（await），同时把状态置 `AwaitingHuman`（§3）。
  - **是前台** → 把请求送进既有的 `ConsoleRequest`（`Ask` / `Questionnaire`，`src/render/input.rs:133-186`）。
- **切到前台即唤醒**：请求从没被消费过，所以「切过去原样弹出」自动成立，不需要前端记住 N 个悬浮请求、也不需要 ticket 08 设计浮层队列。
- **回答**走既有的 `reply` oneshot，**不经宿主表命令通道**（宿主表不代答）。
- **列表标记与提示行**由状态通道驱动：`AwaitingHuman` → 票 02 的 `?` + 提示行。
- **多场同时待答**：各自挂在自己的 `watch` 上，谁被切到前台谁弹。
- 后台会话的「需人答」**永不自动作答**（冻结项 8）。

### §7 中断与退出

**`Ctrl-C` 只取消前台当前的动作，永不退出。** 这条**改既有行为**：现状「第二次按下变退出码」要删（`src/agent/cancel.rs:21` 的注释、`src/cli.rs:1748-1750` 的 `quit.code()` 路径）。

- 「前台当前动作」= 前台会话正在跑的**回合**，或正在跑的**重播**（启动时把历史分帧重播进转录，`.scratch/tui-history-replay/`）—— 两者是同一件事的两种形态，重播打断因此不再是第三个特例。
- **前台没有可取消的东西**时（空闲且不在重播）：提示行一句（如「前台没有在跑的回合」；后台有在跑时顺带说那句）。
- **`Ctrl-D` 退出，现状不变**（退出确认）。

**退出闸门**：所有退出入口（`Ctrl-D`、`Submission::Quit` 等）共用一条检查 ——

> **只要有任何一场会话在跑回合，就拒绝退出。**

拒绝时的回执：提示行说「还有 N 场在跑」并给一个**可点入口**，指向在跑集合里的**第一场**（只有一场时就是它）；点它切前台、`Ctrl-C` 取消、再 `Ctrl-D`。若还剩就再指下一场，直到清空才能退。落点（提示行里那个可点元素怎么画）归票 06 / 票 02，本票只定「要有这个入口」。**不自动取消、不代用户做决定。**

**退出流程**：过闸门后广播 `Close` → await 全部 `JoinHandle` → 退出码照旧。

**退出不写 `SessionEnded`**（与「切走不卸载」同一条立场）。每场流停在自己最后一条事件上；既有的「看最后有没有收尾事件」判据与 `-c <id>` 续会话语义照旧（细化的判据归票 07 §2）。`EventLog::append` 已经是加锁 + flush（`src/events.rs:1052`），停在哪就是哪。

**进程被杀的多场恢复：不需要新机制。** `recover_pending_calls`（`src/agent/history.rs:40`）是**按一场会话的流**工作的，与同时有几场无关；N 场被杀就是 N 条各自可恢复的流。本票只补一条不变量（见下）。

### §8 进程级与会话级手势

判据一句话：**这个设置改了以后，切到另一场会话时该不该跟着变。**

| 落当前前台（会话级） | 全进程一份（进程级） |
|---|---|
| `/model`、`/effort`（ADR 0017 已定「只活在这一场会话里」） | `Ctrl-O` 左栏开关 |
| `Shift+Tab` 权限模式 | 宽度档 |
| `/clear` | 终端标题 |
| | 左栏页签选择与页内焦点（**交票 08**） |

**`/clear`、阈值 rollover（goal-loop 的翻页）、页签里的新建 = 同一个动作**：同 cwd 组装一场新会话 → 切到前台；**旧的留在宿主表里**，仍是活着的后台会话、可以切回去；不写 `SessionEnded`。

理由：`rollover`（`src/lib.rs:926`）本来就不写 `SessionEnded`、不卸载渲染器；若 `/clear` 把旧的从表里踢掉，那就是「关」它，与冻结项 11「旧的不关」直接矛盾。goal-loop spec 已有的「两段入口、一段机制」自然扩成三段。

**细节交票 07**：新会话继承哪些模型 / 档位 / 权限模式 / 沙箱档、旧会话空闲后要不要自动停、关闭语义。

### §9 工具副作用与首次装载

**写互斥**：进程级一张 `cwd → PathLocks` 表，**键是规范化 cwd**。同 cwd 共享一张（两场写同一文件因此串行），不同 cwd 各一张。

现状：`PathLocks`（`src/tools/paths.rs:246-274`）= 逐路径一把 + 一把**工作区级** `exclusive`，但每个组装点各建一张（`src/cli.rs:422`、`784`、`2961`）→ 同进程两场独立组装的会话不互斥。按 cwd 分键是必须的：若全进程只共享一张表，两场**不同 cwd** 的会话会因为那把 `exclusive` 互相阻塞 —— 那是错的作用域。宿主在 `assemble` 时从这张表取。跨进程仍不做文件锁（既有状况，本票不扩大范围）。

**沙箱探测 / 权限策略 / 技能 / 工具表：一场一份。** 这是冻结项 6 的直接推论，写下来防止将来被提到进程级共享。现状已经是这样：`sandbox_probe` 是 per-`OpenedSession` 的 `OnceLock`（`src/lib.rs:228`），`policy` 是 per-`assemble` 的 `Arc<Mutex<Policy>>`（`src/lib.rs:213`）。

**首次装载（目标不在宿主表）**：点击时先用**列表已有的数据**做能立即判定的检查（已是前台 / cwd 目录不存在 / 流损坏 → 走票 02 的回执）；通过后交给一条**装载 task** 去 `assemble`，装完再切前台；期间提示行一句；**不新增列表标记态**（票 02 的标记列保持原样）。

理由：`assemble` 里的沙箱探测 + 技能发现不是一次点击能同步承担的；票 03 测得的「读流很便宜」（单场几 ms）不含这部分。**装载路径的细节**（`SessionStarted` 补记字段、旧流回退、失败降级、`-c` 是否共用）**交票 05**。

### §10 上限与卸载

**宿主表无上限**（与 §5 一致）。**卸载只由手动关闭触发**（冻结项 7）。关闭语义（前台不许关自己、有回合在跑时能不能关、要不要确认）**交票 07**；票 07 §6 要的「上限」取同一个值：**无上限**。

### 不变量（票不得违反）

1. 事件流只追加且是唯一真相；渲染器状态不进流（既有）。
2. **同一个 `SessionId` 在宿主表里只能有一行**（§1）。
3. 前台只有一个；后台会话照跑且不卸载（冻结项 7）。
4. 后台会话的「需人答」永不自动作答（冻结项 8）。
5. `messages` 投影前缀与模型可见文本一个字节不动；plain / headless 渲染器不受影响。
6. **退出时不得有正在跑的回合**（§7 的闸门）；退出不写 `SessionEnded`。

### 要改的既有行为（实现期的落点）

- `Ctrl-C` 第二下退出的语义去掉（`src/agent/cancel.rs:21`、`src/cli.rs:1748-1750`）。
- `render` / `render_task` 从 `OpenedSession` 抽成进程级一份（`src/lib.rs:229-230`、`257-258`）。
- `PathLocks` 从三处各建改为进程级按 cwd 分表（`src/cli.rs:422`、`784`、`2961`）。
- `RenderEvent` 加 session id（`src/render/mod.rs:86-121`）。
- `Harness` 要能 `Send`（新约束；`assemble` 的返回值要被 `tokio::spawn` 接受）。
- `rollover`（`src/lib.rs:926`）从「换会话」扩成「新建一场 + 入表 + 切前台」。

### 交出去的（不属于本票）

| 内容 | 票 |
|---|---|
| 切回怎么补齐、状态形状（每会话一份投影 vs 单份重建）、浮层与草稿归属 | [08](08-front-view-isolation.md) |
| 装载路径（补记字段、旧流回退、失败降级、`-c` 共用） | [05](05-session-facts-on-attach.md) |
| 新建与关闭的细节（继承规则、忙时关闭、确认、上限取值） | [07](07-new-session-and-unload.md) |
| 会话页形态、并发读数落点、退出被拒时那个可点入口怎么画 | [06](06-session-page-shape.md) |
| `SessionStarted` 补字段与 `SCHEMA_VERSION` 涨版 | 05（与票 01 同一次） |
| 会话列表的搜索与过滤 | [10](10-session-list-search.md) |
| 文档回改（`lifecycle.md` 等） | [09](09-doc-revision.md) |

### 一条被本票清掉的雾

「后台状态在列表里要不要自刷新」由 §3 + §4 答掉一半：状态转换**经渲染通道**到前端，所以标记会随状态变。剩下的「重画时要不要抢走焦点行与阅读位置」取决于票 06 定的页生命周期，仍留在图的 `尚未明确`。

## 评论

**2026-10-12 · 票 06 / 07 / 08 收走了本票交出去的东西**：

- §4 的「切回怎么补齐」由 [grilling：前台视图的会话隔离与切换重建](08-front-view-isolation.md) 定成
  「投影整块换 + 按事件流快照重建 + 按 `seq` 去重排空」；票 08 同时是本票 §4 那条「`RenderEvent`
  打 session id」的唯一消费方。
- §5 的并发读数落在会话页页顶、§7 退出被拒的可点入口写法，由
  [prototype：左栏「会话」页的形态](06-session-page-shape.md) 画定。
- §10 的上限取值（**无上限**）由 [grilling：新建会话与卸载边界](07-new-session-and-unload.md) 取同值；
  票 07 还给「关闭」补了第一条落点。
- 本票 §3 那条「`resuming` 只认 `StreamStarted`」的闸门在票 07 §7 变成一条显式不变量：
  **已在宿主表里的会话绝不重走 `assemble`**。
