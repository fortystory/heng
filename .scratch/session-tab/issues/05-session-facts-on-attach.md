# grilling：切入历史会话的会话级事实

Type: grilling
Status: resolved
Blocked by: 04
Part of: ../map.md

## 问题

把「点一条磁盘上的历史会话，怎么把它装成一个活的 host」定到可实现级：那些**绑一次、不能猜**的会话级事实（模型、思考档、权限模式、沙箱档、cwd、技能）从哪来、缺了怎么退。

**范围级决定已在 charting 冻死**（见 `../map.md` 的冻结项 9）：会话级事实**从流上来** —— `SessionStarted` 补记 model / effort / 权限模式 / 沙箱档；旧流缺字段则回退「沿用当前前台设置」并**如实说明**。本票只定形状与回退细节。

## 现状（charting 期已核实，别重查）

- `SessionStarted { session_id, cwd, schema_version }` —— **没有模型**；继续的流**不会**再写第二条 `SessionStarted`（`src/lib.rs:342-351`）。
- `SessionFacts`（`src/cli.rs:376-393`）在组装期注入一次；运行中只有 `SessionUpdate` 能改 model / context_window / speakers（`src/render/input.rs:291-304`），`session_id` / `session_dir` **没有**运行中更新路径。
- `Harness::rollover` 走 `EventLog::create`（只能开新会话）；打开已有流的 `EventLog::open` 已在组装路径用过（`src/lib.rs:262-266`）。

## 需要定

1. **补记哪些字段**：model、effort、权限模式、沙箱档；要不要连 provider profile / 上下文窗口一起记（换了厂商名字会变）；哪些是"会话的身份"、哪些是"那一刻的事实"（对照既有 `SandboxStatus` 是**每次运行**都补记一条的写法）。
2. **形状与兼容**：加进 `SessionStarted` 的字段用 `Option` 还是 `#[serde(default)]`（旧流读新结构不能失败）；`SCHEMA_VERSION` 与票 01 的 `SessionTitled` 是不是**同一次涨版**（若同一次，两票必须给出一致的版本号与改动清单 —— 这是本票与票 01 的对接点）。
3. **旧流的回退**：缺字段时沿用当前前台会话的设置，界面上**哪一句**说出来（措辞层出）；要不要在流上补记一条"这次运行用的模型"（模仿 `SandboxStatus` 的做法），好让下一次切入不再靠猜。
4. **装载清单**：切入一场磁盘会话时，host 里哪些东西必须按**那场会话**建 —— cwd、`agents_md` / 技能发现、沙箱探测（`OnceLock` 现在绑 `self.cwd`）、权限策略、工具表（工具声明是缓存前缀的一部分）、provider。`Harness::rollover` 今天不更新这些，本票要说清是"重建成一个新 host"还是"改造 rollover"。
5. **与启动路径的关系**：`-c <id>`（`choose_session` 已经会跨桶找、并让 cwd 跟着目标会话走）与"界面里点一条"是不是同一套装载逻辑；若是，抽在哪一层。
6. **模型侧的前缀**：每场会话的 `messages` 前缀本来就是它自己流的投影，切会话**不重放 messages**；确认这条不需要额外机制，并写清"模型不在流上"这件事在新字段落地后就不再是问题。
7. **失败降级**：流损坏、`SessionStarted` 缺失、cwd 已不存在（worktree 被删）时能装到什么程度；要不要允许"只读地看，不接着跑"（**注意**：只读预览不在冻结范围里，若要引入必须显式说明这是新范围）。

## 先读

- `../map.md`（冻结项 6、9；票 01 的版本号对接）
- `src/events.rs` 的 `SessionStarted` / `SandboxStatus` / `SCHEMA_VERSION`
- `src/lib.rs` 的 `OpenedSession::open` / `reopen` / `rollover` / 沙箱探测
- `src/cli.rs` 的 `choose_session` / `find_session` / `SessionFacts` 组装
- `.scratch/continue-by-id/spec.md`（`-c <id>` 的既有语义与 cwd 跟随）
- `src/render/input.rs` 的 `SessionUpdate`（运行中可改的那三样）

## 答案落点

契约级：补记字段表 + 版本与兼容写法 + 回退与如实说明的文案 + 装载清单 + 与 `-c` 的共用接缝 + 失败降级表。不要写实现代码。

## 作答

**一句话**：切入一场磁盘会话 = 走一条共用的装载路径 `assemble_session`；会话级事实从流上读（开场快照 + 变更快照），旧流缺字段回退**进程配置默认**并如实说明；装不成可写会话的（cwd 已不存在 / 大部分可读的坏流）降级成**只读预览**。

本票引入**两处新范围**，`/to-spec` 折 spec 时必须显式声明：① 新事件 `SessionFactsChanged`（§1、§2）；② 只读预览与它的"读到坏点为止"新读法（§6、§7）。

### 1. 事件形状与版本

**`SessionStarted` 补五字段**（与票 01 同一次涨版）：

```rust
SessionStarted {
    session_id: SessionId,
    cwd: String,
    schema_version: u32,
    model: Option<String>,           // 模型 id（配置里那个名字）
    provider: Option<String>,        // ProviderProfile.name
    effort: Option<String>,          // reasoning_effort 档位名
    permission_mode: Option<String>, // readonly / ask / workspace / auto
    sandbox_mode: Option<String>,    // bwrap / off
}
```

- 全部 `#[serde(default)]` + `Option`：旧二进制读新字段安全（serde 忽略未知字段，`tests/event_log.rs:358` 是先例），新二进制读旧流安全（缺 = `None`）。`SCHEMA_VERSION` 2 → 3，与票 01 **同一次、共用一份改动清单**（票 01 §1、§10）；本票不改任何读写分派。
- **不记** `context_window`：它由 model 经静态能力表派生（`src/provider/capability.rs:121-137`），记进流会在能力表升级后变成假事实。**不记**沙箱 `availability`：`SandboxStatus`（`src/events.rs:349-356`）已经负责"每次运行那一刻"的事实。
- 值的类型是**字符串协议标记**，不引 `permissions::Mode`：`events` 在内部依赖 DAG 最底层、不依赖任何内部模块，照 `SandboxStatus.mode: String` 的先例。

**新变体 `SessionFactsChanged`**（log-only，投影零产出，照 ADR 0019 与 `CommandRun` 的先例）：

```rust
SessionFactsChanged {
    model: Option<String>,
    provider: Option<String>,
    effort: Option<String>,
    permission_mode: Option<String>,
}
```

- **全量快照**，不是增量：缺（`null`）= 回到配置默认。于是清空语义自然（增量写法要区分"这次没变"与"清空"，得用 `Option<Option<T>>`）。
- `redact()` 无事可做：四个值都是标识符 / 协议标记，没有散文。
- **当前会话事实的权威链** = `SessionStarted` 五字段打底 → 流上**最后一条** `SessionFactsChanged` 覆盖。与 `GoalSelected` 的"当前值 = 流上最后一条"同一条派生纪律。
- `sandbox_mode` **不进**这条事件：沙箱是每次运行重探的（§4）。

### 2. 写入点

- **开场**：`record_skeleton` / `agent::record_session_started` 带上五字段（`src/lib.rs:384-403`、`src/agent.rs:302-313`）。
- **每次装载后立刻补一条完整快照**（含从流里恢复出来的值）：续流路径今天只补 `SandboxStatus`（`src/lib.rs:348-362`），这里再加一条 `SessionFactsChanged`。理由与 `SandboxStatus` 同构 —— "流上最后一条 = 最近一次真正用它跑过的设置"，于是**下一次切入不再靠猜**。**只读预览不写任何东西**（§6），所以没有这一条。
- **运行中变更**：`/model`、`/effort`、`Shift+Tab` 三个入口各写一条完整快照（共同落点 `apply_switch`，`src/cli.rs:1353-1358`；模式走 `ModeCycle::cycle`，`src/lib.rs:1244-1252`）。**这条推翻 ADR 0017 的「模型与档位不进事件流」**，交棒见 §10。

### 3. 回退表（旧流缺字段）

| 事实 | 缺失时取谁 |
| --- | --- |
| `model` | `--model` 或 `config.default_model`（`src/cli.rs:263-266`） |
| `provider` | 上面那个 model 经 `resolve_model` 得到的 profile（`src/config.rs:1324-1343`） |
| `effort` | 该 profile 的 `params.reasoning_effort`（`src/config.rs:349-360`） |
| `permission_mode` | `--mode` 或 `[permissions] mode`（`effective_mode`，`src/cli.rs:230-232`） |
| `sandbox_mode` | `[sandbox] mode`（可用性反正每次重探） |
| `cwd` | `SessionStarted.cwd`；整条 `SessionStarted` 都缺 → 请求目录（启动 cwd / 点选时的进程 cwd） |

**冻结项 9 的措辞收窄**：原文「沿用**当前前台**设置」在启动路径上不成立（那时前台就是目标自己），本票改成「沿用**进程配置默认**」—— 与 `-c` 今天的既有行为一致。

### 4. 装载清单

**按目标会话重建**（每次 `assemble_session` 一份）：

- `cwd` = `SessionStarted.cwd`（`session_cwd` 已有，`src/session/store.rs:49-51`）。
- `agents_md` / 技能 = `context::load_agents_md(&cwd)` / `Skills::discover(&cwd, home)`（`src/lib.rs:271-272`）。用户级技能根仍取**进程的 HOME**，跨机器搬运时可能与原机不同 —— 按"环境重探"接受。
- 沙箱探测 = 对目标 cwd **重新** `resolve_availability`（`OnceLock` 绑 `self.cwd`，`src/lib.rs:333-340`）；流里的 `SandboxStatus` 只用于说明"当时是这么跑的"。
- 权限策略 = 恢复出的 mode 走 `Policy::for_mode(mode)`（`src/cli.rs:425`）。
- 工具表 + provider = 按目标会话的 cwd 与配置重建（工具声明是缓存前缀的一部分）。
- MCP = 随工具表按目标 cwd 重连（`.mcp.json` 本来就按目标 cwd 读，`src/cli.rs:301`）。
- `outputs_dir` = 目标会话目录（`src/lib.rs:251-254`）。
- `EventLog` = `EventLog::open`（已有流，`src/events.rs:1018-1032`），不是 `create`。

**进程级共用**（不随会话重建，票 04 已定）：渲染通道与渲染器任务、宽度档、终端标题、左栏页签与焦点、`PathLocks` 的 cwd 分表、宿主表与前台指针。

**`carried_tokens` 不按流上的 `UsageRecorded` 重算**：沿用进程口径，不复活旧目标的额度；目标归属由票 07 定。

### 5. 与 `-c` 的共用接缝

抽一个**装载函数**（契约名 `assemble_session`，落地名由实现期定）：

- **输入**：`StoredSession` + 目标 cwd + 只读标志 + 进程级那份 `render` / `render_task` / `PathLocks` 表。
- **`-c <id>`**：启动期唯一调用点，装完直接成为唯一前台（没有宿主表、没有"切前台"这一步）；`choose_session`（`src/cli.rs:3420-3463`）照旧只决定"装哪一场、cwd 是谁"。
- **界面点选**：运行期由票 04 §9 的装载 task 调用它，装完入表 + 切前台。
- 两条路径的差异**只有**上述启动期那一处，以及 `-c` 的初始 cwd 可能被换成目标会话的 cwd（`src/cli.rs:3442-3461`）。`-c` 命中 cwd 不存在 / 坏流时，走同一条只读降级。
- `interactive` 里散着的 model / mode / policy / sandbox / 工具表组装（`src/cli.rs:263-425`）搬进这个函数。

### 6. 只读预览（新范围）

- **入口只有降级**，不新增手势、列表里没有第二个动作。触发两类：cwd 不存在；坏流但可半读（§7 第二档）。
- **装配最小**：`EventLog`（半读）+ 历史重放（`ConsoleRequest::Replay`，现状只在 `-c` 时发，`src/cli.rs:449-451`）+ 身份与标题。**不建** provider、工具表、MCP 连接、沙箱探测；工具详情的 Schema 面拿不到就空着。
- **闸门：凡写流的一律禁** —— 输入提交、所有 `/` 命令（含 `/clear`）、`Shift+Tab`、票 01 的页内改名、**新建**（`/clear`、阈值 rollover、页签新建是同一个动作，票 04）。被拒回执统一一句「这场会话只读」（措辞层出，**不带**可点入口）。
- **保留**：切页、开详情、滚动、复制、切前台、切走；`Ctrl-C` 走票 04 的"前台无可取消动作"回执；`Ctrl-D` 退出闸门**不算**它在跑。
- **身份**：宿主表每行加一个只读标志，**不动**票 04 的 `Idle` / `RunningTurn` / `AwaitingHuman` 三态；列表里落在**标记列**（那个永远预留的 1 列，票 02）画一个只读字形 + 整行 `MUTED` —— **不新增尾标**，行宽是定宽的（窄档 1 + 21 + 5），尾标会挤掉标题或时间。
- **卸载**：手动关闭（票 04），关闭手势与卸载边界归票 07。
- **本票推翻的三处**：票 04 §9「cwd 不存在 → 直接回执、不装载」；票 02 里「cwd 不存在 / 流损坏 → 切不了」那两句回执（改成「只读打开」）；`CONTEXT.md` 的「会话」页签词条里「cwd 目录已不在的整行 `MUTED` 且切不过去」那半句。

### 7. 失败降级表（三档）

| 流 / cwd 的状态 | 结果 |
| --- | --- |
| 正常；或只有尾行坏（`repair_before_append` 能修） | **可写**装载（既有行为，`src/events.rs:1018-1032`） |
| 中间坏，但坏点之前能解析出 `SessionStarted` | **只读预览**：读到坏点为止（**新读法，新范围**） |
| 连 `SessionStarted` 都读不到 / `read_events` 整体失败 | 拒装 + 提示行「读不出这场会话」 |
| cwd 已不存在 | **只读预览** |
| 整条 `SessionStarted` 缺失（手工截断的旧流） | 仍装成**可写**：cwd 用请求目录、其余事实用配置默认，并如实说明 |

"读到坏点为止"**只用于只读预览**（不写流的场景安全），永不用于可写装载 —— 可写装载的判据仍是"整条流可解析"。

### 8. 如实说明（措辞层）

- **出口**：提示行一条 + 状态行 / 页内一个**常驻标记**（提示行会被下一次交互刷掉，而"这场会话跑在配置默认上"是持续状态）。
- **粒度**：**最多一条**，把同时缺的字段合并进一句：`这场会话没记 model、权限模式，已按当前配置来`。
- 只读降级与流损坏各有自己的常驻标记（`只读` / `损坏`）。
- **不说**：技能菜单与模型上下文不同源。核查结论 —— 切入时模型读到的是流里旧 `ContextInjected{SkillsCatalog}` 的逐字快照（`src/lib.rs:348-351`），菜单来自本次组装按目标 cwd 重扫的 `Skills`（`src/lib.rs:272` → `779-781` → `src/cli.rs:485-489`）；只有磁盘或 home 变过才不同，且即使同一次扫描两者也**按设计**不同文案（菜单留 disable 技能且不截断，模型清单过滤它并按预算截断）。这条只在票与文档里记账，不进界面。

### 9. 模型侧前缀与边界

- 切会话**不重放 `messages`**：每场会话的前缀本来就是它自己流的投影。`SessionFactsChanged` log-only、零产出，前缀逐字不变。"模型不在流上"这件事在 `SessionStarted` 补字段后不再是问题。
- **与票 08 的边界**：本票只定"事实从哪来、装完交给谁"；切回前台时渲染侧 facts（model / mode / context_window / speakers）的同步与视图重建由票 08 定。现状 `SessionFacts` 是组装期注入一次（`src/render/tui.rs:340-396`、`src/cli.rs:376-393`），运行中只有 `SessionUpdate` 能改 model / context_window / speakers（`src/render/input.rs:154-160`）—— **权限模式与 provider 没有运行中更新路径**，是交给票 08 的缺口。
- **与票 04 的边界**：宿主表、三态、切换语义、asker 挂起、退出闸门不动。

### 10. 交棒项

- **票 09（文档回改）追加**：
  - `CONTEXT.md`：「会话」页签词条里"切不过去"那半句改成"只读打开"；补新词**只读预览**；
  - `docs/permissions.md` 与 `src/permissions.rs:352-354` 的「模式从不进事件流」要改（模式现在进流，见 §2）；
  - **ADR 0017 的「不进事件流是刻意的」被本票推翻** —— 建议补一条 ADR（会话事实进流：可恢复性 vs "流只记发生过的事"），或在 ADR 0017 上注明推翻与理由；
  - `docs/observability.md`（事件清单）与逐面文档补 `SessionStarted` 新字段与 `SessionFactsChanged`；
  - `docs/render.md` 的会话页一节：只读尾标与只读态闸门。
- **留给别的票**：只读会话的关闭手势与卸载边界 → 票 07；切回前台的视图重建与 facts 同步 → 票 08；两场会话同时推同一个目标 → 票 07 的雾。

### 11. 本轮 grilling 的轮次

三轮提问（16 问）关掉：五字段与"开场快照 / 每次运行的事实"的切分、变更落流（新范围）、回退来源收窄成配置默认、环境事实重探、装载清单、`assemble_session` 接缝、只读预览（入口 / 闸门 / 身份）、每次装载补快照、如实说明的出口与粒度、损坏流三档。
