# DSH 的 Agent Teams：形态与原理

> **调研日期：2026-10-20**。
> **一手源**：`/usr/lib/node_modules/@deepseek-ai/dsh/`（npm 发行包，含编译后的 `lib/*.js` 与
> `README.zh.md`）。本文所有 `文件:行号` 都指这个目录树下的文件，行号经 `grep`/读文件核对过。
> 下文把包前缀简写为 `PKG = /usr/lib/node_modules/@deepseek-ai/dsh/node_modules/@deepseek-ai/`。
> **补充源**：公共仓库 `https://github.com/deepseek-ai/deepseek-harness` 的
> `docs/subsystems/agent-team.zh.md` 与 `.agents/notes/implemented/feature/2026-08-05-agent-teams.zh.md`
> （该仓的 `docs/` 与 `.agents/` 不在 npm 包里，本文在第 6 节列出 URL）。
> **范围**：只研究 DSH 自己的 Agent Teams（实验性），不评价好坏，也不提可移植性之外的意见。
> **本地笔记没有修改。**

---

## 0. 结论摘要

1. **DSH 的 team 是一棵树，不是一张图。** 团队身份就是 Lead 的 `SessionId`：每个普通运行时
   root 都是一个「隐式 Team」的 Lead，没有 creation event，Lead 那行由身份直接存在
   （`PKG/dsh-experimental-agent-team/lib/index.js:509-518`、`:684-703`）。
   teammate 是 **Lead 的直接 continuable 子会话**（child Session），不是独立进程、不是平级会话。
   只有 Lead 能创建和中断 teammate（`lib/index.js:1006`、`:965`）。

2. **teammate = 一个独立的 Agent + 一条独立的 Session 日志 + 一个可冷恢复的 activation。**
   它和 Lead 共享进程、共享工作目录、共享 cwd，但有**自己的上下文、自己的回合、自己的日志**
   （`PKG/dsh-subagent-spawn-in-process/README.zh.md:12`；`lib/index.js:1032-1042` 用
   `ctx.subagents.startActivation({ delivery: "parent", childId, provider, request: { prompt, parent: root } })`）。

3. **通信走「目标会话自己的 inbox」，不另设一套队列。** `send_message` 是 **steer**：
   running 目标在最近的 step 边界收到，inactive 目标被启动或**冷恢复**（从持久化 Session 重建
   activation）后收到（`lib/index.js:1604-1611`；`PKG/dsh-subagent/lib/types/manager.js:515-517`、
   `:1171-1214`）。消息体第一块固定是 `Team message from <name>:`（`lib/index.js:1593-1596`）。
   **没有 outbox、没有重试、没有去重、没有送达回执**（`lib/index.js:1587-1614`）。

4. **共享任务板是 Lead 会话日志上的事件溯源投影，不是独立存储。** `team/member` 与
   `team/task` 事件追加到 **Lead 那一条 Session 日志**上，先 flush 再报成功
   （`lib/index.js:156-160`、`:1025-1029`、`:1375-1379`）。任务状态机是
   `pending → in_progress → completed / deleted`，每次变更都是 **CAS**：必须带
   `expectedRevision`，过期直接报 `TEAM_TASK_STALE_REVISION`（`lib/index.js:1278`）。
   `writeScopes` 只是**建议性**路径前缀，重叠只产生警告，**绝不阻止 claim 或授予写权限**
   （`lib/index.js:345-347`、`:366-385`）。

5. **Lead 不是运行时调度器，运行时也不替 agent 编排。** 编排全靠 **Lead 这个模型**调九个工具；
   运行时只提供机制（消息通道、任务板、activation 生命周期）并在工具执行时强制权限
   （`PKG/dsh-experimental-tool-agent-team/lib/index.js:22-30`、`:531-554`）。
   Lead 与 teammate 拿到**同一套九个 schema**、同一段策略文本；差别只在执行时检查角色。

6. **并发是真的：teammate 与 Lead 同进程并行。** 每个 teammate 是自己的 agent loop，
   Lead 调完 `spawn_teammate` 立刻返回，不等结果；`wait_agent` 只等「下一次变化」，
   **永不唤醒 inactive 成员**（`tool-agent-team/lib/index.js:329-350`）。

7. **团队状态不进模型上下文。** roster 与任务板的变化**不增加任何 token**；
   只有 peer 消息进入收件方的历史（`PKG/dsh-experimental-agent-team/README.zh.md:186-188`；
   `PKG/dsh-experimental-tool-agent-team/README.zh.md:131`）。这条对 heng 极有价值。

8. **对 heng 最有参考价值的三件：**
   (a) **把「派出去」和「等结果」拆成两个工具**（`spawn_teammate` / `wait_agent`），
       Lead 因此不必被阻塞，模型可以在等待期间干别的；
   (b) **任务板 = 会话日志上的可重放投影 + CAS 修订号**，不另开一份数据库；
   (c) **团队状态与模型上下文分离**（状态在日志里，模型要用就调工具查）。

9. **两个明显的坑：**
   (a) **共享 checkout，没有文件锁。** 所有成员用 Lead 的当前目录，改动立刻互相可见；
       `write_scopes` 纯建议；Bash / formatter / codegen 绕过一切版本护栏。
       DSH 明说「写作用域提示是 advisory，不是锁」（`tool-agent-team/lib/index.js:25`）。
   (b) **没有任何失败恢复承诺。** 消息无重试、owner 不自动释放、失败的 teammate 永久占名字与
       槽位、`wait_agent` 在没人跑时立刻返回 `noProgress`（模型容易空转烧回合）。

---

## 1. 形态：有哪些实体、哪些工具

### 1.1 团队身份：隐式 Root Team

- 每个普通运行时 root 都是一个隐式 Team 的 Lead，**Team id 等于该 root 的 `SessionId`**
  （`PKG/dsh-experimental-agent-team/README.zh.md:126`；代码：`lib/index.js:267-269` 的
  `TeamId(id)` 只是品牌类型，`lib/index.js:894-899` 把 root 自己返回为 `{ role: "lead", name: "lead" }`）。
- **没有 creation event**。Lead 那行是伪行，由身份直接存在；持久状态从第一条 member / task 事件开始
  （`lib/index.js:509-518` 的 `emptyTeamState`；投影端 `lib/index.js:684-703` 的 `buildTeamProjection`
  永远先塞一行 `{ id, name: "lead", role: "lead", phase: "active" }`）。
- **Team 是树**：成员解析看的是 `agent.session.header.parentSession`
  （`lib/index.js:870-903` 的 `tryMembership`）。父活着且自己是 roster 里的成员 → teammate；
  自己是 provider 管的普通 subagent → **不是成员**；否则 → 自己是另一个 Team 的 Lead。
  普通 fork 是**新 Root**，继承来的 Team 记录因 ancestor `TeamId` 不同而被过滤掉
  （投影过滤见 `lib/index.js:572`：`if (selector.teamId !== state.id) return state;`）。
- roster **扁平且不可变**：只有 Lead 能建直接 teammate；不支持嵌套 Team、重命名、删除、名字复用
  （`PKG/dsh-experimental-agent-team/README.zh.md:205`）。

### 1.2 teammate 是什么实体

一句话：**同一进程内的一个独立 Agent 实例 + 一条独立 Session 日志 + 一个可冷恢复的 activation**。

| 维度 | teammate | 依据 |
| --- | --- | --- |
| 进程 | 同一个进程（`spawn` / `fork` 都是 in-process 提供方） | `PKG/dsh-subagent-spawn-in-process/README.zh.md:12`；`PKG/dsh-subagent-fork-in-process/README.zh.md:12` |
| 会话 | 独立 child Session（`parentSession` 指向 Lead） | `lib/index.js:1040`（`request: { prompt, parent: root }`） |
| 上下文 | 独立的；`fresh` 完全不带 Lead 历史，`fork` 只带 Lead **已完成**的 turn 前缀 | `lib/index.js:287`（provider 选择）；`PKG/dsh-subagent-fork-in-process/README.zh.md:36` |
| 工作目录 | **共享 Lead 的当前目录**；后续目录变更只影响各自 Session | `PKG/dsh-experimental-agent-team/README.zh.md:203`；Agent Note「所有 member 创建时使用 Lead 的当前目录，并立即观察共享文件系统写入」 |
| 生命周期 | durable：名字、provider、context、phase 都进 Lead 日志；冷恢复会重建 activation | `lib/index.js:1112-1146`（`reconcileProvisioning`） |
| 深度 | 只能一层（teammate 不是 Lead，不能再 spawn teammate） | `lib/index.js:1006`（`only the Team Lead can create teammates`） |

teammate 的 `id` 是**预留的随机 UUID**（`lib/index.js:1012`：`brandString(randomUUID())`），
不是 Session id 本身——它是「roster 行 id」，在 activation 建立后成为 child session id。

### 1.3 roster 的四个状态

roster 行有两组状态，**必须分开看**：

- **持久 phase**（写进 Lead 日志，只走 `provisioning → active | failed`）：`lib/index.js:444-456`
  （`teamMemberSnapshotSchema`）、`:591-607`（投影端校验 phase 迁移合法）。
- **运行态 status**（现算，不写盘）：`running` / `inactive`
  —— `lib/index.js:1188-1190`：`availability(agent)` 只看 `agent?.status === "running"`。

模型看到的 `status` 是二者的合成（`lib/index.js:920-934`）：
`phase === "failed"` → `failed`；`phase === "provisioning"` → `provisioning`；否则取 `availability(live)`。

**`inactive` 不表示任务完成、失败或等待**（`tool-agent-team/lib/index.js:29` 的策略文本；
`PKG/dsh-experimental-tool-agent-team/README.zh.md:64`）。它只表示「当前没有回合在执行」，
包括「已加载但空闲」和「只有存储、需要冷恢复」两种。

### 1.4 九个工具的完整 schema

全部注册在 **member scope**（每个 Agent 自己的 `ctx` 上），Lead 与 teammate 完全相同
（`tool-agent-team/lib/index.js:531-554` 的 `maybeInstall` 对每个 `tryMembership` 成功的 Agent 安装）。

| 工具 | 参数 | 返回 | 谁能调 | 行号 |
| --- | --- | --- | --- | --- |
| `spawn_teammate` | `name`（lower-kebab-case）、`description`、`prompt`、`context?: "fresh"\|"fork"` | `{ member: MemberView }` | **仅 Lead** | `:250-291` |
| `send_message` | `target`、`message` | `{ sent: true }` | 任何成员 | `:292-319` |
| `list_agents` | 无 | `MemberView[]` | 任何成员 | `:320-328` |
| `wait_agent` | `timeout_ms?`（10000–3600000，默认 30000） | `{ timedOut, noProgress? }` | 任何成员 | `:329-350` |
| `interrupt_agent` | `target` | `{ previousStatus: "running"\|"inactive" }` | **仅 Lead** | `:351-363` |
| `team_task_create` | `subject`、`description`、`blocked_by?`、`write_scopes?` | `TaskView` | 任何成员 | `:364-398` |
| `team_task_list` | `status?`、`owner?`、`ready?`、`cursor?`、`limit?` | `{ tasks, nextCursor? }` | 任何成员 | `:399-442` |
| `team_task_get` | `task_id` | `TaskView` | 任何成员 | `:443-455` |
| `team_task_update` | `task_id`、`expected_revision`、`action`、以及动作相关字段 | `TaskView` | owner / Lead（`reassign` 仅 Lead） | `:456-521` |

**成员行的形状**（`tool-agent-team/lib/index.js:38-74`）：
`target`（= 成员**名字**，不是 Session id）、`role`（`lead`\|`teammate`）、
`status`（`running`\|`inactive`\|`provisioning`\|`failed`）、`description`、`provider`、
`context`（`fresh`\|`fork`）、`model`、`diagnostics`。
`target` 就是模型用来寻址的句柄（`spawn_teammate` 与 `list_agents` 返回它，
`send_message` / `interrupt_agent` / 任务的 `owner` 都用它，`tool-agent-team/lib/index.js:296-300`）。

**任务行的形状**（`:84-135`）：`id`（`task-<n>`）、`revision`、`subject`、`description`、
`status`（`pending`\|`in_progress`\|`completed`\|`deleted`）、`ownerName?`、
`blockedBy[]`、`writeScopes[]`、`ready`、`writeScopeWarnings[]`。

**`team_task_update` 的八个 action**（`:470-484`）：
`claim`、`release`、`edit`、`set_dependencies`、`complete`、`reopen`、`reassign`、`delete`。

### 1.5 共享策略与身份注入

每个成员拿到两样东西：

1. **一段固定的 system policy**（`team:policy` 段），原文见
   `tool-agent-team/lib/index.js:22-30`。它讲清：共享 cwd 与共享文件系统、切分写作用域、
   `FS_STALE_VERSION` 后要重读并 rebase、Bash/formatter/codegen 不受版本护栏保护、
   `send_message` 的语义、`wait_agent` 只观察不唤醒、Lead 必须等完才能给最终答案。
2. **一段 required runtime context**（`team:identity` 段），运行时按当前 membership 现算
   （`:239-249`）：

   ```
   You are the Team Lead named "lead".
   ```
   或
   ```
   You are teammate "<name>".
   Your Team Lead is named "lead".
   To message your Team Lead, use send_message({ target: "lead", message: "..." }).
   Use list_agents({}) to find your teammates and their names.
   To message another teammate, use send_message({ target: "<teammate name>", message: "..." }).
   ```

   身份**不在初始任务里**，`spawn_teammate` 原样发送 prompt（`PKG/dsh-experimental-tool-agent-team/README.zh.md:28`）；
   身份由 loop 的 runtime-context 投影承载，压缩移除快照后会恢复
   （Agent Note `2026-10-13-team-identity-runtime-context.zh.md`，见第 6 节 URL）。

---

## 2. 原理：机制怎么跑

### 2.1 `spawn_teammate`：两段式，先落日志再启动

`lib/index.js:1004-1074`（`spawnAdmitted`）：

1. **校验**：`name` 必须匹配 `/^[a-z0-9]+(?:-[a-z0-9]+)*$/u`、≤64 字符、不能叫 `lead`
   （`:816`、`:1163-1166`）；`description` 非空 ≤200 字符（`:796-801`）。
2. **第一段事务**（`:1021-1030`）：在 Lead 的 `journal.transact(root.id, ...)` 里
   查重名（`TEAM_MEMBER_NAME_TAKEN`）、查上限（`TEAM_MEMBER_LIMIT`，默认 16，`:1420`），
   然后 `appendAndFlush(root, "team/member", { version: 2, teamId, member: { phase: "provisioning" } })`
   —— **append 之后立刻 flush**（`:156-160`）。
3. **第二段**：调 `ctx.subagents.startActivation(...)` 起一个 continuable child
   （`:1032-1042`），provider 由 `context` 决定（`fresh` → `spawn`，`fork` → `fork`，
   `tool-agent-team/lib/index.js:287`）。
4. **checkpoint 初始 prompt**（`:1076-1110`）：循环 flush 子会话、监听 `session/event`，
   直到初始 user message 真的落在子会话历史里（`messageAccepted`，`:783-785`）才继续。
   **这一步意味着 `spawn_teammate` 不是瞬时返回的**，它等到了持久化。
5. **结算**（`:1059-1073`、`:1168-1181`）：再 append 一条 `team/member`，
   phase 变 `active`；起不来则追加 `failed` 并附错误文本（`:1044-1058`）。

**失败即终态**：名字被永久占用，槽位被永久占用（`PKG/dsh-experimental-agent-team/README.zh.md:60`、`:206`）。

**恢复对账**（`:1112-1146`）：进程重启后，Lead root 起来时会把未终结的 `provisioning` 记录
与**子会话自己持久化的日志**对照 —— 要求「直接 parent 匹配 + continuable descriptor 匹配 +
provider 匹配 + 初始 user message 已记录」四项全中才算 `active`，否则 `failed`
（`:1120-1125`）。这是 DSH 不用在 Team 日志里重建初始 prompt 的原因。

### 2.2 `send_message`：目标 inbox + steer

`lib/index.js:1587-1614`（`sendAdmitted`）：

```
membership = roster.membership(caller)          // 精确的 live Agent 是权限凭据
target = resolveActiveMember(root, state, request.target)   // 名字 → 持久 id；找不到报 TEAM_MEMBER_NOT_FOUND
if (target.id === caller.id) throw TEAM_SELF_MESSAGE
content = [{ type: "text", text: `Team message from ${membership.name}:` }, ...request.content]
if (byteLength(JSON.stringify(content)) > maxMessageBytes) throw TEAM_MESSAGE_TOO_LARGE   // 默认 65536
source  = { kind: "agent-message", form: "relay", senderSessionId: caller.id }
if (target.id === root.id)  root.steer(createUserMessage({ content, source }))     // 投给 Lead
else                         messageId = await steerHostSubagentPrompt(ctx.subagents, root, target.id, content, source, signal)
this.activity.notify(membership.id)      // 唤醒同队等待者
return { messageId }
```

关键点：

- **走的是既有 Agent inbox**，不是 Team 自己的队列。`steerHostSubagentPrompt` 只是
  `runtime[Symbol.for('dsh.subagent.deliverPrompt')](..., 'steer')` 的一层壳
  （`PKG/dsh-subagent/lib/types/internal.js:31`、`:55-57`）。
- **steer vs followup**：`steer` 进 `next-step` 队列，`followup` 进 `next-turn` 队列
  （`PKG/dsh-agent-loop/lib/index.js:800-814`）。Team **只用 steer**
  （Agent Note：`send_message 对 running、idle 和 cold target 使用 Steer`）。
  语义：running 目标在**最近的 step 边界**收到；idle 目标开一个新 turn；cold 目标先冷恢复再收。
- **收件方什么时候读？** 不是「空闲时读」，也不是「被打断」。是**轮次边界**：loop 在每个 step
  前 `inbox.claim(target, position.turn)` 把 pending 输入取出来拼进请求
  （`dsh-agent-loop/lib/index.js:902-927`、`:103-111`）。发消息**不会**中断当前正在跑的模型请求。
- **落盘**：消息作为普通 user-role 消息进入**收件方自己**的 Session 日志
  （source 是 `agent-message`），带真实 `senderSessionId`。因此它进收件方的模型上下文。
- **兄弟消息不会伪装成 Lead**：host-only 路径携带 Team 发送者 source，
  sibling 消息绝不通过公开的相邻 Agent 消息通道冒充 Lead（`PKG/dsh-experimental-agent-team/README.zh.md:132`）。
- **没有送达保证**：`Team` 不保留未接收的发送意图，不在重启后重试，不对显式重发去重
  （`PKG/dsh-experimental-agent-team/README.zh.md:68`；Agent Note `2026-09-26-team-direct-inbox`）。
- **历史 mailbox 事件已废弃**：`team/message/queued` 与 `team/message/delivered` 仍可读、可投影，
  但 Team **不再写**（`lib/index.js:533-535` 的 `isTeamEvent` 仍认这四种类型；
  `PKG/dsh-experimental-agent-team/README.zh.md:144`）。历史 pending 邮件**永不投递**。

### 2.3 `interrupt_agent`：停当前回合，保留收件箱

`lib/index.js:963-977`（roster 层）：

```
membership = this.membership(caller)
if (membership.role !== "lead") throw TEAM_LEAD_REQUIRED
target = resolveActiveMember(...)
if (target.id === membership.root.id) throw TEAM_INVALID_TARGET   // Lead 不能中断自己
live = agents.get(target.id); if (live === undefined) return { previousStatus: "inactive" }
previousStatus = availability(live)
ctx.subagents.interrupt(target.id, { kind: "ancestor", agent: caller })
```

再往下（`PKG/dsh-subagent/lib/types/manager.js:132-166`）：校验调用方是**精确的 live 祖先 Agent**，
然后 `activation.handle.agent.cancel({ kind: 'parent' }, { keepInbox: true })`
（`:159`）。`keepInbox: true` 意味着 **inbox 里的排队消息不清空**
（`dsh-agent-loop/lib/index.js:815-821`）。

效果：**只停当前这一回合**；后代继续跑；子级仍能接受后续消息；任务归属不变。
返回的是**取消之前**采样的状态，不是「已经停稳」的确认（`PKG/dsh-tool-subagent-control/README.zh.md:52`）。

### 2.4 `wait_agent`：等「下一次变化」，不唤醒任何人

`tool-agent-team/lib/index.js:329-350`：

```
timeoutMs = args.timeout_ms ?? 30000
if (越界) return ctx.agentTeams.waitForChange(...)      // 交给领域层抛 TEAM_INVALID_TIMEOUT
if (!listMembers(caller).some(m => m.id !== caller.id && (m.status === "running" || m.status === "provisioning")))
    return { timedOut: false, noProgress: { reason: "no-active-peer", message: NO_ACTIVE_PEER_MESSAGE } }
return ctx.agentTeams.waitForChange(caller, timeoutMs, exec.signal)
```

领域层（`lib/index.js:47-89`，`TeamActivity.wait`）是**一次性 waiter 集合**：
按 Team id 分组，10 秒到 1 小时，只报告**是否超时**，不返回「发生了什么」。

**什么会唤醒等待者**（`notify` 的三个来源）：

| 来源 | 位置 | 含义 |
| --- | --- | --- |
| journal commit | `lib/index.js:1461-1463` | 任何 roster / task 变更 |
| `agent/status` 事件 | `lib/index.js:1469-1472` | 任何成员状态变化（含 running→idle） |
| 一次发送被接收 | `lib/index.js:1612` | 有人发了消息 |

**它不会**：唤醒 inactive 成员；回放调用之前发生的变化；返回具体变化内容。
调用方醒来后必须**重新读状态**（`list_agents` / `team_task_list`）。

### 2.5 共享任务板：CAS + DAG + 派生视图

**存储**：任务不是一张表，而是 **Lead 会话日志里的一串完整快照**。
`lib/index.js:1234-1238`（create）与 `:1375-1379`（update）都 append
`{ type: "team/task", data: { version: 2, teamId, task } }`。

**id**：`task-<n>`，`n` 从 1 单调递增（`lib/index.js:1222`；投影端 `lib/index.js:615-620` 维护 `nextTaskNumber`）。
id 空间耗尽报 `TEAM_TASK_LIMIT`，**不复用最后一个 id**（`lib/index.js:1223`）。

**revision**：每次变更 +1，**必须连续**（投影端 `:613`：`task.revision !== prior.revision + 1` 就报错）。
新建任务 revision = 1（`:1226`）。

**CAS**：`update` 先比 `current.revision !== request.expectedRevision` → 抛
`TEAM_TASK_STALE_REVISION`（`lib/index.js:1278`）。**这防止两个成员悄悄覆盖彼此的成果。**

**授权**（`lib/index.js:1280-1284`）：`lead` 或 `owner` 才能 edit / release / set_dependencies /
complete / reopen / delete；`reassign` **仅 Lead**（`:1339`）；`claim` 任何成员都能做，但要求
「未被他人认领 + status 是 pending + 所有 blocker 已完成」（`:1287-1289`）。

**状态机**（`lib/index.js:1286-1369`）：

| action | 前置 | 结果 |
| --- | --- | --- |
| `claim` | pending、无其他 owner、ready | `in_progress` + owner=caller |
| `release` | in_progress、owner/Lead | `pending`，**清 owner** |
| `edit` | owner/Lead，且至少给一个字段 | 改 subject / description / writeScopes |
| `set_dependencies` | owner/Lead | 替换整份 blocker 列表 |
| `complete` | in_progress、owner/Lead | `completed` |
| `reopen` | completed、owner/Lead | `pending`，**清 owner** |
| `reassign` | **仅 Lead**，pending 或 in_progress，且 ready | `in_progress` + owner=被指派人；不给 owner 则清 owner 并回 pending |
| `delete` | owner/Lead，且**没有别的未删任务依赖它** | `deleted`（tombstone） |

**依赖是 DAG**（`lib/index.js:308-335`，`assertTaskGraphCandidate`）：
禁止自依赖、重复 blocker、指向缺失或已删任务；再做一次 DFS 检测环。

**ready 是派生量**，不是字段（`lib/index.js:354-356`）：
`task.status === "pending" && 所有 blocker 都是 completed`。

**ownerName 是派生量**（`lib/index.js:367`）：ownerId 等于 Team id 时是 `"lead"`，
否则查 roster 里的名字。

**writeScopes 重叠警告**（`lib/index.js:345-347`、`:368-372`）：
按路径**分段**判重叠（`a/b` 与 `a/bc` 不算重叠），**只对 `in_progress` 的任务算**，
产出 `writeScopeWarnings: ["write scopes overlap with task-3"]`。
**它只是字符串提示，不阻止任何操作**（Agent Note「绝不阻止 claim 或授予写权限」）。

**删除的任务**保留为 tombstone：不在 `listTasks()` 里（`:1263` 过滤 `status !== "deleted"`），
不占 `maxTasks`（`:1221` 只数非 deleted），但保留 id 稳定性与可回放性。

### 2.6 持久性与投影

- **写入即持久**：roster / task 事件 append 到 Lead 会话后**立刻 flush** 才报成功
  （`lib/index.js:156-160`）。所以「成功了」意味着已经落盘。
- **状态是投影出来的**：`teamProjectionDefinition`（`lib/index.js:727-737`）注册为
  `agentTeam` 投影，`stateVersion: 4`。它把一条 root Session 重放成 roster + 任务板
  （+ 历史 mailbox）。
- **过滤按 TeamId**（`lib/index.js:572`），所以普通 fork 继承来的 `team/*` 事件
  不会污染新 Root 的状态。
- **投影只发布集合，不重复保存 seq/time**（顺序与时间继续由 Session 事件的 `seq`/`time` 负责）。
- **浏览器侧**：Lead Session 通过 `agentTeam` 投影发布 roster 与未删任务；
  每次 roster 或任务变化都会把**完整**视图发给**所有**已连接浏览器，即使它正看着别的 Session
  （`PKG/dsh-experimental-agent-team/README.zh.md:201`）。
  面板**只读**，不能 spawn / rename / delete / interrupt
  （`PKG/dsh-experimental-client-ui-agent-team/README.zh.md:92`）。
- **四种 Team 事件只存在于日志**，从不进入会话表面，因此派生模型历史不受协作记录影响
  （`PKG/dsh-experimental-agent-team/README.zh.md:144`）。

### 2.7 并发模型

- **teammate 之间并行**：每个 teammate 是自己的 agent loop；Lead 调 `spawn_teammate` 之后
  立刻返回，**不等结果**（`tool-agent-team/lib/index.js:250-291`，`execute` 里只 await
  `spawnTeammate` 本身）。Lead 的回合继续。
- **同进程**：两个 in-process 提供方（`spawn` / `fork`）都在当前进程里跑子 Agent
  （`PKG/dsh-subagent-spawn-in-process/README.zh.md:12`）。
- **Lead 也是普通参与者**：它有同样的九个工具、同样的策略文本；Lead 专属的只有
  `spawn_teammate` / `interrupt_agent` / 任务的 `reassign`。
- **策略要求 Lead 等完**：「The Lead must wait for required teammates before giving the final answer.」
  （`tool-agent-team/lib/index.js:29`）。

### 2.8 `subagent` / `subagent_fork` / `workflow` 与 teammate 的差别

| | `subagent` | `subagent_fork` | `workflow` | **teammate（`spawn_teammate`）** |
| --- | --- | --- | --- | --- |
| 工具名 | `subagent`（`dsh-tool-subagent`，`provider: spawn`） | `subagent_fork`（同一个包，`provider: fork`） | `workflow`（`dsh-tool-workflow`） | `spawn_teammate` |
| 常驻 vs 临时 | 临时，但本地 spawn/fork **continuable**（可再发消息、可冷恢复） | 同左 | 临时：脚本里 `agent()` 起 fresh one-shot 子代理 | **常驻**：进持久 roster，有名字、有 owner、有任务归属 |
| 上下文 | 空对话（`fresh`） | 父级**已完成** turn 前缀（一次性快照） | 脚本里 `agent()` 是 fresh，提示词必须自足 | `fresh` 或 `fork`，由创建请求决定 |
| 是否进 roster / 任务板 | **否** | **否** | **否**（工作流记录自己的成员，不加进父级 subagent 目录，也不发完成通知） | **是** |
| 谁管它 | 父级（parent） | 父级 | workflow 引擎（运行结束即释放） | Team 领域 + continuation owner |
| 模型看到结果 | 父级收到**完成通知**（含最终答案） | 同左 | 父级只看到**最终 JSON 值**，中间子代理消息被省略 | 成员之间靠 `send_message` 互发；Lead 用 `wait_agent` 等变化 |
| 并发控制 | 无显式上限（宿主管） | 同左 | `maxConcurrentAgents`（0 = 按 CPU 并行度）、`maxTotalAgents`（默认 1000） | 无显式并发闸门；上限是 `maxMembers`（默认 16） |

依据：`PKG/dsh-base/cordis.patch.yml:380-395`（`subagent` / `subagent_fork` 是
`dsh-tool-subagent` 的两个实例，只差 `provider` 与 `toolName`）；
`PKG/dsh-tool-workflow/README.zh.md:38`（中间子代理消息省略）；
`PKG/dsh-workflow-ptc/README.zh.md:53`（工作流记录自己的成员、不加进父级目录、不发完成通知）。

**workflow 的 phase / pipeline / parallel 语义**（`PKG/dsh-tool-workflow/lib/index.js:140-148`）：

- `agent(prompt, opts?)` —— 起一个子代理跑到完成；`opts.schema` 给结构化输出；失败返回 `null`。
- `pipeline(items, ...stages)` —— 每个 item **独立**流过各阶段，**阶段之间无栅栏**；
  某阶段抛错只把**那个 item** 变成 `null` 并跳过它的后续阶段。
- `parallel(thunks)` —— 并发跑零参函数，**等全部**（这是栅栏）；抛错的 thunk 返回 `null`。
- `phase(title)` / `log(message)` —— 进度叙事；`args` —— 工具调用的原始 `args`。
- 误用（坏参数、未知选项、不支持的 schema、撞上限）**终结整个脚本**，不产生 `null`。
- 脚本**没有** filesystem / network / timer / Node API；干活的是 agent。

---

## 3. 共享什么、隔离什么

| 维度 | 共享？ | 依据 |
| --- | --- | --- |
| 进程 | 共享 | in-process 提供方 |
| 工作目录 / 检出 | **共享**，改动立刻互相可见 | `PKG/dsh-experimental-agent-team/README.zh.md:203` |
| 文件写冲突 | **没有锁**。DSH 明确拒绝把 owner / write scope 当锁：外部 writer 会绕过，崩溃的 owner 会永久占住，路径前缀重叠不能证明语义独立 | Agent Note「Shared checkout boundary」与「Alternatives considered → 把 task ownership 或 write scope 当作锁」 |
| 文件版本护栏 | 只有 read/edit/write 这类工具有 `FS_STALE_VERSION` 检查；Bash、formatter、codegen、外部写入**绕过** | `tool-agent-team/lib/index.js:27` |
| 上下文 | **隔离**。每个成员有自己的历史；peer 消息只进**收件方**的历史 | `PKG/dsh-experimental-agent-team/README.zh.md:184` |
| 模型上下文里的团队状态 | **不进**。roster / task 变化不增加 token，要查就调工具 | `PKG/dsh-experimental-agent-team/README.zh.md:188` |
| token 预算 | **不共享**。每个成员烧自己的上下文；Lead 的上下文里只有 peer 消息与工具结果 | 同上；`PKG/dsh-experimental-tool-agent-team/README.zh.md:131` |
| 回合预算 | 各成员独立（DSH 侧未建模，Agent 各自跑） | 推论，见 `spawn` 提供方「子 agent 为全新的独立上下文与历史消耗 token」（`PKG/dsh-subagent-spawn-in-process/README.zh.md:113`） |
| 事件流 | **不共享**。roster/task 在 **Lead** 的日志，peer 消息在**收件方**的日志 | `lib/index.js:156-160` 与 `:1604-1611` |
| 任务板 | **共享**（Lead 日志上的投影），CAS 保护 | `lib/index.js:1272-1382` |

DSH 明确说：**不提供 worktree、远端成员、merge 或文件锁**
（`PKG/dsh-experimental-agent-team/README.zh.md:203`），也不推断 branch、不创建 worktree、
不静默改 cwd（Agent Note「Shared checkout boundary」）。worktree 交给部署方另用显式工作树创建。

---

## 4. 对 heng 的可移植性判断

### 4.1 分层：**运行时框架能力** vs **工具层能力**

这是本次调研最关键的一条。DSH 把两层切得很干净，heng 应该照抄这条缝。

**A. 运行时必须自己造的能力（heng 目前没有）**

| 能力 | DSH 的实现 | heng 要做的事 |
| --- | --- | --- |
| **A1 可重放的 Team 状态日志** | `team/member` / `team/task` 事件追加到 Lead Session 日志 + `agentTeam` 投影重放（`lib/index.js:727-737`） | heng 的会话已经是只追加 JSONL —— 但要决定**板落谁的流**（见 4.2） |
| **A2 每 Team 的串行事务队列** | `TeamJournal.transact(rootId, op)`：把同一 Lead 的异步变更串成一条链（`lib/index.js:140-149`） | CAS 之所以能成立，全靠这条队列把「读—判—写」做成原子。缺了它，`expected_revision` 只是摆设 |
| **A3 持久 inbox（双队列）+ 唤醒语义** | `next-turn` / `next-step` 两个持久队列 + `splice` 事件日志 + `claim` + `cancel(keepInbox)`（`dsh-agent-loop/lib/index.js:21-209`、`:800-821`） | heng 的 `interjection-flow` 恰好是这条缝，最接近可抄 |
| **A4 冷恢复（continuation 生命周期）** | 从持久化 Session + descriptor 重建 activation（`PKG/dsh-subagent/lib/types/manager.js:1171-1214`）；DSH 还额外用 Team 日志对账（`lib/index.js:1112-1146`） | heng 的会话是可搬运目录，**天然可冷恢复**；这是 heng 比 DSH 好的地方 |
| **A5 变化通知总线** | `TeamActivity`：按 Team id 分组的一次性 waiter（`lib/index.js:37-106`） | `wait_agent` 需要它。注意：**只报「有变化」，不报内容** |
| **A6 按 Agent 作用域装工具 + 身份注入** | `agent.ctx` 上注册 + `systemPrompt.context({ required: true })`（`tool-agent-team/lib/index.js:227-249`） | heng 的执行者身份是写进 `EXECUTOR_IDENTITY` 的常量文本；team 身份需要「按当前 membership 现算」 |
| **A7 准入与结算生命周期** | `TeamRuntimeLifecycle`：关闭准入 → 中止并等待已准入操作 → 有界释放（`lib/index.js:166-258`、`:1626-1637`） | heng 的取消语义（`agent::cancel`）已经有一半 |

**B. 工具层暴露给模型的能力（薄壳，抄起来便宜）**

九个工具在 DSH 里**几乎没有自己的逻辑**，全是对 `ctx.agentTeams` 的转发
（`tool-agent-team/lib/index.js:276-289`、`:308-317`、`:325-326`、`:337-348`、`:360-361`、
`:390-396`、`:430-440`、`:452-453`、`:509-519`）。真正属于工具层的只有三样：

1. **参数名与领域名的映射**：`blocked_by` → `blockedBy`、`expected_revision` → `expectedRevision`、
   `write_scopes` → `writeScopes`（`:394-395`、`:514-518`）。
2. **结果 schema 声明 + 紧凑 JSON 渲染**（`:204-219` 的 `jsonOutput`）：声明完整 schema，
   渲染时不花 token 在缩进上。DSH 说这是刻意选择，因为「未声明的结果类型会让 `execute`
   在没有编译错误的情况下偏离对模型的承诺」（Agent Note「Alternatives considered →
   让 Team 工具返回未类型化 JSON」）。
3. **权限落地与角色判断**：Lead-only 在执行时检查，不靠提示词。

### 4.2 heng 与 DSH 的**结构差异**（决定了抄不动的地方）

| 维度 | DSH | heng 现状 | 后果 |
| --- | --- | --- | --- |
| **team 形状** | **树**：Lead + 它的直接子会话 | seed 要的是**平级**：同目录几个会话互发（`seed.md:10-11`） | DSH 的 peer 消息**只在 Team 内部**成立；DSH 没有「两个平级 root 互发」的先例。heng 要自己定义 team id 与成员判定 |
| **板落哪儿** | **Lead 那一条**会话日志 | heng 每条会话一个目录（`seed.md:33-34`） | seed 分叉 2 问的正是这个。DSH 的答案是「Lead 的流」，heng 若要平级团队，可能需要**独立的团队级目录**（DSH 没有这个形态） |
| **执行者是不是成员** | 执行者（Executor）在 DSH 里**不存在**这个概念；DSH 的 teammate 是**独立 child Session** | heng 的 Executor 是**同一会话内的嵌套会话**，共享同一条事件流，深度为一 | heng 的 Executor **不能直接当 team member**：它没有独立 Session 日志、没有独立持久身份。要升格成成员，得先让它有自己的会话目录 —— 这是最大的结构性改动 |
| **工作目录** | **共享**，无锁 | heng 有 `PathLocks`（`docs/executor.md:32`、`:56`） | heng 的写排斥**比 DSH 强**（DSH 明确拒绝把 owner 当锁）。这条路 heng 不用抄 DSH 的 advisory write scope，直接把 `PathLocks` 暴露成硬约束更好 |
| **token 记账** | 各成员独立 | heng **共享计入会话总量**（`docs/executor.md:53`） | heng 的 team 形态下，token 归属要么继续共享（则预算难切分），要么改成按成员记 —— 这是个明确的设计决策点 |
| **深度限制** | team 只有一层；teammate 不能 spawn | heng Executor 深度为一 | **天然吻合**：DSH 的 team 深度也是一。heng 若做 team，深度一就是默认值，不需要额外机制 |

### 4.3 三个最值得直接抄的设计

1. **「派出去」与「等结果」拆成两个工具。**
   `spawn_teammate` 立即返回；`wait_agent` 只等下一次变化。
   Lead 因此可以在成员跑着的时候继续干别的，而不是阻塞在一次 `task()` 调用上。
   DSH 还额外加了一条「**先查再等**」的快捷路径：没有别的成员 running/provisioning 时
   `wait_agent` 立刻返回 `noProgress` 并附带一句可执行的提示
   （`tool-agent-team/lib/index.js:32`、`:341-347`），避免模型空转。

2. **任务板 = 会话日志上的可重放投影 + CAS 修订号。**
   不另开数据库。任务快照整条进日志，状态由投影算出来。
   `expected_revision` 让两个成员**不会悄悄覆盖彼此的成果**（`lib/index.js:1278`），
   并且过期编辑被**拒绝**而不是静默合并（`PKG/dsh-experimental-tool-agent-team/README.zh.md:68`）。

3. **团队状态与模型上下文分离。**
   roster 与任务的任何变化**不增加任何 token**；模型要用就调 `list_agents` / `team_task_list` 查。
   只有 peer 消息进收件方的历史（`PKG/dsh-experimental-agent-team/README.zh.md:188`）。
   heng 若做 team，这条能直接省掉一大块上下文开销。

### 4.4 可以抄、但要改的

- **DSH 的 write scope 是 advisory，heng 不用抄这一点。** heng 有 `PathLocks`，
  可以把「写作用域」做成**硬约束**（claim 时登记路径前缀，冲突则拒绝 claim）。
  这是 heng 相对 DSH 的**升级点**，不是要退让的地方。
- **DSH 的 `wait_agent` 不唤醒成员，heng 可以做得更好。**
  DSH 自己在策略文本里反复强调「先 `send_message` 唤醒，再 `wait_agent`」
  （`tool-agent-team/lib/index.js:29`）—— 这是**模型的负担**。
  heng 的 `interjection-flow` 已经在处理「收件方怎么被唤醒」，可以顺手让
  「等待」变成「等待并唤醒」的组合工具，少一次模型往返。
- **DSH 的 `send_message` 没有送达保证。** heng 的会话是可搬运目录，
  可以给消息一条**独立的持久投递记录**（收件方那条 JSONL 里写一条 `inbound` 事件，
  带上 `delivered_at`），换取「重启后未投递的仍在队列里」—— 这是 DSH 明确放弃的东西
  （Agent Note `2026-09-26-team-direct-inbox` 的「后果」段）。

### 4.5 抄不动的

- **DSH 的 Team 身份 = Lead 的 SessionId。** heng 若做平级团队，
  team id 必须另立（一个团队级目录 / 一个「同桶」判据），
  这套「隐式 root 即 team」的省事做法用不上。
- **DSH 的四个 Team 事件类型与 `agentTeam` 投影版本号（stateVersion 4）**，
  绑在 DSH 的 Session 事件体系上。heng 的事件流是另一套（`ExecutorSpawned` /
  `ExecutorFinished` / `UsageRecorded`），要自己设计对应物。
- **DSH 的 activation 生命周期（residency / drain / cold-resume / descriptor 折叠）**
  是 DSH 自己的 subagent 子系统，heng 没有对应物。
  但好消息是：**heng 的会话本来就是独立目录，天然可冷恢复**，
  不需要 DSH 那套「从 descriptor 重建 activation」的间接层。

---

## 5. 坑与代价（DSH 自己承认的）

1. **共享 checkout，没有隔离。** 所有成员用 Lead 的当前目录，改动立刻互相可见；
   不提供 worktree、远端成员、merge 或文件锁（`PKG/dsh-experimental-agent-team/README.zh.md:203`）。
2. **写作用域提示是 advisory，不是锁。** Bash、formatter、codegen 与直接外部写入可以绕过
   文件版本检查；Lead 必须协调 owner 并检查最终 diff（`PKG/dsh-experimental-agent-team/README.zh.md:204`）。
3. **不会自动释放任务 owner。** 成员不活动、被 interrupt、进程退出、工作失败，
   **都不会**释放 owner（`PKG/dsh-experimental-agent-team/README.zh.md:206`）。
4. **失败永久占位。** provisioning 失败的 teammate 保留其名字与成员槽位，名字永不复用
   （`PKG/dsh-experimental-agent-team/README.zh.md:60`、`:206`）。
5. **无发送重试保证。** Team 不保留新尝试、不去重；**不支持多个进程同时操作一个 Team**；
   从未到达目标的历史排队消息永远不会被投递（`PKG/dsh-experimental-agent-team/README.zh.md:207`）。
6. **`wait_agent` 容易让模型空转。** 没有别的成员 running/provisioning 时立刻返回 `noProgress`；
   它**永不唤醒成员**；它**不回放**之前的变化（`tool-agent-team/lib/index.js:331`）。
7. **`inactive` 语义模糊。** 它同时表示「已加载但空闲」和「只有存储、需要冷恢复」；
   冷恢复有延迟，冷恢复失败会**拒绝新发送**（Agent Note「Consequences」）。
8. **token 成本固定且不低。** 每个成员每次请求都付「固定策略 + 九个 schema + 身份快照」的固定成本
   （`PKG/dsh-experimental-tool-agent-team/README.zh.md:131`）；且团队工具**覆盖**了同名的
   旧全局控件，不能与 `subagent` / `subagent_fork` 并存
   （`PKG/dsh-experimental-agent-team-profile/README.zh.md:49`）。
9. **实验性、无稳定性承诺。** schema 仍可自由变更（`PKG/dsh-experimental-tool-agent-team/README.zh.md:145`）。
10. **投影广播开销。** 每次 roster 或任务变化都把完整 roster + 任务板发给**所有**已连接浏览器
    （`PKG/dsh-experimental-agent-team/README.zh.md:201`）。

---

## 6. 来源清单

### 6.1 本地一手源（`/usr/lib/node_modules/@deepseek-ai/dsh/`）

下表 `$` = `/usr/lib/node_modules/@deepseek-ai/dsh/node_modules/@deepseek-ai/`。

| 路径 | 用到的行 | 内容 |
| --- | --- | --- |
| `$dsh-experimental-tool-agent-team/lib/index.js` | `:22-30` | 共享 `team:policy` 策略原文（共享 cwd、advisory write scope、`FS_STALE_VERSION`、steer 语义、`wait_agent` 语义、Lead 必须等待） |
| 同上 | `:31-32` | `ACTIVE_WAIT_STATUSES` 与 `NO_ACTIVE_PEER_MESSAGE` 原文 |
| 同上 | `:38-74` | `MEMBER_VIEW_SCHEMA`（成员行字段） |
| 同上 | `:84-135` | `TASK_VIEW_SCHEMA`（任务行字段） |
| 同上 | `:136-203` | 五个工具的结果 schema（spawn / list / send / wait / interrupt / task_list） |
| 同上 | `:204-219` | `jsonOutput`：声明式结果 schema + 紧凑 JSON 渲染 |
| 同上 | `:227-249` | 按 Agent scope 安装；`team:policy` 段与 required `team:identity` 上下文 |
| 同上 | `:250-291` | `spawn_teammate`（name/description/prompt/context，provider 选择） |
| 同上 | `:292-319` | `send_message`（target/message，返回 `{ sent: true }`） |
| 同上 | `:320-328` | `list_agents` |
| 同上 | `:329-350` | `wait_agent`（`noProgress` 快捷路径、超时边界） |
| 同上 | `:351-363` | `interrupt_agent` |
| 同上 | `:364-398` | `team_task_create` |
| 同上 | `:399-442` | `team_task_list`（过滤 + 分页） |
| 同上 | `:443-455` | `team_task_get` |
| 同上 | `:456-521` | `team_task_update`（八个 action + `expected_revision`） |
| 同上 | `:531-554` | `apply`：对每个 live / 新建的 Team 成员 Agent 安装工具 |
| `$dsh-experimental-agent-team/lib/index.js` | `:13-32` | `TeamError` 与 `errorMessage` |
| 同上 | `:37-106` | `TeamActivity`：一次性 waiter、`wait` / `notify` / `close` |
| 同上 | `:111-161` | `TeamJournal`：`state` / `transact` / `appendAndFlush` |
| 同上 | `:166-258` | `TeamRuntimeLifecycle`：准入截止、有界结算、`disposalTimeoutMs` |
| 同上 | `:267-286` | `TeamId` / `TeamTaskId` / `TeamMessageId` 品牌类型 |
| 同上 | `:290-335` | `assertTaskGraphCandidate`：DAG 校验（自依赖 / 重复 / 缺失 / 环） |
| 同上 | `:345-385` | `scopesOverlap` / `taskReady` / `projectTaskView`（派生视图） |
| 同上 | `:444-471` | 持久快照 schema（member / task） |
| 同上 | `:509-518`、`:684-703` | 空 Team 状态；Lead 伪行 |
| 同上 | `:533-535`、`:567-648` | 四种 Team 事件；投影重放与一致性校验（revision 连续、phase 迁移合法、id 不复用） |
| 同上 | `:727-737` | `teamProjectionDefinition`（key `agentTeam`，`stateVersion: 4`） |
| 同上 | `:749-761`、`:766-785` | 冷恢复用的持久会话读取与「初始消息是否已被接受」判定 |
| 同上 | `:796-812` | 输入规范化（`requiredText`、`writeScope`） |
| 同上 | `:816`、`:824-836`、`:1163-1166` | 成员名正则、`resolveActiveMember`、名字校验 |
| 同上 | `:860-903` | `membership` / `tryMembership`：Lead / teammate / 非成员 的判定 |
| 同上 | `:909-936`、`:1188-1190` | roster 列表合成；`availability`（running / inactive） |
| 同上 | `:943-946`、`:1004-1074` | `spawn` / `spawnAdmitted`：两段式创建 |
| 同上 | `:1076-1110` | `checkpointInitialPrompt`：等初始消息真正落盘 |
| 同上 | `:1112-1146` | `reconcileProvisioning`：冷恢复对账 |
| 同上 | `:963-977` | `interrupt`：Lead-only、保留 inbox |
| 同上 | `:1217-1241`、`:1248-1264` | 任务 create / get / list |
| 同上 | `:1272-1382` | 任务 CAS 状态机（八个 action + 授权规则） |
| 同上 | `:1384-1416` | 依赖校验、write scope 规范化、图校验错误码映射 |
| 同上 | `:1420-1423`、`:1438-1443` | 四个部署上限与默认值 |
| 同上 | `:1461-1472` | 变化通知的两个来源（journal commit、`agent/status`） |
| 同上 | `:1490-1585` | `ctx.agentTeams` 的公开 API 面 |
| 同上 | `:1587-1614` | `sendAdmitted`：消息构造、大小校验、投递分支 |
| 同上 | `:1616-1637` | 恢复调度与 dispose 流程 |
| `$dsh-experimental-agent-team/README.zh.md` | `:60-88`、`:100-152`、`:194-207` | teammate 语义、消息语义、任务板语义、设计理念、已知限制 |
| `$dsh-experimental-tool-agent-team/README.zh.md` | `:28`、`:59-68`、`:98-102`、`:127-135`、`:143-145` | 九个工具分组、行为契约、scoping、model experience、限制 |
| `$dsh-experimental-agent-team-profile/README.zh.md` | `:12`、`:37-51`、`:90`、`:104-108` | profile bundle 的 composition（禁用 `subagent` / `subagent_fork` / 全局 child control）与限制 |
| `$dsh-experimental-agent-team-profile/cordis.patch.yml` | 全文 | 组合清单：`maxMembers: 8`、四个 Team 包 id |
| `$dsh-experimental-client-ui-agent-team/README.zh.md` | `:30-41`、`:88-92` | Web 面板的 roster / 任务板呈现与只读限制 |
| `$dsh-agent-loop/lib/index.js` | `:21-62`、`:70-209`、`:800-821`、`:902-927`、`:938+` | 持久 inbox 投影与实现；`followup` / `steer` / `inject` / `cancel(keepInbox)`；step 边界 claim |
| `$dsh-subagent/lib/types/internal.js` | `:31`、`:42-57` | `deliverSubagentPrompt` 符号与 host-only 投递壳 |
| `$dsh-subagent/lib/index.js` | `:3072-3089`、`:3105-3107` | `sendMessage` 与 `[deliverSubagentPrompt]` 的分派 |
| `$dsh-subagent/lib/types/manager.js` | `:132-166`、`:473-531`、`:1099-1142`、`:1171-1214` | `interrupt`（keepInbox）；`sendMessage` / `queuePrompt` / `steerPrompt` / `deliver`；`deliverToChild` 与冷恢复 |
| `$dsh-subagent-spawn-in-process/README.zh.md` | `:12`、`:54-56`、`:109-117` | spawn 提供方语义（进程内、空对话、继承 cwd/模型） |
| `$dsh-subagent-fork-in-process/README.zh.md` | `:12`、`:36`、`:112-120` | fork 提供方语义（已完成 turn 前缀、一次性快照） |
| `$dsh-tool-subagent/README.zh.md` | `:12`、`:59-63`、`:99-104`、`:169-175` | `subagent` 工具语义、完成通知、深度上限、委派指导原文 |
| `$dsh-tool-subagent-control/README.zh.md` | `:12`、`:46-52`、`:168-170` | 全局 `send_message` / `interrupt_agent` / `list_agents` 语义与竞态警告 |
| `$dsh-tool-workflow/lib/index.js` | `:140-148` | workflow 工具描述里的 `agent` / `pipeline` / `parallel` / `phase` / `log` 契约 |
| `$dsh-tool-workflow/README.zh.md` | `:12`、`:38`、`:42` | workflow 工具语义、中间消息省略、后台运行 |
| `$dsh-workflow-ptc/README.zh.md` | `:12`、`:41-49`、`:53`、`:124-132` | 工作流引擎的并发上限、成员归属、token 影响 |
| `$dsh-base/cordis.patch.yml` | `:380-395` | `subagent` / `subagent_fork` 是同一包的两种 provider 实例 |

### 6.2 公共仓库（web，只当资料）

| URL | 用到的内容 |
| --- | --- |
| <https://raw.githubusercontent.com/deepseek-ai/deepseek-harness/master/docs/subsystems/agent-team.zh.md> | 子系统参考：身份与 roster、消息与历史 mailbox、共享任务 DAG、Web 投影、回放语义、`ctx.agentTeams` 完整签名 |
| <https://raw.githubusercontent.com/deepseek-ai/deepseek-harness/master/.agents/notes/implemented/feature/2026-08-05-agent-teams.zh.md> | Agent Note：Problem / Decision / Alternatives / Consequences —— provisioning 与恢复、共享 checkout 边界、为什么不把 owner 与 write scope 当锁、为什么不另开内存板 |
| <https://raw.githubusercontent.com/deepseek-ai/deepseek-harness/master/.agents/notes/implemented/simplification/2026-09-26-team-direct-inbox.zh.md> | 决策：为什么 Team 消息走目标 inbox 而不再建一套 outbox |
| <https://raw.githubusercontent.com/deepseek-ai/deepseek-harness/master/.agents/notes/implemented/architecture/2026-10-13-team-identity-runtime-context.zh.md> | 决策：为什么身份放 required runtime context 而不是初始任务或 system prompt |

> 以上四份是**公共仓库当前 `master`** 的内容，与本地 npm 包 `0.2.1-rc.2` 可能有出入；
> 凡是两者冲突的，本文以**本地包**为准并标了本地行号。
