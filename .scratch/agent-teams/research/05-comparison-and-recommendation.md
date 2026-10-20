# agent teams：四种形态的横向结论与 heng 的选型

> **这是材料的收口，不是 spec。** 它把 [01 DSH](01-dsh-agent-teams.md)、[02 Claude Code](02-claude-code-teams.md)、
> [03 Codex](03-codex-multi-agent.md)、[04 其他各家](04-other-agents.md) 四份一手调研折成一张
> 对比表与一份建议。**结论要落地还得走 `/grill-with-docs`**（把 [seed](../seed.md) 折成 `spec.md`），
> 本文里带「建议」二字的都是待议的决策，不是既成事实。
>
> 调研日期：2026-10-20。上游事实以当天抓到的文档与源码为准。

## 0. 结论摘要

1. **上游有四种形态，主流是 A（父子委派）。** Gemini CLI、OpenCode、Cline、OpenHands、Goose、
   Amp、Codex 全是「父 agent 用一个工具派生子 agent，摘要回灌」这一种形状 —— heng 今天的
   **执行者（Executor）** 就在这一类。
2. **「平级 agent + 共享板」（形态 B）的一手先例很少，全是近一年的新东西**：Claude Code 的
   agent teams（实验性、默认关、**只对交互式会话生效**）、DSH 的 `dsh-experimental-agent-team`
   （实验性）、Cline 的 Agent Teams。三家都不是「主流路线」，而是在同一段时间各自撞上同一个需求。
   连 Anthropic 自己也承认 **agent 之间实时互相协调尚未解决**（2025-06 博客：LLM agents are not
   yet great at coordinating and delegating in real time）。
3. **原理层的共同骨架只有三件机制**：**可寻址的成员身份**、**消息通道（信箱）**、
   **共享状态（任务板）**。**没有一家把编排交给运行时调度器** —— 编排权都在模型手里
   （lead 用工具调），运行时只提供能力。DSH 说得最直白：「Lead 不是调度器，是普通参与者
   + 三项 lead-only 权限」。
4. **对 heng 的推荐**：形态取 Claude Code 的（信箱 + 共享任务板 + 成员面板），
   机制纪律取 DSH 的（**团队状态不进模型上下文**、任务板用 CAS 修订号、只有 lead 能建成员），
   寻址与同伴清单取 Codex 的（规范路径 + 名单直接进每个成员的上下文）。
   **但成员不是派生出来的子 agent，而是 heng 已有的一批同工作区会话** —— 这一条三家上游都没做，
   因为它们的成员都得由 lead 生成；heng 的会话本来就能由人开、能平铺、能在同一进程里并存。
5. **第一版要小**：只做「只读成员清单 + 共享任务板 + 会话间消息」，
   **写操作仍然走现有执行者**（heng 已有 `PathLocks` 做写排斥）；worktree 隔离后置。

## 1. 四种形态

| 形态 | 代表 | 原理 | 通信机制 | 共享状态落在哪 | 权限与隔离 | 人怎么看见 | 代价 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **A. 工具派生** | Gemini CLI、OpenCode、Cline、OpenHands、Goose、Amp、Codex、**heng 的执行者** | 父的工具调用创建一个子上下文；子跑完把最终报告作为工具结果回到父的上下文 | **走父的上下文**；子与子之间**没有通道** | 子自己的上下文 / 会话库（Gemini 官方写明待办「scoped to the current session」） | 多按「继承拒绝」实现；**无工作区隔离** | 父的 TUI 里只看到「派了 N 个、各自什么结论」 | 父看不到中间过程；子之间无法协调；写冲突要靠人或上层分配 |
| **B. 平级 + 共享板** | **Claude Code agent teams**、**DSH agent-team**、**Cline Agent Teams** | 成员从**共享板**上 claim 任务、互相发消息；lead 派活与收结果 | **不经过父上下文**：成员各自的信箱 | **一等持久化对象**：板 + 信箱 + 任务日志（Claude：三个目录；DSH：lead 日志上的投影；Cline：`teams.db`） | 成员有独立上下文与独立权限档；共享工作区 | 成员面板 / 看板 / 卡面（Claude 卡面只显示最新一条消息或工具调用） | 要设计板的语义、持久化、投递与唤醒；写冲突要另解决 |
| **C. 运行时调度** | OpenHands、Cline hub-spoke、Gemini A2A、Goose roaming、Amp orb | agent 拆成独立进程 / 容器 / 远程服务，调度层持有会话与路由 | **协议**（A2A / ACP / MCP / WebSocket） | 运行时状态，不是任务板；Goose 明说多机工作流「**without any shared state**」 | 强隔离（容器 / worktree / 独立 client） | 事件流、看板、远端 TUI | 进程 / 容器 / 网络基础设施；调试链路长 |
| **D. 同上下文角色分工** | aider 的 architect/editor、**heng 的讨论会话** | 同一段对话里换角色、换模型 | **就是父的上下文** | 无 | **零隔离** | 普通对话流 | 两次模型调用；角色边界靠提示词维持 |

**形态 B 为什么少**：它要额外造三样东西 —— 成员身份（谁在队里）、信箱（消息往哪投）、
共享板（状态怎么一致）。形态 A 只要一个工具调用。绝大多数 harness 先做 A，等真的撞上
「子与子需要商量」才补 B。三家形态 B 的实现恰好都在 2026 年前后出现，且都标着实验性。

## 2. 形态 B 的三家先例细看

| 维度 | Claude Code agent teams | DSH agent-team | Cline Agent Teams |
| --- | --- | --- | --- |
| 开关 | `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`，默认关；**只在交互式会话生效**（`-p` 与 Agent SDK 不派发 teammate） | 挂两个实验包（`dsh-experimental-agent-team` + `-tool-agent-team`） | 平台内建 |
| 成员身份 | lead 派生的独立 Claude Code 实例（in-process 或 tmux 分屏） | lead 的**直接子会话**（同进程、独立上下文、独立日志、可冷恢复） | 协调者 spawn 的成员，按 **session id 归档** |
| 团队作用域 | **一个会话一支队**（v2.1.178 起取消「建队」步骤，每会话一支隐含队）；跨会话共享一份任务列表靠 `CLAUDE_CODE_TASK_LIST_ID` | **一个 root 会话一棵树**（TeamId = lead 的 SessionId） | 一个 session id 一支队 |
| 消息载体 | `~/.claude/teams/{team}/inboxes/{agent}.json`（**每成员一个 JSON 文件**） | 收件方**自己的会话日志**，首块固定 `Team message from <name>:` | `~/.cline/data/db/teams.db` 里的 mailbox |
| 消息语义 | 纯文本，自动投递 | steer：running 目标在最近 step 边界收到，inactive 目标被**冷恢复**后收到 | `team_send_message` / `team_broadcast` / `team_read_mailbox` |
| 共享任务板 | `~/.claude/tasks/{team}/`；状态 pending / in progress / completed + 依赖；**claim 用文件锁**防竞态 | **lead 会话日志上的事件溯源投影**；CAS 靠 `expected_revision`，过期报 `TEAM_TASK_STALE_REVISION` | task board + mission log，同一份 `teams.db` |
| 寻址 | 成员**名字**；另有一套路径式名单 | 成员**名字**（`target`），不是 Session id | session id |
| 谁能建成员 / 中断 | **只有 lead** | **只有 lead** | 只有协调者能关成员 |
| 团队状态进不进模型上下文 | 板与名单是模型**调工具查**的，不常驻 | **不增加任何 token**（roster / task 变化零 token，只有 peer 消息进收件方历史） | 成员要看板得调工具 |
| 可见性 | 提示框下方的成员面板（上下键选人、Enter 看转录）；或 tmux / iTerm2 分屏 | Web 投影 `agentTeam`（成员身份、阶段、任务视图） | 看板卡面 + 详情页 |
| 已知代价 | 官方自陈**九条限制**（`/resume` 不恢复 in-process teammate、任务状态滞后、一会话一队、不能嵌套、lead 固定、权限在 spawn 时定死、分屏要 tmux/iTerm2、in-process teammate 不能跑后台 subagent、关闭慢）；**teammate 无硬上限**（建议 3–5，成本线性增长）；**lead 没有阻塞等待原语** | 共享 checkout、**无文件锁**（`write_scopes` 只是提示）；**消息无重试无去重无回执**；**owner 不自动释放**；失败的成员永久占名字 | 与 Cline Kanban（每任务一个 ephemeral worktree）配套才有隔离 |

**三家的共同点比差异更值得记**：成员**由 lead 生成**（不是人开的）、板与信箱**是独立于会话
的一份存储**、只有 lead 有治理权、**没有一家提供跨进程协调**（Claude 有跨会话消息但那是另一条
产品线，DSH 明说「不支持跨进程」，Cline 是库内）。

## 3. 三个必答问题

### 3.1 通信：往哪投，什么时候被看见

| 问题 | 上游答案 |
| --- | --- |
| 消息落在哪 | Claude：每成员一个 JSON 文件；DSH：收件方**自己的会话日志**（于是可搬运、可重放）；Cline：`teams.db` |
| 收件方怎么被唤醒 | Claude：运行中的会话在**工具调用之间**读到（不打断正在跑的工具），空闲则开新一轮；DSH：running 目标在最近 step 边界收到，inactive 目标被**冷恢复**后收到；Cline：成员自己轮询 / await |
| 「留言」与「派活」是一件事吗 | **Codex 明确拆开**：`send_message` 只投邮箱不唤醒（QueueOnly），`followup_task` 才唤醒并开新一轮（TriggerTurn）。Claude 的 `SendMessage` 是后者（带 `notify_when_idle` 订阅时例外） |
| 送达有没有承诺 | 基本没有：DSH **无重试、无去重、无回执**；Claude 只保证「写进收件人信箱文件成功」，且 **v2.1.224 之前写失败也报成功**；Codex 的 mailbox 队列在驱逐时保留待收邮件 |

### 3.2 共享状态：落在哪，为什么不落在会话流里

- 形态 A 的待办几乎都落在**单个会话的上下文或内存**里（Gemini 官方明写「scoped to the
  current session」；opencode 的 `todowrite` 默认对 subagent 关闭）。
- 形态 B 才把板当**一等持久化对象**：Claude 三个目录（队配置 / 任务 / 信箱）、DSH 挂在 lead
  的日志上、Cline 一个 `teams.db`。
- **「板进会话流」与「板是多写者」正面相撞** —— 这是 heng 选型时最硬的一条约束（见 §5.3）。

### 3.3 可见性：人怎么看见一支队伍

- Claude：提示框下方的成员面板（空闲行 30 秒后折叠、超过三个折叠成 `N idle agents`），
  `Enter` 进转录并直接给它发消息；分屏模式是 tmux / iTerm2。
- DSH：Web 侧读共享投影 `agentTeam`（成员身份与阶段、成员错误、未删除任务视图）。
- Cline：卡面**只显示最新一条消息或工具调用**，详情页才给完整 TUI + diff。
- 共同教训：**不要并排渲染 N 份完整流**（Cline 的做法）；**任务板要显示 owner 与就绪状态**
  （DSH 的 `task-view` 专做这件事）。

### 3.4 收益到底有多大：唯一一组可引用的量化

Anthropic 的多 agent 研究系统（[How we built our multi-agent research system](https://www.anthropic.com/engineering/multi-agent-research-system)，
2025-06-13）是上游唯一给出可引用数字的地方：

| 事实 | 数字 | 边界 |
| --- | --- | --- |
| 多 agent（Opus 4 当 lead + Sonnet 4 当子 agent）比单 agent（Opus 4）在**内部研究评测**上高 | **90.2%** | 是**研究类 breadth-first 查询**，不是编码任务、不是 SWE-bench、不是公开评测集 |
| 多 agent 系统比普通对话多用 token | **约 15×**（单 agent 约 4×） | 官方原话：「多 agent 主要靠<b>多花 token</b> 来解决问题」 |
| BrowseComp 上性能方差的解释力 | **token 用量单独解释 80%** | 另两个因素是工具调用数与模型选择 → 收益基本是「token 的平行化」 |
| 并行化（lead 并行 3–5 个子 agent + 子 agent 并行 3+ 工具）对复杂查询的耗时削减 | **最多 90%** | 墙钟时间，不是结果质量 |

**同一篇文章自己写下的三条限制**（比数字本身更重要）：

- 「**大多数编码任务可并行的部分比研究少得多**」；
- 「LLM agent 目前**还不擅长**实时地相互协调与委派」；
- 同步执行会造成瓶颈（lead 干等 → 无法中途引导子 agent、子 agent 之间无法协调）。

**反向证据**：mini-SWE-agent（只有 bash 一个工具、无 subagent、无 todo、无 fork）拿到
>74% SWE-bench verified —— 「不做多 agent 也能很强」的最好证据（[既有横向笔记 §15](../../../docs/research/coding-agent-features.md)）。

**Claude Code 自己的口径**：teammate 在 plan mode 下约 **7× token**；建议 **3–5 个**起步；
明确写「顺序型任务、改同一批文件、依赖很多的活，单会话或 subagent 更合适」。

**结论**：**「多 agent 显著更好」只在很窄的一组条件下成立** —— 任务能高度并行且分支独立、
信息量超单个上下文、任务价值高到吃得下 15× token。形态 B 相对形态 A 的**增量**因此是：
**上下文隔离 + token 的平行化 + 成员之间的交叉检查**；不是「一群人更聪明」。

## 4. heng 今天有什么、缺什么

| heng 现有机制 | 对应上游形态 | 差在哪 |
| --- | --- | --- |
| **执行者（`task` 工具）** | 形态 A | 一次工具调用即生命周期；**无可寻址名字**、不能被追问、活不过父会话；深度一（工具表里没有 `task`） |
| **讨论会话**（一条流多发言人） | 形态 D | 共享上下文；角色是配置不是结构 |
| **多会话并存**（宿主表 + 前台指针 + 平铺格） | heng 独有 | **会话之间没有通道**，也没有共享待办 |
| **会话 = 一个可搬运目录**（JSONL 事件流 + `outputs/`） | — | 跨会话消息往哪儿落，与这条形状正面相撞 |
| **`PathLocks`**（共享路径锁） | 比上游都强 | DSH 明确拒绝把「写作用域」当锁（外部写入会绕过、崩溃的 owner 永久占住）；**heng 是真锁** |
| **`SessionTurn::Injected`**（注入驱动） | — | 已有的「本轮由别处注入」入口，团队消息唤醒可以挂在它上面 |
| 权限门 + 规则表达 | — | 已能表达「派发者的子集」「继承拒绝不继承放行」 |

## 5. 建议方案：工作区作用域的平级团队（形态 B'）

**一句话**：把「同工作区的一批会话」变成一支有名字、有信箱、有共享任务板的队伍，
**成员是人开的会话，不是 lead 派生的子 agent**。

### 5.1 成员身份

- **谁在队里**：同一个**会话桶 / 检出**下活着的会话（heng 已有宿主表这个事实源）。
  「同目录」按桶还是按检出，仍是待议（seed 分叉 1）；建议**按检出**（跨 worktree 也算一队），
  因为 Claude 的 worktree 隔离是 chat 级的、Codex 的 worktree 与多 agent 无关。
- **寻址**：成员**名字**（Claude、DSH、Cline 都用名字，不用 Session id），名字唯一且不复用
  （DSH 明说「名字永不复用」）。
- **同伴清单进上下文**：每个成员知道自己有哪些同伴（Codex 的做法：环境上下文里注入
  `- task_3: Herodotus`）。heng 的做法可以是注入一段「队友名单 + 各自在做什么」。

### 5.2 消息通道

- **两个动作，不是一个**（Codex 的 `send_message` / `followup_task` 之分）：
  - **留言**（投信箱，不唤醒）：收件方在下一个回合边界读到。
  - **派活**（唤醒，开新一轮）：收件方空闲时立刻起一轮新的。
  这条正好接住 seed 分叉 3（排到边界 / 立刻打断 / 只在空闲时投递）。
- **落在哪**：建议**收件方自己的会话日志**（DSH 的做法）—— 好处是消息可搬运、可重放、
  进上下文是自然投影；代价是**发送方的流里没有副本**，人的可见性要另做（见 5.6）。
  备选是另开团队级 mailbox 目录（Claude 的做法），代价是消息与会话目录分离。
- **送达语义**：**不承诺送达**（DSH、Claude 都是这样），但要有**上限**（Claude 限 100 条待收、
  Codex 限队列；DSH 限 16 名成员 / 256 个任务 / 单条 64 KiB）。
- **投递成功要有可核验的判据**（Claude 的教训：v2.1.224 之前**写失败也报成功**）：
  建议事件流上落两条 —— `MessageSent`（我发了）与 `MessageDelivered`（**进了收件方的流**），
  两者的差别就是「投递成功」的凭据。
- **headless 要另想**：Claude 的 teams **只对交互式会话生效**（`-p` 与 Agent SDK 都不派发
  teammate）。heng 有 headless 适配器，所以「无人值守时团队还在不在」是一个独立决策
  （Claude 的答案是「不在」）。

### 5.3 共享任务板

- **不要塞进某个会话的流**（heng 的会话是单写者只追加，板是多写者）。
  建议**团队级目录**（例如 `~/.config/heng/teams/<team>/`），里面：
  - `roster.json` —— 成员名单与状态；
  - `tasks.jsonl` —— 追加式任务日志 + 每次变更的 `revision`；
  - `mailbox/` —— 若走 5.2 的备选方案。
- **CAS 语义照 DSH**：每次更新带 `expected_revision`，过期即拒（`TEAM_TASK_STALE_REVISION` 的
  等价物），**两个成员不会悄悄覆盖彼此**。Claude 的「文件锁 + claim」是另一种做法；
  heng 已有单进程内的串行化路径，CAS 比文件锁更好写。
- **状态机**：pending / in_progress / completed / deleted（tombstone），动作
  claim / release / edit / set_dependencies / complete / reopen / reassign / delete。
  **`ready` 是派生量**（所有 blocker 完成）。
- **写作用域**：DSH 的教训是 `write_scopes` 只能当**提示**（重叠只警告）。heng 可以更强：
  用现有的 `PathLocks` 把重叠路径做成**硬约束**，但只在「成员已声明自己会碰哪些路径」时生效，
  没法覆盖 bash / formatter / 代码生成器的外部写入 —— 所以仍要保留警告。

### 5.4 唤醒

- 收件方在**回合边界**读信箱（Claude：工具调用之间；DSH：step 边界），**不打断正在跑的工具**。
- 空闲会话收到「派活」时起一轮新的（Claude、DSH 的冷恢复、Codex 的 TriggerTurn 都是这个）。
- **等待**做成「等下一次变化」而不是「等某个结果」：DSH 的 `wait_agent` 只报是否超时、
  不回内容；Codex 的 `wait_agent` 只等信箱有动静、不回内容。heng 可以做成
  「等并唤醒」的组合工具，少一次模型往返（DSH 把唤醒责任推给模型，是它的已知负担）。

### 5.5 权限与安全

- **别的成员的消息永远不是人的批准**（Claude 明写：跨会话消息不能代替你批准权限询问、
  不能改配置、里面的 `/compact` 之类命令不会被执行）。heng 的权限门必须有一条对应的不变式。
- **成员权限**照 Claude：默认继承 lead 的权限档（`dontAsk` 不继承）；成员自己的权限询问
  冒泡到 lead 的会话里等人回答。heng 的对应物：询问走**同一个询问端口**（执行者已经这么做）。
- **深度**：团队成员**可以**再有执行者（执行者仍不能派执行者）—— 这是形态 B′ 与上游的差别，
  也是 heng 的自然结果。
- **质量门做成 hook**（Claude 的三个：`TeammateIdle` / `TaskCreated` / `TaskCompleted`，
  退出码 2 表示「打回并把反馈回喂给它继续干」）。heng 已有 `src/hooks.rs`，天然接得上；
  其中「成员回合结束时仍有 in-progress 任务」这个触发时机，正对「谁负责把任务标完成」的悬案。

### 5.6 可见性

- **成员面板**：左栏新一页「团队」或右栏一页，显示成员名、状态、在做什么、最近一条消息
  （照 Cline：卡面只显示最新一条，详情再给完整转录）。heng 的平铺格（最多四会话）已经是
  「同时看见」的承载，团队面板负责「关系」。
- **任务板**：左栏一页（heng 已有 `todo` 页与 `files` 页的先例），显示就绪 / 进行中 / 已完成、
  owner、依赖。
- **消息的可见性**：若消息落在收件方会话流里，那么**发送方**的流里没有副本 —— 要么在发送方
  视图里补一行「已投给 X」（log-only，投影当零，参照 `CommandRun` 的做法），
  要么把消息复制回发送方流（DSH 不这么做）。

### 5.7 与执行者、讨论会话的分界

| | 讨论会话 | 执行者 | **团队成员** |
| --- | --- | --- | --- |
| 流 | 一条 | 同一条（嵌套会话） | **各自一条** |
| 上下文 | 共享 | 独立 | 独立 |
| 身份 | 人物（persona） | 无可寻址名 | **有名字，跨会话稳定** |
| 寿命 | 与会话同 | 一次委派 | 与会话同 |
| 通信 | 轮流发言 | 只回摘要 | **成员之间直接** |
| 共享状态 | 无 | 无 | **共享任务板** |

## 6. 第一版明确不做

- **不做 worktree 隔离**（DSH 明说它不提供；Claude 的 split-pane 也不隔离文件；
  Cline Kanban 用 ephemeral worktree 是另一条产品线）。
- **不做跨进程 / 跨机协调**（DSH 明说不支持；Claude 的跨会话消息是另一条产品线，且有认证与
  入站三档的复杂度）。
- **不把写操作交给团队成员**（写仍走执行者，共享 `PathLocks`）。
- **不做嵌套团队**（Claude、DSH 都不做）。
- **不把板做成「进模型上下文」的常驻清单**（DSH 的零 token 纪律）。

## 7. 上游踩过的坑（照抄的教训）

1. **成员永不释放 owner**：DSH 明说成员不活动、被中断、进程退出、工作失败都**不释放任务
   owner** → 任务永远 in_progress，依赖它的任务永远卡住。heng 需要一条「成员消失时释放其
   owner」的规则。
2. **失败的成员永久占名字与槽位**（DSH）→ 名字不复用是对的，但要有回收策略。
3. **任务状态可能滞后**（Claude 明写：teammate 有时不把任务标记完成，挡住依赖任务）→
   board 需要一个「按实际产物核对」的手段，或允许 lead 强制改状态。
4. **没有送达保证**（DSH 无重试无去重）→ 消息是**尽力而为**，重要的事走任务板而不是消息。
5. **消息回环**（Claude 明写：消息回环被限速、重复消息被丢弃、排队上限 50）→
   heng 要有同类的护栏。
6. **在途恢复**（Claude 不许 `/resume` 恢复 in-process teammate；DSH 靠 child 独立持久化的
   会话做对账）→ heng 的会话本身就是可搬运目录，恢复天然比上游容易。
7. **lead 没有阻塞等待原语**（Claude 只能靠「成员空闲通知」+ 自然语言提示）→ heng 若不补一个
   「等成员」的动作，模型会反复空转烧回合（DSH 的 `wait_agent` 也把唤醒责任推给模型）。
8. **官方自己承认这事没解决**：Anthropic 2025-06 明说「LLM agents are not yet great at
   coordinating and delegating in real time」—— heng 也不该把「团队一定能协同」写进预期。
9. **投递成功必须可核验**（Claude v2.1.224 之前写失败也报成功）→ 事件流上分
   `MessageSent` / `MessageDelivered` 两条。

## 8. seed 待谈分叉的初判（供 grilling 用，非结论）

| seed 分叉 | 初判 | 依据 |
| --- | --- | --- |
| 1. 「同目录」按桶还是按检出 | **按检出** | worktree 隔离是 chat 级的（Codex），跨 worktree 同一检出更接近「同一个团队」 |
| 2. 消息落在哪 | **收件方自己的会话流**；板另开团队级目录 | DSH 的「收件人拥有收到的消息」；heng 单写者只追加的流与多写者的板正面相撞 |
| 3. 收件方怎么被唤醒 | **两个动作**：留言（不唤醒）/ 派活（唤醒）；收件方在回合边界读 | Codex 的 QueueOnly vs TriggerTurn；Claude 的「工具调用之间读」 |
| 4. 团队形态；执行者要不要升格 | **不升格**：执行者是「一次委派」，团队成员是「一支队伍里的同事」；成员可以有执行者，执行者不能当成员 | 形态 A 与 B 的分工；Codex 的「done ≠ released」 |
| 5. 人怎么看见 | 左栏「团队」页（成员 + 任务板）+ 平铺格 | Cline 卡面只显示最新一条；heng 已有左栏多页与平铺 |
| 6. 与讨论会话的分界 | 一份流多角色 vs 多份流 | 形态 D 与 B 的分工 |
| 7. 要不要「这个目录下现在有谁」 | **要**，且与左栏会话页分开：会话页列盘上跑过的，团队页列**活着的成员** | 宿主表与 store 的分工已经是这个形状 |

## 9. 来源

- 本目录：[01 DSH](01-dsh-agent-teams.md)、[02 Claude Code](02-claude-code-teams.md)、
  [03 Codex](03-codex-multi-agent.md)、[04 其他各家](04-other-agents.md)。
- Claude Code 官方文档（2026-10-20 抓取）：
  [agent teams](https://code.claude.com/docs/en/agent-teams.md)、
  [cross-session messaging](https://code.claude.com/docs/en/cross-session-messaging.md)、
  [subagents](https://code.claude.com/docs/en/sub-agents.md)、
  [interactive mode](https://code.claude.com/docs/en/interactive-mode.md)、
  [Managed Agents 多 agent 编排](https://platform.claude.com/docs/en/managed-agents/multiagent-orchestration.md)。
- 本仓库既有材料：[`seed.md`](../seed.md)、[`docs/executor.md`](../../../docs/executor.md)、
  [`docs/discussion.md`](../../../docs/discussion.md)、
  [`fs-agent-v1` spec §16](../../../.scratch/fs-agent-v1/spec.md)。
