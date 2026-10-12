# grilling：新建会话与卸载边界

Type: grilling
Status: resolved
Blocked by: 04
Part of: ../map.md

## 问题

把「再加载一场空会话」与「手动关闭一场会话」这两头定死 —— 它们是宿主生命周期的两端。

**范围级决定已在 charting 冻死**（见 `../map.md` 的冻结项 7、11）：新建 = 同 cwd 再加载一场**空**会话，**旧的不关**（不写 `SessionEnded`）；不卸载直到**手动关闭**。

## 需要定

1. **新建的动作**：同 cwd `SessionStore::create` + 写 `SessionStarted`（带上票 05 的补记字段）；新会话用什么模型 / 思考档 / 权限模式 / 沙箱档（继承当前前台？按 `config` 重解析？给一条规则与理由）。新会话是立即成为前台，还是只入列表。
2. **旧会话的去处**：不写 `SessionEnded` 意味着它的流停在最后一条事件上；下次 `-c <id>` 拿到它时的语义是"继续追加"（既有行为）。**写清这不会让"正常收尾 / 异常中断"的判定出错**（`src/lib.rs:853` 一带有按"最后有没有收尾事件"分流的逻辑，见 `harness_resume` 一族）—— 若会出错，给一个不引入新状态标记的解法。
3. **与 `/clear`（rollover）的关系**：`/clear` 现在就是"同进程开一场新会话"（`Harness::rollover`）。它是改成走同一套"新建会话"路径，还是保留（两条路各有语义）？两条路并存要不要在界面上区分（`/clear` 清当前这一格、页签里的"新建"新起一格）。
4. **关闭的动作**：从宿主移除（盘上目录留着、随时能再加载）；**前台不许关自己**（先切走再关）；有回合在跑时能不能关（等它结束 / 拒绝 / 强停并写收尾）；关闭要不要确认（不可逆的是"正在跑的会话被你关掉"，盘上数据不丢）。
5. **进程退出时**：后台会话写不写收尾；下次启动是否自动加载上次的宿主集合（默认**不**自动，除非本票论证要）；退出提示行要不要说"还有 N 场在后台跑"。
6. **上限**：宿主里同时留几场（与票 04 §10 对接，两票给同一个值）；超上限时的行为由谁决定（拒绝新建 / 提示先关一场）。
7. **视图**：新建与关闭都会换前台的"形状"（新会话是空转录、关闭的是别的会话），谁触发视图重建 —— 交给票 08，本票只写清触发点。

## 先读

- `../map.md`（冻结项 7、11；票 04 的宿主形状）
- `04-session-host-and-concurrency.md` 的作答（**本票被它阻塞**）
- `src/lib.rs` 的 `rollover` / `reopen` / 恢复与收尾判定（`resuming` / `SessionEnded` / `recover_pending_calls`）
- `src/cli.rs` 的 `/clear` 分支与 `clear_session`
- `src/session/store.rs` 的 `create` / `delete` / `prune`
- `.scratch/goal-loop/spec.md` §"翻页"（rollover 的既有契约）
- `.scratch/clear-command/seed.md`（意向已改成"结束当前会话、开一个新的"）

## 答案落点

契约级：新建规则（含继承与是否上前台）+ 旧会话不关的后果与收尾判定 + 与 `/clear` 的关系 + 关闭规则（前台自关 / 忙时 / 确认）+ 退出与上限。不要写实现代码。

## 作答

2026-10-12 定完：两轮 grilling（9 + 8 问）加第三轮 3 问，另有两份只读代码调研（收尾判定、
`/clear` 与 `create`/`delete`/`prune` 现状）。以下契约**自足**；范围级决定（冻结项 7、11）不在
本票重开。调研推翻票面的一处假设（见 §2）。

### §1 新建会话的动作

**路径**：`SessionStore::create(cwd)`（`src/session/store.rs:85-115`，只建桶 + 会话目录 +
`outputs/`，**不建 `log.jsonl`**）→ 组装点建 log（`src/lib.rs:262-266`）→ `record_skeleton`
写 `SessionStarted`（`src/agent.rs:302-313`，由 `src/lib.rs:384-385` 调，只在 `resuming == false`
时）。票 05 的补记字段（model / provider / effort / permission_mode / sandbox_mode）**现状不在
代码里**（`src/events.rs:340-344` 只有 `session_id` / `cwd` / `schema_version`），由票 05 加，
本票按它已经加上来写。

**cwd**：当前前台的 cwd；**空宿主态时**（§5）= **进程启动 cwd**。

**继承规则**（表里每一项都给依据；「继承前台」是现状 `rollover` 的行为）：

| 值 | 新建时 | 依据 |
|---|---|---|
| 模型 / effort | **继承前台** | `rollover` clone `self.session.config()`（`src/lib.rs:927`） |
| 权限模式 | **继承前台** | 现状是**脚手架级** `Arc<Mutex<Policy>>`（`src/lib.rs:281`、`:830-835`）；多会话下按票 04 §1 变成**一场一份**，语义不变 |
| 沙箱档 / 沙箱探测 | 按 cwd **重探** | 票 05 §环境类事实重探；同 cwd 结果等价于继承（`OnceLock`，`src/lib.rs:228`、`:333-340`） |
| 技能 / `agents_md` / 工具表 | 按 cwd 重探或重取 | 票 05；同 cwd 等价于继承（`src/lib.rs:271`、`:303`、`:311`） |
| `carried_tokens` | **清 0** | `src/lib.rs:932` |

**完成后立即成为前台**（与 `/clear` 现状一致）。**新建 = `/clear` = 阈值 rollover**：同一个
动作，旧的留在宿主表里可切回、不写收尾（票 04 §8）。两条路在界面上**不区分**。

**要改的既有行为：取消 `/loop` 闸门。** `Submission::Clear` 现在在 `loop_running` 时只回一句
（`src/cli.rs:1667-1678` 的 `clear_while_looping()`）。多会话之后这没有意义 —— 旧会话连它
在跑的 loop 一起留在后台，新会话开新的。键盘入口与页签入口**都去掉这道闸门**。

**入口**：会话页页顶读数行右端 `新建`（本票新定，回改票 06 §4），加上既有的 `/clear`。

### §2 旧会话不写收尾的后果（调研结论，票面的假设被推翻）

**零后果。** `SessionEnded` 在 `src/` 里**没有任何写入点**（定义在 `src/events.rs:357-359`，
24 处命中全是定义 / 投影忽略 / 渲染与统计消费 / hook 映射）；真实 store 里 **0/281** 条流以它
结尾。所有「正常收尾 / 异常中断」的判定读的是别的东西：

- goals 的收尾事件：`is_closing` 只认 `GoalCompleted` / `GoalStopped`（`src/goals.rs:772-777`），
  `ending` 从最后一条 `GoalSelected` 起扫（`:784-799`）。
- pending tool call：`recover_pending_calls` 只看 `ToolCallStarted` / `ToolCallCompleted` 配对
  （`src/agent/history.rs:40-59`、`src/events.rs:878-909`）。
- 复装闸门 `resuming` 只看流里有没有 `SessionStarted`（`src/lib.rs:267-270`）。

票面提到的 `harness_resume` **这个标识符不存在**（grep 零命中）；对应物是
`Harness::resume_goal` / `current_goal`（`src/lib.rs:856-871`）。`-c <id>` 对普通会话（没有
`GoalSelected`）**一个字都不说**（`src/cli.rs:1507-1539`），`--continue` 行为不变。

唯一被看见的错在显示层（`sessions ls` 末列恒「未结束」、`sessions stats` 的 stops 恒空）——
**这条已不是既成事实而是要在本票修**：它是「切走不卸载」让「未结束」成为常态之后必须说清的
东西。修法见 §6；**不新增 `SessionEnded` 写入点**。

### §3 关闭的规则

**关闭 = 从宿主表移除**：发 `Close` 命令 → await `JoinHandle` → 从表移除（票 04 §1）。
**不写收尾事件、不删盘**（`SessionStore::delete` 仍只属于 `prune`，`src/session/store.rs:166-181`）。
盘上目录留着，随时能再装载。

| 情形 | 规则 | 回执 |
|---|---|---|
| 焦点行 = 当前前台 | **禁止**（先切走再关） | 提示行一句「先切到别的会话再关」 |
| 那一场正在跑回合 | **拒绝**，不代发 Cancel、不自动写收尾 | 提示行一句「先在那一场按 ctrl-c 取消」 |
| 空闲会话 | **关**，**不二次确认** | 无 |
| 只读预览会话 | **能关**（只读禁的是写流；关闭不碰盘） | 无 |

- 「忙时拒绝」与票 04 §7 的退出闸门是**同一立场**：不自动取消、不代用户做决定。
- 「不确认」的理由：不可逆的只有「正在跑的会话」，而那一档上一行已经挡住；空闲会话关了随时能再开。
- 关闭**可以关掉最后一场**：宿主表为空 → 空宿主态（§5）。

### §4 退出与下次启动

- **退出不写收尾**：无需任何改动（`SessionEnded` 本来就没有写入点）。退出流程照票 04 §7
  （过闸门 → 广播 `Close` → await 全部）。
- **退出提示行**：过了在跑的闸门之后，若后台还有**空闲**会话，补一句 `还有 N 场后台会话`；
  与票 04 §7 的「还有 N 场在跑」不是同一句（一个说在跑、一个说驻留），两句各自出现。
- **下次启动不自动加载**：调研确认 `-c` 之外没有任何自动恢复路径（`src/cli.rs:168-193`，
  `parse_interactive` 默认 `resume = None`）。没有「上次宿主集合」这种状态，也不新增。
- **上限：不设**（票 04 §10，本票取同值）。调研确认 `src/` 里没有任何并发会话数常数或闸门；
  唯一现成的并发闸门 `max_parallel_executors`（`src/config.rs:49-52`）管的是执行者，不用。

### §5 空宿主态（新范围）

**触发**：关闭宿主表里最后一场。

- **会话页**：页顶行变成 `0 场`，右端只剩 `新建`（没有可关的，`关闭` 消失）；页区一句
  `没有会话`。页签常驻（票 02 §5）。
- **主列**：没有前台，转录不画内容；输入区禁用；向它提交（或在里面敲字）时提示行回
  `没有会话，先新建一场`。
- **新建的 cwd = 进程启动 cwd**（没有前台可继承，`§1` 的规则在这里退化成这一条）。
- 它**不是退出**：进程还活着，页签、`Ctrl-O`、宽度档照常。
- 新文案三条：`没有会话`、`没有会话，先新建一场`、`0 场` 这一行；**回改票 06**（页顶行的
  入口集合 + 页区文案多一条）。词汇表新增**空宿主**（`CONTEXT.md`）。

### §6 `sessions ls` / `sessions stats` 的末列派生（新范围）

**判据 = 末条事件的类型**（替换现在「读 `SessionEnded`」那一支，`src/session/observe.rs:91-101`）：

| 末条事件 | 显示 |
|---|---|
| `TurnEnded` / `RoundEnded` / `GoalCompleted` / `GoalStopped` | `停稳` |
| 缺配对的 `ToolCallStarted`（照 `pending_tool_calls` 的配对口径，`src/events.rs:878-909`） | `中断` |
| 其余（含只有 `SessionStarted` 的空会话） | `未收尾` |

- **纯派生，不加字段、不改事件流、不写 `SessionEnded`**；`Listing.ended: Option<StopReason>` 的
  类型可以不动，改的是**填入点**。
- 原来的四档 `stop_reason()` 与 `open_session()` 的「未结束」（`src/render/wording.rs:3459-3462`）
  随之退休，换成 `停稳` / `中断` / `未收尾` 三句新文案。
- `sessions stats` 的会话级 stops（`src/session/observe.rs:1009-1011`）**用同一条派生**，两个
  表面不给两套口径。
- 落点是 `observe.rs` 的派生 + `wording.rs` 的文案；`docs/` 里描述 `sessions ls` 的那句由票 09 回改。

### §7 不变量（票不得违反）

1. **已在宿主表里的会话绝不重走 `assemble`**：切回只是改前台指针（票 04 §2）。否则 `resuming`
   闸门（`src/lib.rs:349-356`）会把**正在跑**的工具调用读成崩溃遗留，给它合成一条「结果未知」。
   只有「不在宿主表里」的会话才走装载路径。
2. 同一个 `SessionId` 在宿主表里只有一行（票 04 §1）。
3. 关闭不写盘、不删盘；空宿主态下也不写。
4. `messages` 投影前缀与模型可见文本一个字节不动。

### §8 交出去的

| 内容 | 票 |
|---|---|
| 视图重建的触发点（新建 / 关闭 / 空宿主态各触发什么重建） | [08](08-front-view-isolation.md) |
| 文档回改：`docs/render.md` 的会话页小节、`sessions ls` 的说明、`/clear` 文案 | [09](09-doc-revision.md) |
| `/loop` 目标与多会话的关系（在跑的 loop 留在后台会话里） | 图上仍留着的雾（「两场会话同时推进同一个目标」） |

### 要改的既有行为（实现期落点）

- `clear_while_looping()` 那道闸门去掉（`src/cli.rs:1667-1678`）。
- `/clear` 的回执文案：`wording::cleared` 现在说「已结束会话 {from}…」（`src/render/wording.rs:3103-3105`），
  与「旧的不关」矛盾 —— 改成「新开一场，旧会话留在列表里」的口径。
- `Listing.ended` 的填入点从 `SessionEnded` 改成 §6 的末条事件派生（`src/session/observe.rs:91-101`）。
- `sessions stats` 的 stops 跟着走同一条派生（`src/session/observe.rs:1009-1011`）。
- 权限策略从脚手架级（`src/lib.rs:281`）变成一场一份（票 04 §1 的推论，本票只钉语义）。

**判据复核**：只产决策，无实现代码；模型可见文本与 `messages` 投影前缀一个字节不动；
plain / headless 渲染器不受影响（会话页只在 TUI，冻结项 12）。

**产物**：本票无独立资产；两份调研的事实已折进 §2/§3，`CONTEXT.md` 新增**空宿主**一条。

## 评论

### 2026-10-12：[票 32](32-close-interaction.md) 给「前台不许关自己」加了唯一例外

- **只剩一个会话时，前台可以关掉自己** → 空宿主态。§3 本来就写着「关闭可以关掉最后一场」，
  而 `close_check` 先拒前台，于是那条路必须先 `新建` 一个才能走通 —— 这是实现与本文的偏差，
  由票 32 改正。
- 关到最后一场**不打 notice**：页顶 `0 个`、页区 `没有会话`、输入区禁用已经在说同一件事。
- 「不二次确认」对最后一场同样成立。
