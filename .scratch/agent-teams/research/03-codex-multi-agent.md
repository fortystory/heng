# OpenAI Codex 的多 agent 能力 —— 形态与原理调研

> 调研日期：**2026-10-20**。
> 上游源码快照：`github.com/openai/codex` @ **`e1b9c9457ceca75a99b034a5df39ac48c3998d15`**（2026-10-20，"Preserve parent history by default in full agent forks (#55548)"），浅克隆到 `.cache/codex-upstream`。
> 官方文档：`learn.chatgpt.com/docs/...`（`developers.openai.com/codex/...` 现在 308 跳到该站）。
> 本地既有笔记：[`docs/research/notes/codex-gemini.md`](../../../docs/research/notes/codex-gemini.md) —— 本轮**未修改**它。凡是笔记里已有的事实，本文标 `[笔记已有]`；相对笔记新增或被修正的，标 `[本轮新增]` / `[本轮修正]`。

标注约定：`[源码]` = 上游仓库某个文件；`[文档]` = 官方文档站某页；`[笔记]` = 本地笔记。

---

## 1. 结论摘要

1. **Codex 没有「agent teams」这个概念。** 整个 `codex-rs` 树里 `agent team` / `agent_team` / `AgentTeam` **零命中**，官方文档也从不使用这个词。Codex 讲的是 **subagent（子 agent）**：一个 agent 通过**工具调用**去派生别的 agent，父子之间是**树**，兄弟之间有一条**消息通道**。`[源码]`（全树 grep）、`[文档]`
2. **形态 = 「工具驱动的委派树」**，不是「运行时调度的工作流引擎」。模型手里有一组 collaboration 工具（`spawn_agent` / `send_message` / `followup_task` / `wait_agent` / `interrupt_agent` / `list_agents`），它自己决定派谁、派几个、什么时候等。运行时只提供能力：独立线程、邮箱、限额、路径树。`[源码]`
3. **两代工具面并存**：**V1**（默认，命名空间 `multi_agent_v1`）与 **V2**（`features.multi_agent_v2`，命名空间默认 `collaboration`，扁平工具名）。V2 的 feature 标记是 `Stage::Stable` 但 `default_enabled: false`，且**官方文档完全没有提到 V2**——它只写「当前 Codex 版本默认启用 subagent 工作流」。也就是说：**对外稳定的是 V1；V2 是代码里已就绪但默认关闭、文档外的一代**。`[源码] features/src/lib.rs`、`[文档] agent-configuration/subagents.md`
4. **每个子 agent 是独立 thread**：独立 `ThreadId`、独立 rollout、独立模型调用、独立上下文窗口；**但共享同一个进程、同一份工作区、同一个 cwd**。Codex 在 V2 给 agent 的系统提示里明写："All agents share the same directory… edits made by one agent are immediately visible to all other agents."`[源码] prompts/src/multi_agent_instructions.rs`
5. **兄弟之间能直接通信**，这是它比「只有父子委派」强的地方：一条 **mailbox**（每 agent 一个队列）承载 `MESSAGE` / `NEW_TASK` / `FINAL_ANSWER` 三种消息体，投递分 **QueueOnly**（不唤醒）与 **TriggerTurn**（唤醒并开新一轮）。`[源码] core/src/agent/control/mailbox.rs`、`delivery.rs`
6. **真正共享的「任务板」只有一个，还是实验特性**：`agent_message_board`（`Stage::UnderDevelopment`，默认关）提供一个论坛式的板子 —— channel / thread / post，带 ACL 与订阅，9 个工具。这是本轮找到的**最接近「agent teams」的东西**。`[源码] ext/agent-message-board/`
7. **worktree 与多 agent 无关**。`--worktree` / `/worktree` 是**chat 级**隔离（一个 chat 一个 git worktree）；子 agent 不自带 worktree，继承父的 cwd。唯一沾边的是实验特性 `subagent_thread_environments`。`[源码]`、`[文档] environments/git-worktrees.md`
8. **对 heng 最值得抄的三件**（详见 §5）：①**规范路径树 + 同伴清单**（每个 agent 知道自己是谁、有哪些同伴）；②**两种投递语义分离**（留言 vs 派活）；③**「wait 只等信箱有动静、不回内容」**（把拉取与轮询的负担从模型身上卸掉）。

---

## 2. 形态

### 2.1 全貌与开关

Codex 的多 agent 能力由**一组 feature flag** 控制，稳定性分级写在 `codex-rs/features/src/lib.rs` 的 `FeatureSpec` 表里。`[源码]`

| feature key | 内部 id | Stage | 默认 | 作用 | 来源 |
|---|---|---|---|---|---|
| `multi_agent` | `Collab` | **Stable** | **开** | V1 多 agent 工具面 | `[源码]` features/src/lib.rs L1493-1498 · `[笔记已有]` |
| `multi_agent_v2` | `MultiAgentV2` | Stable | **关** | V2 工具面 + V2 运行时 | `[源码]` L1499-1504 |
| `multi_agent_v2_dynamic_tools` | `MultiAgentV2DynamicTools` | UnderDevelopment | 关 | V2 动态工具 | `[源码]` L1511-1516 |
| `agent_message_board` | `AgentMessageBoard` | UnderDevelopment | 关 | 共享消息板 | `[源码]` L1541-1546 |
| `subagent_thread_environments` | `SubagentThreadEnvironments` | UnderDevelopment | 关 | 子 agent 可有自己的 thread environment | `[源码]` L1517-1522 |
| `subagent_default_context_limits` | `SubagentDefaultContextLimits` | UnderDevelopment | 关 | 子 agent 不继承父的 context window / auto-compact 限额 | `[源码]` L1523-1528 |
| `defer_mailbox_preemption` | `DeferMailboxPreemption` | UnderDevelopment | 关 | 邮箱消息不抢占当前轮 | `[源码]` L1529-1534 |
| `wake_on_agent_messages` | `WakeOnAgentMessages` | UnderDevelopment | 关 | 收到 agent 消息时唤醒会话 | `[源码]` L1535-1540 |
| `multi_agent_mode` | `MultiAgentMode` | **Removed** | 关 | 旧的 multi-agent mode | `[源码]` L1547-1552 |
| `enable_fanout` | `SpawnCsv` | **Removed** | 关 | 旧的 CSV fan-out | `[源码]` L1553-1558 |

**V1 / V2 怎么选**（`[源码] core/src/config/mod.rs`）：

```
multi_agent_version_from_features():
    override ?? if features.enabled(Collab) { V1 } else { Disabled }

multi_agent_version_for_model(model_multi_agent_version):
    override ?? model_multi_agent_version ?? features 推出的值
```

即：**模型目录里自带 `multi_agent_version` 字段**，模型声明自己是 V2 就走 V2；否则走 V1（默认开）。V1 还是 V2 由 `TurnContext.multi_agent_version` 决定，而 `collab_tools_enabled()` 还要再看模型：`model_info.multi_agent_version == Some(V2)` 才允许**子 agent 自己也用** collaboration 工具（V2 明确让子 agent 能再派子 agent）。`[源码] core/src/tools/spec_plan.rs` `collab_tools_enabled`

**CLI flag**：`--enable FEATURE` / `--disable FEATURE` 是全局 flag；`codex features enable|disable` 会把选择持久化到 `$CODEX_HOME/config.toml`；TUI 里有 `/experimental` 切换实验特性。`[文档] developer-commands.md`

**文档侧的官方说法**（`[文档] agent-configuration/subagents.md`，抓取 2026-10-20）：

> Current Codex releases enable subagent workflows by default. Subagent activity appears in the ChatGPT desktop app, Codex CLI, and the IDE extension.

以及 CLI 侧的触发方式：

> Use `/subagents` to inspect and switch between agent threads while they run.

注意：官方文档**通篇不提 `multi_agent_v2`**，也不提 `agents.max_concurrent_threads_per_session` 之外的 `[features.multi_agent_v2.*]`。**文档落后于源码**这一点，与本地笔记 §2.19 记录的版本偏差一致。

### 2.2 agent roles

#### 内置角色

`[源码] core/src/agent/role.rs`（`built_in::configs()`）—— 只有三个：

| 名字 | 说明 | 备注 |
|---|---|---|
| `default` | "Default agent." | 兜底角色；`DEFAULT_ROLE_NAME = "default"` |
| `explorer` | 只读代码库探索 | 描述里写死了使用规则（避免重复探索、鼓励并行开多个 explorer、相关问题复用同一个） |
| `worker` | 执行与产出 | 描述里写死了**归属划分**规则：明确指定文件/模块归属、明确告知 worker「你不是独自在代码库里，不要回退别人的改动」 |

`explorer` 有内置的 `config_file`（`core/assets/agent/builtins/explorer.toml`），该文件在本 commit 里是 **0 字节**（空）；`awaiter` 角色（跑长命令时专职等待）**被临时移除**（源码里整段被注释，标注 "Awaiter is temp removed"），其 `awaiter.toml` 仍在树里。`[源码] core/src/agent/role.rs L388-408`、`core/assets/agent/builtins/`

> `[笔记已有]` 记的是「Built-in agents include `default`, `worker`, and `explorer`」——与本轮一致，`awaiter` 当时也没提。

#### 自定义角色：独立 TOML 文件

**路径**：`~/.codex/agents/*.toml`（个人）与 `.codex/agents/*.toml`（项目）。**递归扫描子目录**，只收 `.toml` 后缀，排序后加载。`[源码] agent-roles/src/discovery.rs` `collect_agent_role_files`

**必填字段**：`name`、`description`、`developer_instructions`。`name` 是**唯一真源**，文件名只是惯例；`developer_instructions` 缺失或空白会硬报错（"agent role file at {path} must define `developer_instructions`"）。`[源码] agent-roles/src/agent_role_config.rs`

**文件结构**（`RawAgentRoleFileToml`）：顶层除 `name` / `description` / `nickname_candidates` 之外，`#[serde(flatten)]` 一个完整的 `ConfigToml`——也就是说**角色文件就是一份 config.toml 的覆盖层**，可以写 `model`、`model_reasoning_effort`、`model_reasoning_summary`、`model_verbosity`、`features.shell_tool`、`features.apps`、`features.plugins`、`features.memories`、`features.request_permissions_tool`、`skills.config`、`skills.bundled.enabled`、`skills.include_instructions` 等。`[源码] agent-roles/src/agent_role_config.rs`、`[文档] agent-configuration/subagents.md`

**不能覆盖的**（文档明写，源码一致）：`sandbox_mode`、`approval_policy`、`model_provider`、`mcp_servers`、service tier —— 角色文件里放这些**不生效**，子 agent 一律继承父会话的。`[文档] agent-configuration/subagents.md`、`[源码] core/src/agent/child_config.rs`

**另一条声明路径**：`config.toml` 里可以内联声明角色，指向一个角色文件：

```toml
[agents.researcher]
description = "Research-focused role."
config_file = "./agents/researcher.toml"
nickname_candidates = ["Herodotus", "Ibn Battuta"]
```

`[源码] config/src/config_toml.rs` `AgentRoleToml`（字段仅 `description` / `config_file` / `nickname_candidates`）

**加载与合并**：按 config layer 从低到高逐层加载；同层内**角色名重复直接报 warning 并丢弃**（"duplicate agent role name `{name}` declared in the same config layer"）；高层只**补齐低层缺的字段**（`merge_missing_role_fields`），不覆盖。`[源码] agent-roles/src/loader.rs`

**角色怎么被选用**：
- 模型在 `spawn_agent` 的 `agent_type` 参数里指定角色名；不指定则用 `default`。`[源码] core/src/agent/role.rs` `DEFAULT_ROLE_NAME`
- 角色的 `description` 会被拼进 `spawn_agent` 工具描述里当"什么时候该用这个角色"的指引 —— **角色是给模型看的说明书，不是给运行时的派发规则**。`[源码] core/src/tools/spec_plan.rs` `agent_type_description()`
- `nickname_candidates` 给这个角色下的 agent 提供候选昵称；昵称从池子里随机取，重名时变成 "name the 2nd"。`[源码] core/src/agent/registry.rs` `reserve_agent_nickname`

### 2.3 协作工具集：确切名字与语义

这是本轮最核心的部分。**两代并存，名字和语义都不同**。

#### V1（默认；命名空间 `multi_agent_v1`）

`[源码] core/src/tools/handlers/multi_agents_spec.rs`、`core/src/tools/handlers/multi_agents/`

模型看到的工具名带命名空间前缀（Responses API namespace），即 `multi_agent_v1.spawn_agent` 这种形态。

| 工具 | 参数 | 返回 | 语义 |
|---|---|---|---|
| `multi_agent_v1.spawn_agent` | `message`（文本）或 `items`（结构化数组，二选一）、`agent_type`、`fork_context`(bool)、`model`、`reasoning_effort` | `{agent_id, nickname}` | 派一个子 agent。`fork_context=true` 把当前 thread 历史 fork 过去；否则只给初始 prompt。`items` 支持 `text`/`image`/`local_image`/`audio`/`local_audio`/`skill`/`mention`（mention 形如 `app://<connector-id>`、`plugin://<plugin>@<marketplace>`）。**V1 下 full-history fork 不允许同时指定 `agent_type`**（"Full-history forked agents inherit the parent agent type"）。 |
| `multi_agent_v1.send_input` | `target`（agent id）、`message` 或 `items`、`interrupt`(bool) | `{submission_id}` | 给已存在的 agent 发消息。`interrupt=true` 立刻打断当前任务处理这条消息；否则排队。工具描述明确说：**如果新任务高度依赖上一个任务的上下文，就用 `send_input` 复用同一个 agent，别新开**。 |
| `multi_agent_v1.wait_agent` | `targets`（agent id 数组，**任一先完成即返回**）、`timeout_ms` | `{status: {agent_id → status}, timed_out}` | 等若干 agent 到达终态。**并行 fan-out**：源码用 `FuturesUnordered` + `timeout_at`，`futures.next()` 谁先回来先处理。返回的 status 里 `completed` 带**子 agent 的最终消息全文**。超时时 `timed_out: true`。 |
| `multi_agent_v1.resume_agent` | `id` | `{status}` | **恢复一个已关闭的 agent**，使它能重新接受 `send_input` 与 `wait_agent`。 |
| `multi_agent_v1.close_agent` | `target` | `{previous_status}` | 关闭一个 agent **及其所有后代**。工具描述明写：**已完成的 agent 仍然开着、仍然占并发额度，直到被 close**；"Don't keep agents open for too long"。 |

`[笔记已有]` 记的是工具名与「`wait_agent` 真正 fan-out（`FuturesUnordered` + `timeout_at`）」——本轮在 commit `e1b9c945` 上逐条复核，**仍然成立**。`[本轮新增]` 是每个工具的**参数集、返回结构、错误语义**。

#### V2（`features.multi_agent_v2`；命名空间默认 `collaboration`，可配）

`[源码] core/src/tools/handlers/multi_agents_spec.rs`、`core/src/tools/handlers/multi_agents_v2/`

模型调用形态是 `functions.collaboration.spawn_agent`（源码 prompt 里就是这么写的：`Call … only as direct tool calls using the recipient shown in their tool definitions, such as to=functions.collaboration.spawn_agent`）。`[源码] prompts/src/multi_agent_instructions.rs`

| 工具 | 参数 | 返回 | 语义 |
|---|---|---|---|
| `spawn_agent` | `task_name`（**必填**，小写字母/数字/下划线）、`message`（**必填**）、`agent_type`、`fork_turns`（`all` \| `none`，**默认 `all`**）、`model`、`reasoning_effort` | `{task_name}` 或 `{task_name, nickname}`（`hide_spawn_agent_metadata` 默认 **true**，所以默认只回 `task_name`） | 派一个子 agent。**V2 的寻址是规范路径树**：若当前是 `/root/task1` 且 `task_name="task_3"`，子 agent 的规范名是 `/root/task1/task_3`；之后可写 `task_3` 或 `/root/task1/task_3` 互换指代，而 `/root/task2/task_3` 只能被它的规范名指到。**子 agent 拥有与父相同的工具，且能再派自己的子 agent**。`fork_turns="none"` 不传任何上下文（会缺上下文），`"all"` 传全部周边上下文。 |
| `send_message` | `target`（相对或规范任务名）、`message` | 无 output schema | 投一条消息。**"Does not trigger a new turn."** —— 纯留言，目标 agent 空闲时也不会被叫醒。 |
| `followup_task` | `target`（agent id 或规范任务名）、`message` | 无 output schema | 给**非 root** 的目标 agent 派一件新任务并**触发一轮**。若目标正在跑，则"在采样时的消息边界"或"当前 pending 工具调用结束后"送达。 |
| `wait_agent` | `timeout_ms`（可选） | `{message, timed_out}` | **等 mailbox 里有动静**，不等某个具体 agent。会在三种情况下结束：有 mailbox 更新 / 用户输入被 steer 进当前轮 / 超时。**"Does not return the content; returns either a summary of which agents have updates (if any), an interruption summary for steered input, or a timeout summary"**。默认 30s，最小 10s，最大 1h（`features.multi_agent_v2.{min,default,max}_wait_timeout_ms`）。 |
| `interrupt_agent` | `target`（agent id 或规范任务名） | `{previous_status}` | 打断该 agent 当前的一轮，返回打断前的状态。**agent 本身仍然可用**，还能收消息、接 follow-up。 |
| `list_agents` | `path_prefix`（可选，任务路径前缀过滤，无尾斜杠） | `{agents: [{agent_name, agent_status}]}` | 列出**当前 root thread 树里的活着的 agent**。 |

**V2 的关键语义差异**（相对 V1）：

- **V2 没有 `close_agent`**，改为 `interrupt_agent` + 「agent 一直活着直到树被拆」。
- **V2 把「留言」与「派活」拆成两个工具**：`send_message`（QueueOnly，不触发轮次）vs `followup_task`（TriggerTurn，触发轮次）。这在源码里是 `MessageDeliveryMode::{QueueOnly, TriggerTurn}` 两个枚举值。`[源码] core/src/agent/types.rs`、`core/src/agent/control/delivery.rs`
- **V2 的 `wait_agent` 只等"有动静"，不回内容**；内容通过 mailbox 进入上下文。
- **V2 的子 agent 能递归派子 agent**（工具描述明写，且 `collab_tools_enabled()` 要求模型自身支持 V2）。
- V2 里 `disable_direct_message = true` 时，`send_message` / `followup_task` 被摘掉，只留 `interrupt_agent` / `list_agents`（spawn 与自动回传结果仍在）。`[源码] core/src/tools/spec_plan.rs` `required_child_management_tool_names`

#### V2 的额外行为开关（都在 `[features.multi_agent_v2]` 下）

`[源码] features/src/feature_configs.rs` `MultiAgentV2ConfigToml`，默认值见 `core/src/config/mod.rs` `defaults_for_max_concurrency`：

| 键 | 默认 | 作用 |
|---|---|---|
| `enabled` | — | 开 V2（等价于 feature key） |
| `max_concurrent_threads_per_session` | 4 | 同时打开的 spawned thread 上限（**不含主线程**，故实际子额度 = N-1） |
| `min_wait_timeout_ms` / `default_wait_timeout_ms` / `max_wait_timeout_ms` | 10000 / 30000 / 3600000 | `wait_agent` 超时边界 |
| `tool_namespace` | `"collaboration"` | 工具命名空间，正则 `^[a-zA-Z0-9_-]+$` |
| `hide_spawn_agent_metadata` | **true** | `spawn_agent` 只回 `task_name`，不回 nickname |
| `expose_spawn_agent_model_overrides` | **true** | 是否把 `model` / `reasoning_effort` 暴露给模型 |
| `wait_agent_enabled` | **true** | 是否暴露 `wait_agent` |
| `disable_native_tools` | false | 隐藏原生多 agent 工具（保留 V2 运行时给宿主用） |
| `disable_direct_message` | false | 摘掉 `send_message` / `followup_task` |
| `preserve_fork_prefix` | **true** | 完整 fork 时保留父的 model-visible 历史前缀 |
| `subagent_developer_instructions` | — | 覆盖没有角色专属指令的子 agent 的开发者指令 |
| `root_agent_usage_hint_text` / `subagent_usage_hint_text` | — | 覆盖系统提示里的多 agent 行为指引 |
| `multi_agent_mode_hint_text` | — | 覆盖多 agent 模式提示（见 §3.6） |
| `message_board_in_memory` | false | 消息板只放内存（训练用，含 ephemeral 会话） |
| `message_board_remote` | — | `{url, bearer_token, bearer_token_env_var}`，用 session-scoped 远程板 |
| `non_code_mode_only` | **true** | 只在非 code-mode 下暴露 |

`usage_hint_enabled` / `usage_hint_text` 标了 "Deprecated compatibility field. Its value is ignored."。`[源码]`

### 2.4 `[agents]` 配置（V1 与 V2 共用）

`[源码] config/src/config_toml.rs` `AgentsToml`；文档侧 `[文档] agent-configuration/subagents.md`

| 键 | 类型 | 说明 |
|---|---|---|
| `enabled` | bool | 多 agent 工具总开关，**默认 true**；`features.multi_agent_v2` 开启时优先 |
| `max_concurrent_threads_per_session` | int (≥1) | 同时打开的 spawned agent thread 上限（**不含主线程**）；旧别名 `max_threads` |
| `max_depth` | int | **V1 专用**的嵌套深度上限，V2 忽略。默认 `DEFAULT_AGENT_MAX_DEPTH = 1` |
| `default_subagent_model` | string | spawn 未指定时的默认模型 |
| `default_subagent_reasoning_effort` | enum | spawn 未指定时的默认推理强度 |
| `interrupt_message` | bool | agent 被打断时是否记一条**模型可见**的消息，默认 **true** |
| `job_max_runtime_seconds` | u64 | 已移除，保留兼容的空操作 |
| `<任意角色名>` | table | 内联角色声明（见 §2.2） |

**优先级链**（`[文档] agent-configuration/subagents.md` + `[源码] core/src/agent/child_config.rs`）：
显式 spawn 参数 > `[agents]` 默认值 > 父会话的值。若「显式 spawn 或 `[agents]` 默认选了一个模型，但都没给推理强度」，则用**那个模型的默认推理强度**；角色文件里只写 `model` 时会保留此前已解析出的推理强度。`[源码] apply_requested_spawn_agent_model_overrides`

---

## 3. 原理

### 3.1 编排模型：工具驱动，不是运行时调度

**结论：Codex 的多 agent 编排是「agent 用工具调用别的 agent」。** 运行时**不**有一个调度器替模型决定派谁、并行几个、什么时候收敛。模型手里有一组 collaboration 工具，工具描述里甚至直接教它「什么时候该派、什么时候该自己干、什么时候该等」。

`spawn_agent` 的工具描述里有一整段「When to delegate vs. do the subtask yourself / Designing delegated subtasks / After you delegate / Parallel delegation patterns」的规则文本，包括：

> Do not spawn sub-agents unless the user or applicable AGENTS.md/skill instructions explicitly ask for sub-agents, delegation, or parallel agent work. Requests for depth, thoroughness, research, investigation, or detailed codebase analysis do not count as permission to spawn.

> Do not delegate urgent blocking work when your immediate next step depends on that result.

> Call wait_agent very sparingly. Only call wait_agent when you need the result immediately for the next critical-path step and you are blocked until it returns.

> For code-edit subtasks, decompose work so that each delegated task has a disjoint write set.

`[源码] core/src/tools/handlers/multi_agents_spec.rs` `spawn_agent_tool_description`

运行时提供的只是**能力面**：
- `AgentRegistry`（会话级共享）：agent 路径树、昵称池、spawn 额度
- `LocalAgentControl` / `LocalAgentRuntime`：进程内的 agent 树运行时
- `Mailboxes`：每 agent 一个待收队列
- `AgentExecutionLimiter`：并发执行许可

`[源码] core/src/agent/registry.rs`、`core/src/agent/control.rs`

**主 agent（lead）没有特殊类型**。root 就是 `AgentPath::ROOT`；`AgentPath` 是一棵树，节点名是 `task_name`；`SessionSource::SubAgent(SubAgentSource::ThreadSpawn { parent_thread_id, depth, agent_path, agent_nickname, agent_role })` 记录每个 agent 的出身。root 与子 agent 走**同一套** collaboration 工具（V2 下子 agent 甚至可以派自己的子 agent）。`[源码] protocol/src/protocol.rs`、`core/src/agent/control/runtime_context.rs`

### 3.2 线程与上下文

**每个子 agent 是一个独立 thread**：`ThreadId` 独立、rollout 独立（落在 `$CODEX_HOME/sessions/...`）、模型调用独立、上下文窗口独立。spawn 时创建一个新的 session 并把初始消息投进去。`[源码] core/src/thread_spawn.rs`、`core/src/agent/control/spawn.rs`

**上下文继承有三种形态**：

| 形态 | V1 | V2 |
|---|---|---|
| 只带初始消息 | `fork_context=false`（默认） | `fork_turns="none"` |
| 复制父历史前缀 | `fork_context=true` | `fork_turns="all"`（**默认**） |

V2 的 `fork_turns` 只接受 `"none"` / `"all"`（也接受历史遗留的回合数，但只当作 `all` 处理）；传别的字符串报错 "fork_turns must be `none` or `all`"。`[源码] core/src/tools/handlers/multi_agents_v2/spawn.rs` `SpawnAgentArgs::fork_mode`

**`preserve_fork_prefix`（V2，默认 true）**：完整 fork 时保留父的 **model-visible 历史**并追加子 agent 指令；关掉则旧式压缩重建。`[源码] features/src/feature_configs.rs`

**角色指令怎么叠加**（`[源码] core/src/agent/child_config.rs` `prepare_agent_spawn_config`）：
- V1 完整 fork **禁止**指定 `agent_type`（角色会跟父历史里的角色冲突）
- V2 完整 fork **可以**指定 `agent_type`；此时角色的 `developer_instructions` 覆盖继承来的父指令；若角色没给指令则沿用父的
- 非完整 fork 一律应用角色

**子 agent 继承什么**（`[源码] apply_spawn_agent_runtime_overrides` + `build_agent_shared_config`）：
- ✅ 继承：`approval_policy`、`approvals_reviewer`、**cwd**、`permission_profile`、`model_provider`、MCP servers、skills（可按角色关掉个别）、service tier（跟随 root）
- ❌ 不由角色决定：sandbox mode、approval policy、provider、MCP servers
- 模型覆盖优先级：spawn 参数 > `[agents]` 默认 > 父会话
- 实验特性 `subagent_default_context_limits` 打开后，子 agent **不继承**父的 `model_context_window` 与 `model_auto_compact_token_limit`

### 3.3 消息与邮箱

**消息载体**：`InterAgentCommunication`（`author: AgentPath`, `recipient: AgentPath`, `content`, `trigger_turn: bool`）。`[源码] protocol/src/protocol.rs`

**消息体渲染**（`[源码] core/src/context/inter_agent_message.rs`、`inter_agent_completion_message.rs`）：

```
Message Type: MESSAGE | NEW_TASK | FINAL_ANSWER
Task name: <recipient path>
Sender: <author path>
Payload:
<原文全文>
```

**传全文还是摘要**：**全文**。`MESSAGE` / `NEW_TASK` 投的是调用方给的原文；`FINAL_ANSWER` 投的是子 agent 的**最终消息全文**（`AgentStatus::Completed(Some(message))`）。只有错误路径会截断（`truncate_text(error, TruncationPolicy::Tokens(ERROR_MAX_TOKENS))`）。没有摘要、没有裁剪、没有向量检索。

**三种消息类型**（`[源码] core/src/context/inter_agent_message.rs`、`core/src/session_prefix.rs`）：

| 类型 | 谁发 | 何时 | 效果 |
|---|---|---|---|
| `MESSAGE` | 任意 agent | `send_message` / `send_input`（非 interrupt） | 进对方 mailbox，**不触发轮次** |
| `NEW_TASK` | 任意 agent | `followup_task` / `send_input(interrupt=true)` | 进对方 mailbox，**触发一轮** |
| `FINAL_ANSWER` | 子 agent → 父 | 子 agent 到达终态 | 进父 mailbox，**不触发轮次**（`trigger_turn: false`） |

**mailbox 机制**（`[源码] core/src/agent/control/mailbox.rs`）：

- 每个 agent 一个 `Mailbox { pending: Vec<StoredMail>, state: watch::Sender<bool> }`
- `enqueue` 追加并把 `state` 置为 true；`take` 全部取走并清空 `state`
- `watch` 给出 `watch::Receiver<bool>`，`wait_agent` 靠它阻塞
- mailbox **在 agent 被换出（eviction）时保留**："Retains queue-only mail while local sessions are unloaded. Eviction transfers mail before releasing the recipient's residency guard."
- 有个 gauge `core.mailbox.pending` 统计待收消息数

**两种投递模式**（`[源码] core/src/agent/types.rs`）：

```rust
pub enum MessageDeliveryMode {
    QueueOnly,   // 投邮箱，不启动空闲 agent
    TriggerTurn, // 投当前轮，或 agent 空闲时启动工作
}
```

**等待机制**：
- V1 `wait_agent`：并行等**指定的一批** agent，谁先到终态谁先返回，返回里带最终消息
- V2 `wait_agent`：等**任意** agent 的 mailbox 有动静（`watch` 唤醒），或用户输入被 steer 进来，或超时；**只回摘要不回内容**
- 子 agent 终态回传给父时，走的是 `notify_parent_of_terminal_turn` —— 它把消息投进父的 mailbox（`trigger_turn=false`），并额外向**发起方**发一条 `SubAgentActivityItem{ kind: Completed }` 活动事件（如果发起方不是父）。`[源码] core/src/agent/control/completion.rs`

**能不能读彼此的输出**：**不能直接读**。兄弟 agent 之间的输出没有共享存储，只能靠彼此发消息。父能拿到子的最终答案（通过 `FINAL_ANSWER` 或 V1 的 `wait_agent` 返回值）。子拿不到兄弟的输出。

### 3.4 隔离

**文件系统：无隔离。** Codex 在 V2 给 agent 的系统提示里明写（V2 的 usage hint 文本；V1 无此段，但继承同一套 cwd 语义）：

> All agents share the same directory. In detail:
> - All agents have access to the same container and filesystem as you.
> - All agents use the same current working directory.
> - As a result, edits made by one agent are immediately visible to all other agents.

`[源码] prompts/src/multi_agent_instructions.rs` `DEFAULT_MULTI_AGENT_V2_SHARED_USAGE_HINT_TEXT`

**沙箱与审批：无隔离。** 子 agent 继承父的 sandbox 与 approval policy；角色文件**不能**改这两样。CLI 文档明写：

> Subagents inherit the parent turn's current sandbox and approval settings, including changes made with `/permissions` or `--yolo`. A custom agent file can't replace those settings.

> Custom agent instructions can ask an agent to avoid edits, but they don't create a separate read-only sandbox.

`[文档] agent-configuration/subagents.md`

**进程与线程：无隔离（同一进程内）**。所有子 agent 跑在同一个 Codex 进程里，由 `LocalAgentControl` / `LocalAgentRuntime` 统一管理。`[源码] core/src/agent/control.rs`

**上下文：隔离**。每个 agent 独立 thread、独立 rollout、独立上下文窗口与 compaction 状态。

### 3.5 并发与限额

**两级限制**（`[源码] core/src/agent/registry.rs`、`core/src/agent/control/execution.rs`）：

| 限制 | 作用对象 | 默认 | 触发错误 |
|---|---|---|---|
| `AgentRegistry::reserve_spawn_slot(max_threads)` | **打开的** thread 数（注册表容量） | V1: 6（`DEFAULT_AGENT_MAX_THREADS`）；V2: `max_concurrent_threads_per_session - 1`（默认 4-1=3） | `CodexErrorDetails::AgentLimitReached { max_threads }`（context = `RegistryCapacity`） |
| `AgentExecutionLimiter` | **正在跑**的 turn 数 | 同上 | `AgentLimitReached { max_threads }`（context = `ExecutionCapacity`） |

执行限制**只对 V2 的子 agent 生效**：

```rust
fn is_execution_limited(version, session_source) -> bool {
    version == MultiAgentVersion::V2 && matches!(session_source, SessionSource::SubAgent(_))
}
```

root 与 V1 的 turn 不受限。`[源码] core/src/agent/control/execution.rs`

**每 agent 的 token / 回合预算**：**没有 per-agent 的预算**。`token_budget` 与 `rollout_budget` 是**会话级**的（上下文压缩提醒、rollout 上限），`rollout_budget` 超限时返回 `CodexErr::SessionBudgetExceeded`，由 `LocalAgentControl::record_rollout_budget_usage` 记账。子 agent 继承父的 `token_budget` 配置。`[源码] core/src/agent/control/budget.rs`、`features/src/feature_configs.rs`

**深度**：V1 有 `max_depth`（默认 1）；V2 忽略 depth，但要求模型自身支持 V2 才允许子 agent 再派。`[源码] core/src/tools/spec_plan.rs`、`core/src/config/mod.rs`

**`wait_agent` 的防忙等**：最小超时 10s（V2），提示语明确写 "prefer longer waits (minutes) to avoid busy polling"。`[源码]`

### 3.6 主动性：MultiAgentMode

`[源码] core/src/session/multi_agents.rs` `effective_multi_agent_mode`

```
MultiAgentMode::ExplicitRequestOnly   // 默认
MultiAgentMode::Proactive             // 推理强度为 Ultra 时启用
MultiAgentMode::Custom(text)          // 由 multi_agent_mode_hint_text / 模型目录覆盖
```

即：**默认只有用户明说才委派；推理强度开到 `ultra` 才允许模型主动委派**。官方文档（web 侧）也是这个口径："At most intelligence levels, ask ChatGPT to delegate… With Ultra, ChatGPT can proactively delegate work when parallel agents would materially improve speed or quality."`[文档] agent-configuration/subagents.md`

### 3.7 共享任务板：消息板（实验特性）

这是本轮找到的**唯一真正的共享状态**。`[源码] ext/agent-message-board/`、`codex-rs/core/src/agent_message_board.rs`

**形态：论坛式公告板**，三层：

```
Board
├── Channel（频道，有 description、ACL 权限、created_by）
│   └── Thread（主题，以首帖 message_id 为标识）
│       └── Post（帖子 / 回帖）
```

**数据结构**（`[源码] ext/agent-message-board/src/types.rs`）：`PostMetadata { message_id: Uuid, channel_name, author: AgentPath, thread_id, created_at }`、`PostPreview`（带 `text_preview` / `n_chars` / `truncated`）、`PostContent`（带 `next_offset_chars` 分页续读）、`ThreadSummary`、`ChannelSummary`、`SubscriptionState`、`Page<T>`（游标分页，`next_cursor`）。

**9 个工具**（`[源码] ext/agent-message-board/src/tools/spec.rs`，`NAMES` 数组）：

| 工具 | 参数 | 语义 |
|---|---|---|
| `create_channel` | `channel_name`、`description`、`subscribe` | 建频道（"where all agents in this collaboration can read and post messages"），默认订阅 |
| `get_channels` | `query`、`recent_first`、`limit`、`cursor` | 列频道，按活跃度排序，可按名字片段搜 |
| `list_threads` | `channel_name`、`sort`(`created`\|`activity`)、`recent_first`、`limit`、`cursor`、`max_chars_per_post` | 列线程，带首帖与最新回帖的预览 |
| `search_posts` | `channel_name`、`query`、`after_message_id`、`author`、`limit`、`cursor`、`max_chars_per_post` | 搜帖子（大小写不敏感子串），结果只是预览 |
| `read_thread` | `thread_id`、`limit`、`cursor`、`max_chars_per_post` | 读线程，游标推进回帖 |
| `read_post` | `message_id`、`offset_chars`、`limit_chars` | 按 id 读帖子/回帖全文，**按 Unicode 字符偏移续读**（`next_offset_chars`） |
| `subscribe` | `channel_name` 或 `thread_id`（二选一）、`target_agent` | 订阅频道的新帖或线程的回帖。**"Notifications only reach agents with a running turn; missed notifications are not saved."** |
| `unsubscribe` | 同上 | 退订 |
| `post` | `text`、`channel_name` 或 `new_channel_name` 或 `thread_id`（三者恰一）、`agents_to_notify` | 发帖 / 回帖。`agents_to_notify` 发一次性通知但**不订阅、不启动空闲 agent** |

**后端三选一**（`[源码] core/src/agent_message_board.rs`）：本地 SQLite（`LocalAgentMessageBoard`）、内存（`InMemoryMessageBoards`）、远程 HTTP（`RemoteAgentMessageBoard` + bearer token）。板子**按 agent 树（session tree）隔离**。

**启用条件**：需要 `features.agent_message_board` **且** `features.multi_agent_v2` 都开；或者宿主显式提供 `LocalBoardConnection`（这时 ephemeral 会话也能用）。`[源码] core/src/agent_message_board.rs`

**命名空间**：消息板工具复用 `multi_agent_v2.tool_namespace`（默认 `collaboration`）。`[源码]`

**其他相关 crate**（本轮新发现，笔记里没有）：
- `codex-rs/agent-graph-store` —— 存储无关的父子拓扑（`ThreadSpawnEdgeStatus::{Open, Closed}`），是 agent 树的持久化骨架
- `codex-rs/agent-message-board-client` —— 远程板的 HTTP 客户端 + 通知注册
- `codex-rs/agent-identity` —— agent 身份
- `codex-rs/runtime-agent-instance` —— 运行时 agent 实例（actor / manager / registration）
- `codex-rs/external-agent-migration` —— 从别的 agent 迁入
- `codex-rs/core/templates/agents/orchestrator.md` —— 一份 prompt 模板，开头是「Prefer multiple sub-agents to parallelize your work… If sub-agents are running, **wait for them before yielding**… When you ask sub-agent to do the work for you, your only role becomes to coordinate them」

`[源码]`（各 crate 目录）

### 3.8 worktree 与多 agent 的关系

**结论：基本无关。**

- `--worktree`（`codex` / `codex exec` / `codex fork`）与 `/worktree` 是**chat 级**的：一个 chat 在一个受管 git worktree 里工作。文档明确 "--worktree requires local execution. Don't combine it with `resume`, `review`, `--ephemeral`, or `--ignore-user-config`."`[文档] developer-commands.md`、`environments/git-worktrees.md`
- `codex-rs/worktree` crate 里**没有任何 agent 相关的代码**（grep "agent" 零命中）。`[源码] codex-rs/worktree/src/`
- 子 agent **不**自动获得 worktree；它继承父的 cwd，因此也就在同一个工作目录里。`[源码] core/src/agent/child_config.rs`
- 唯一沾边的是实验特性 `subagent_thread_environments`：打开后，子 agent 用**配置的** environment 快照（而不是当前 turn 的），也就是可以让不同子 agent 待在不同的 thread environment 里。`[源码] core/src/session/mod.rs` `inherited_environment_snapshot`
- 官方文档在 worktree 页完全没提 subagent。`[文档] environments/git-worktrees.md`

---

## 4. 与 heng 的差异

对照 [`docs/executor.md`](../../../docs/executor.md) 与 [`.scratch/agent-teams/seed.md`](../../seed.md)：

| 维度 | Codex | heng 现状 |
|---|---|---|
| 多 agent 的形态 | **委派树**：agent 用工具派 agent，可递归（V2） | **委派链**：讨论者用 `task(brief)` 派**执行者**，执行者**不能再派** |
| 执行者的身份 | 独立 thread（`ThreadId`）+ 独立 rollout | **嵌套会话**，落在**同一条**会话流上（`ExecutorSpawned` / `ExecutorFinished`） |
| 兄弟之间通信 | **可以**（mailbox + `send_message` / `followup_task`） | **没有** —— 「会话之间没有任何通道」 |
| 共享状态 | 消息板（实验）：channel / thread / post + ACL + 订阅 | **没有** —— 「没有消息投递，也没有跨会话共享的待办」 |
| 上下文继承 | 三种：仅初始消息 / 复制父历史 / 角色指令叠加 | 执行者只拿 `brief`（投影成第一条发言，署名派发者） |
| 回传 | **最终答案全文**（`FINAL_ANSWER`） | **一份摘要加元数据**，从不报过程 |
| 地址 | **规范路径树** `/root/task1/task_3`，相对名与规范名互换 | 无地址概念（只有 `executor_id`） |
| 寻址与同伴 | agent 知道自己是谁、有哪些同伴（`- task_3: nickname` 清单） | 无 |
| 隔离 | 无文件隔离（明写共享 cwd）、无沙箱隔离 | 执行者与派发者**共用工作区**，写排斥靠共享的 `PathLocks` |
| 并发 | 两级限额（打开 thread 数 / 正在跑 turn 数），默认 V1=6、V2=3 | `task` 批量并行，靠 `PathLocks` 做写排斥 |
| 预算 | 会话级 `token_budget` / `rollout_budget`；**无 per-agent** | 执行者**有自己的回合预算**（`docs/executor.md`） |
| 委派触发 | 默认必须用户明说；`ultra` 推理强度才允许主动委派 | 由讨论者决定 |
| 「团队」这个词 | **Codex 全树零命中** | seed.md 的核心词 |

**Codex 有而 heng 没有的**（可直接抄）：规范路径树与同伴清单、留言 vs 派活两种投递语义分离、`wait` 只等信箱有动静而不回内容、角色（agent type）与角色说明书分离、消息板这种「论坛式共享板」。

**heng 有而 Codex 没有的**：执行者**自己的回合预算**；执行者与派发者**落在同一条事件流**（于是 `/undo` 覆盖得到）；`PathLocks` 做写排斥；「多角色轮流发言」的讨论会话。

---

## 5. 可移植部分（给 heng 的借鉴点）

### 5.1 最值得抄的三个机制

**(1) 规范路径树 + 同伴清单**

Codex 给每个 agent 一个 `AgentPath`（`/root/task1/task_3`），并把它**注入子 agent 的环境上下文**：

```
- task_3: Herodotus
- task_4: Ibn Battuta
```

（`format_subagent_context_line`）`[源码] core/src/agent/control/runtime_context.rs`、`core/src/session_prefix.rs`

价值：**agent 知道自己在团队里的位置，也知道同伴是谁**。这正好接上 seed.md 待谈分叉第 7 条（「要不要一条『这个目录下现在有谁』的清单」）。Codex 的答案是：**清单进每个 agent 的上下文**，不是一个只给人看的 UI。

**(2) 留言与派活分成两个动作**

Codex V2 把「说句话」和「派个活」拆成两个工具，因为它们的**唤醒语义不同**：

- `send_message` → QueueOnly → 消息进 mailbox，**不启动**空闲 agent
- `followup_task` → TriggerTurn → 消息进 mailbox，**并启动**空闲 agent 跑一轮

`[源码] core/src/agent/types.rs` `MessageDeliveryMode`

这正好接上 seed.md 待谈分叉第 3 条（「收件方怎么被唤醒：排到边界、立刻打断，还是只在空闲时投递」）——**Codex 的答案是：两种都要，而且要用不同的工具名显式区分**，不要用一个 `send` 加一个 flag。

**(3) `wait` 只等「有动静」，不回内容**

Codex V2 的 `wait_agent` 描述：

> Wait for a mailbox update from any live agent… Does not return the content; returns either a summary of which agents have updates (if any), an interruption summary for steered input, or a timeout summary if no activity arrives before the deadline.

`[源码] core/src/tools/handlers/multi_agents_spec.rs`

价值：**把「轮询」从模型身上卸掉**。模型调一次 `wait_agent`，阻塞在 `watch` 上，有消息才醒；内容自己进上下文，不需要模型手动 `read` 一次。这比「wait 返回全文让模型自己挑」更省 token，也更不容易漏。对应 heng 的 seed 第 3 条：唤醒应该发生在**边界**（消息边界 / pending 工具调用后），而不是打断当前轮 —— Codex 的 `followup_task` 描述正是这么写的："deliver the task promptly at message boundaries while sampling, or after the pending tool call completes"。

### 5.2 次一级可抄的

- **角色 = 一份 config.toml 覆盖层 + 一段给模型看的说明书**。`name` 是唯一真源，`description` 是「什么时候该用它」，`developer_instructions` 是「它该怎么干」，`nickname_candidates` 是可读性。**角色不含能力边界**（Codex 明说角色文件不能改 sandbox / approval / provider / MCP）——这是干净的边界划分。`[源码] agent-roles/src/agent_role_config.rs`
- **禁递归要有，但别做成硬限制**。Codex V1 有 `max_depth`（默认 1），V2 直接**取消深度限制但改成要求模型自己支持 V2** —— 递归能力绑到「模型是否声明支持」，而不是绑到运行时开关。`[源码] core/src/tools/spec_plan.rs`
- **父 agent 不自动汇总子 agent 的过程**。Codex V2 里子 agent 的中间产物**不进**父的上下文，只有 `FINAL_ANSWER` 进。heng 的执行者已经是这个形状（只回摘要加元数据）——**heng 这条比 Codex 更严格，值得保留**。
- **完成的 agent 仍然占并发额度**，必须显式关闭（V1 的 `close_agent` 描述明说）。这是 heng 做团队并发配额时该照抄的细节：**「完成 ≠ 释放」**。
- **共享板的形状**：channel → thread → post 三层 + 游标分页 + 按字符偏移续读 + 订阅 + ACL + 「通知只送达正在跑的 agent，漏掉的不会保存」。这是把「共享待办」做成「论坛」而不是「一张 JSON 数组」的完整形态。`[源码] ext/agent-message-board/src/types.rs`、`tools/spec.rs`

### 5.3 不可移植 / 依赖 Codex 内部实现的

- **V1 / V2 双轨**：Codex 自己在两代之间迁移（V1 的 `close_agent` / `resume_agent` 在 V2 里没有，V2 把 `send_input` 拆成 `send_message` + `followup_task`）。**照抄某一代会抄到过渡态**；建议只抄语义，不抄工具名。`[源码]`
- **`ResponsesApiNamespace` 工具命名空间机制**（`multi_agent_v1.*` / `collaboration.*`）是 Codex 对接 OpenAI Responses API 的产物，别的 provider 未必有对应概念。`[源码] codex-rs/tools/src/`
- **`AgentRegistry` 的昵称池**（"name the 2nd"、随机取、池空后清空重置 + `codex.multi_agent.nickname_pool_reset` 计数器）纯属实现趣味，与团队语义无关。`[源码] core/src/agent/registry.rs`
- **消息板是 extension crate**（`codex-rs/ext/agent-message-board/`），走 Codex 的 extension registry 与 host 抽象；要抄得连 host 抽象一起抄，工作量不小。`[源码]`
- **`AgentPath` 与 `codex_protocol::AgentPath`** 是 Codex 的协议类型；V2 的「相对名 / 规范名互换指代」依赖「同名节点在兄弟路径下不可见」这个树形约束（`/root/task2/task_3` 只能被 `/root/task2/task_3` 指到）。`[源码] protocol/src/`

---

## 6. 查不到 / 不确定 / 明确不存在

| 问题 | 结论 |
|---|---|
| Codex 有没有「agent teams」？ | **没有这个概念**。`codex-rs` 全树 grep `agent team` / `agent_team` / `AgentTeam` **零命中**；官方文档也从不使用该词。Codex 的说法是 subagent / subagent workflow。`[源码]`、`[文档]` |
| 官方有没有 multi-agent 专门文档页？ | **没有**。只有 `agent-configuration/subagents.md` 一页（2026-10-20 抓取）。`developers.openai.com/codex/multi-agent` 308 跳到 `learn.chatgpt.com`，无对应 slug。`[文档]` |
| 官方文档讲 `multi_agent_v2` 吗？ | **不讲**。V2 的全部配置（`tool_namespace`、`wait_agent_enabled`、`message_board_*`、`preserve_fork_prefix` 等）只在源码与 `config.schema.json` 里，文档站没有。 |
| 消息板 / 共享任务板可用吗？ | **不可用（默认）**。`agent_message_board` 是 `Stage::UnderDevelopment`、`default_enabled: false`，且要求 `multi_agent_v2` 同时开；两者默认都关。属于「源码里已就绪、对外不可用」。`[源码]` |
| 子 agent 有独立 token / 回合预算吗？ | **没有**。只有会话级 `token_budget` / `rollout_budget`（超限抛 `SessionBudgetExceeded`）。V1 `max_depth` 是深度不是预算。`[源码]` |
| 子 agent 能读兄弟的输出吗？ | **不能**，除非兄弟互相发消息。没有共享输出存储。`[源码]` |
| 有共享的待办 / 任务板（稳定版）吗？ | **没有**。`update_plan` 是 per-thread 且 **Codex 不持久化它**（只发 `EventMsg::PlanUpdate` 事件），而且默认关闭（`tools.update_plan.enabled` 默认 false）。`[源码] core/src/tools/handlers/plan.rs` |
| 子 agent 有自己的 worktree 吗？ | **没有**。`--worktree` / `/worktree` 是 chat 级的；子 agent 继承父的 cwd，共享工作区。唯一沾边的是实验特性 `subagent_thread_environments`。`[源码]`、`[文档]` |
| V1 在当前版本还活着吗？ | **活着，且是默认路径**（`features.multi_agent` Stable + 默认开 → `multi_agent_version_from_features()` 返 V1）。V2 只在 feature 开或模型目录声明 `multi_agent_version: V2` 时接管。`[源码] core/src/config/mod.rs` |
| V2 是不是 research preview？ | 官方**没有**这个措辞。源码侧 `Stage::Stable` + `default_enabled: false`；文档侧完全不提 V2。本轮**未找到**任何官方把 multi-agent 标为 "research preview" / "experimental" 的说法。`[源码]`、`[文档]` |
| 官方 blog / changelog 有多 agent 的公告吗？ | 本轮**未查到**。仓库 `CHANGELOG.md` 是 stub（笔记 §2.18 已记）；`learn.chatgpt.com/docs/whats-new/` 有周更页但本轮未逐页核对是否提到 subagent。**标为未核实**。 |
| `codex exec` 的 JSON 事件里怎么表达 agent 活动？ | 有 `collab_tool_call` item 类型；协议层枚举 `CollabAgentTool::{SpawnAgent, SendInput, ResumeAgent, Wait, CloseAgent, SendMessage, FollowupTask, InterruptAgent, ListAgents}`，状态 `InProgress / Completed / Failed / Interrupted`，带 `sender_thread_id` / `receiver_thread_ids` / `receiver_agents` / `agents_states`。`[源码] protocol/src/items.rs`、`app-server-protocol/src/protocol/event_mapping.rs` |

---

## 7. 来源清单

### 7.1 上游源码（`github.com/openai/codex`，浅克隆，commit `e1b9c9457ceca75a99b034a5df39ac48c3998d15`，2026-10-20）

全部路径相对仓库根 `codex-rs/`；引用时补全为 `https://github.com/openai/codex/blob/main/<path>`。

**多 agent 核心**
- `features/src/lib.rs` —— feature flag 表（`multi_agent`、`multi_agent_v2`、`agent_message_board` 等）与 `Stage` / `default_enabled`
- `features/src/feature_configs.rs` —— `MultiAgentV2ConfigToml`、`TokenBudgetConfigToml`、`RolloutBudgetConfigToml`
- `core/src/tools/handlers/multi_agents_spec.rs` —— **两代工具的 schema、参数、返回、描述文本（最关键的一份）**
- `core/src/tools/handlers/multi_agents/`（`spawn.rs` / `send_input.rs` / `wait.rs` / `resume_agent.rs` / `close_agent.rs`）—— V1 执行
- `core/src/tools/handlers/multi_agents_v2/`（`spawn.rs` / `message_tool.rs` / `followup_task.rs` / `interrupt_agent.rs` / `list_agents.rs` / `wait.rs`）—— V2 执行
- `core/src/tools/handlers/multi_agents_common.rs` —— 超时常量、错误映射、`parse_collab_input`
- `core/src/tools/spec_plan.rs` —— 工具注册、`collab_tools_enabled`、`required_child_management_tool_names`
- `core/src/tools/multi_agent_tool.rs` —— namespace 覆盖层
- `core/src/agent/registry.rs` —— agent 路径树、昵称池、spawn 额度
- `core/src/agent/control.rs` + `core/src/agent/control/{mailbox,delivery,completion,execution,budget,runtime_context,spawn}.rs` —— 编排运行时
- `core/src/agent/types.rs` —— `MessageDeliveryMode`、`SpawnAgentOptions`、`AgentMetadata`
- `core/src/agent/role.rs` —— 内置角色、`apply_role_to_config`
- `core/src/agent/child_config.rs` —— 子 agent 配置解析与继承规则
- `core/src/thread_spawn.rs` —— spawn 主流程
- `core/src/agent_communication.rs` —— 通信遥测
- `core/src/context/inter_agent_message.rs` / `inter_agent_completion_message.rs` —— 消息体渲染格式
- `core/src/session_prefix.rs` —— `format_inter_agent_completion_message`、`format_subagent_context_line`
- `core/src/session/multi_agents.rs` —— `MultiAgentMode`、`resolve_usage_hints`
- `core/src/config/mod.rs` —— 并发常量、`multi_agent_version_from_features`、`MultiAgentV2Config` 默认值
- `config/src/config_toml.rs` —— `AgentsToml`、`AgentRoleToml`
- `core/config.schema.json` —— 配置键的权威清单
- `prompts/src/multi_agent_instructions.rs` —— **「所有 agent 共享同一目录」的明文**
- `core/templates/agents/orchestrator.md` —— 编排者 prompt 模板
- `protocol/src/protocol.rs` —— `MultiAgentVersion`、`InterAgentCommunication`、`SessionSource::SubAgent`
- `protocol/src/items.rs` —— `CollabAgentTool`、`CollabAgentToolCallItem`

**角色**
- `agent-roles/src/discovery.rs` —— `~/.codex/agents/` 递归扫描
- `agent-roles/src/loader.rs` —— 分层加载、合并、重复检测
- `agent-roles/src/agent_role_config.rs` —— 角色文件 schema 与校验
- `core/assets/agent/builtins/{explorer,awaiter}.toml` —— 内置角色配置

**共享板（实验）**
- `ext/agent-message-board/src/types.rs` —— channel / thread / post 数据结构
- `ext/agent-message-board/src/tools/spec.rs` —— 9 个工具的 schema 与描述
- `ext/agent-message-board/src/{api,host,in_memory,local,permissions}.rs`
- `agent-message-board-client/src/{client,protocol,notifications}.rs`
- `core/src/agent_message_board.rs` —— 安装与启用条件
- `agent-graph-store/src/{store,local,types}.rs` —— 父子拓扑

**worktree**
- `worktree/src/` —— 全部；其中无任何 agent 相关代码

### 7.2 官方文档（抓取日期均为 **2026-10-20**）

- [Subagents](https://learn.chatgpt.com/docs/agent-configuration/subagents.md) —— 可用性、内置角色、自定义角色文件 schema、`[agents]` 全表、审批与沙箱继承、模型与推理强度优先级
- [Developer commands（CLI）](https://learn.chatgpt.com/docs/developer-commands.md?surface=cli) —— `/subagents`、`/agents`、`/worktree`、`codex features enable/disable`、`--enable/--disable`
- [Worktrees](https://learn.chatgpt.com/docs/environments/git-worktrees.md) —— worktree 是 chat 级隔离，与 subagent 无关
- [Codex 文档索引](https://learn.chatgpt.com/llms.txt) —— 全站页面清单（确认没有独立的 multi-agent 页）

### 7.3 本地笔记（只读引用）

- [`docs/research/notes/codex-gemini.md`](../../../docs/research/notes/codex-gemini.md)
  - §2.1 Agent loop、§2.2 工具集、§2.5 权限、§2.7 会话、§2.8 plan/todo、§2.9 **Subagents & parallel orchestration**、§2.11 worktree、§2.15 slash commands、§2.18 验证不存在项、§2.19 版本偏差警告
  - 本轮**沿用**的事实：V1 namespace 与工具名、V2 扁平工具名、并发常量（6 / 4 / max_depth 1）、`AgentLimitReached`、`[agents]` 键、`~/.codex/agents/` 路径与三个必填字段、内置 `default`/`worker`/`explorer`、`wait_agent` 的 `FuturesUnordered` fan-out、子 agent 继承 sandbox 与 approval
  - 本轮**修正/细化**的点：V2 的 `Stage` 是 Stable（笔记未记 stage）；V2 的 `task_name` 规范路径语义、`fork_turns` 默认值；`send_message` 与 `followup_task` 的投递语义差别；消息体格式与三种消息类型；mailbox 机制；两级并发限制；`update_plan` 不持久化；worktree 与子 agent 无关；Codex 无 "agent team" 概念

### 7.4 二手材料（仅作线索，本文件未据其下结论）

- 社区讨论 "Restoring subagent roles, model and reasoning in multi_agent_v2"（OpenAI Developer Community）—— 提示 V2 的角色/模型覆盖在实际使用中有摩擦，与本轮读到的 `child_config.rs` 优先级逻辑一致
- 若干第三方 skill 仓库里的 Codex 工具映射表 —— 只用于确认工具名拼写，未引用其结论
