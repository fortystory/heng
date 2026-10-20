# 除 Claude / Codex / DSH 之外的主流 coding agent 的「多 agent / 委派 / 并行」形态

> **抓取日期：2026-10-20**（下文每条 URL 都在这一天实际抓过；只写抓到的内容，没抓到的明说没抓到）。
> **范围**：Gemini CLI、OpenCode、aider、OpenHands、Cline、Continue、Goose、Crush、Amp、SWE-agent。
> **不覆盖**：Claude Code、OpenAI Codex、DeepSeek Harness（DSH）——它们由本轮另外的文件负责。
> **对照材料**：本仓库已有的 `docs/research/coding-agent-features.md`（横向对比入口）与
> `docs/research/notes/` 下四份上游笔记。凡是本地笔记已经写下的事实，本文标「**本地笔记已有**」，
> 并只补本轮的新问题（形态归类、通信机制、共享状态落点、可见性、权限隔离）以及上游复核结果。
> **本地笔记没有修改。**
> **二手来源**：只当线索，不当依据。本文所有事实性断言都指向官方文档站或官方 GitHub 仓库。

---

## 0. 结论摘要

1. **业界主流是形态 A（父 agent 用一个工具派生子 agent），不是形态 B（平级 agent + 共享板）。**
   Gemini CLI 的 subagents、OpenCode 的 `subagent` 工具、Cline 的 `spawn_agent`、OpenHands 的
   `TaskToolSet`、Goose 的 `delegate`、Amp 的专职 subagent —— 全部是「父调用一个工具 → 子跑完 →
   摘要回到父的上下文」这一种形状。本仓库的 Executor 正是这一类。

2. **真正的「平级 agent + 共享板」一手先例，本轮只找到一个：Cline 的 Agent Teams。**
   它明文写着「coordinate through a shared task board」，并把 **task board + mailbox + mission log**
   三样东西存在 `~/.cline/data/db/teams.db`、按 session id 归档，会话恢复时团队回来。
   除此之外，Goose 反而在文档里明说自己那套多机委派是 **"without any shared state"**。

3. **共享状态通常不落在「板」上，落在「会话/事件流」上。** 具体分布是：
   - 内存/上下文里的一份待办（Gemini 的 `write_todos` 明确「Todo state is scoped to the current
     session」；OpenHands 的 `TaskTrackerTool` 在 conversation 里；opencode 的 `todowrite` 默认对
     subagent 关闭）；
   - 一份可追加的事件流 / 会话数据库（OpenHands 的 event stream、goose 的 `sessions.db` 用
     `parent_session_id` 表达父子、Cline 的 `sessions.db`）；
   - **只有 Cline 一家把「板 + 信箱 + 任务日志」当成一个一等对象持久化。**

4. **形态 C（运行时/编排器调度）走的是另一条路：把 agent 拆成独立进程 / 容器 / 远程服务，
   用协议而不是共享状态来连接。** 代表是 Cline 的 hub-spoke、Cline Kanban 的 ephemeral git
   worktree、OpenHands 的 agent-server + workspace、Gemini CLI 的 A2A 远端子 agent、Goose 的
   roaming（iroh QUIC 上的 ACP）、Amp 的 orb / runner。这些形态的通信靠 **A2A / ACP / MCP / WebSocket**
   这类协议，共享的东西是运行时状态，不是任务板。

5. **形态 D（同一上下文内的角色分工）几乎零成本、零隔离。** aider 的 architect/editor 是纯形态 D：
   同一段对话、两次 LLM 请求、先出方案再出编辑指令。heng 的「讨论会话」（一个会话多角色轮流发言）
   也在这一类。

6. **并发写的冲突面是形态 B 的真实代价，各家的答案很朴素。** Goose 的纪律是一句话：
   「no two delegates touch the same file」；Cline 的答案是每个任务一张 ephemeral git worktree。
   没有任何一家做了「共享板 + 共享工作区 + 自动冲突消解」。

7. **可借来直接抄的三个小设计**：
   (a) **Cline 把「派出去」和「等结果」拆成两个工具**（`team_run_task` / `team_await_runs`），
   于是父不必被阻塞，成员之间可以互相 await —— 这正好接住 seed 里的分叉 3；
   (b) **Cline Kanban 卡片上只显示「最新一条消息或工具调用」**，详情页才给完整 TUI + diff；
   (c) **goose 的 `delegate` 工具描述本身写清了协作纪律**（谁能写、谁不能协调），把「编排规则」
   放进工具描述而不是系统提示 —— 与 heng 现在把执行者纪律写进 `EXECUTOR_IDENTITY` 的做法同源。

---

## 1. 逐家要点

### 1.1 Gemini CLI — 形态 A（工具派生），带一条通往形态 C 的线（A2A）

- **子 agent 的定义文件是 Markdown + YAML frontmatter，不是 TOML**：
  `.gemini/agents/*.md`（项目级，团队共享）与 `~/.gemini/agents/*.md`（用户级）。
  frontmatter 字段：`name`（必填）、`description`（必填）、`kind`（`local` 默认 / `remote`）、
  `tools`（支持通配 `*`、`mcp_*`、`mcp_server_*`）、`mcpServers`（该 agent 专属的内联 MCP server）、
  `model`（默认 `inherit`）、`temperature`、`max_turns`（默认 30）、`timeout_mins`（默认 10）。
  正文即该 agent 的 system prompt。
  <https://raw.githubusercontent.com/google-gemini/gemini-cli/main/docs/core/subagents.md>
- **子 agent 以同名工具暴露给主 agent**：主 agent 调它，它跑完把 findings 报回来。
  `@name` 语法可强制（CLI 注入一条 system note 让主模型立刻用那个 subagent 工具）。
  内置四个：`codebase_investigator`、`cli_help`、`generalist`、`browser_agent`（后者默认关闭）。
  同上 URL。
- **递归保护**：subagent **不能调用其它 subagent**；即使给它 `*` 通配也看不到别的 agent。
  同上 URL。
- **工具隔离与策略按 agent 名限定**：`tools` 列表决定它能看见什么工具；`mcpServers` 让每个 agent
  有自己的 MCP server；Policy Engine 的 TOML 规则可加 `subagent = "<name>"` 限定作用范围，
  subagent 在策略匹配时被当作**虚拟工具名**。同上 URL（+ `docs/reference/policy-engine.md`）。
- **远端子 agent（形态 C 的入口）**：走 **A2A 协议**。`kind: remote` + `agent_card_url` 或
  `agent_card_json`；认证支持 `apiKey` / `http` / `google-credentials` / `oauth`（OAuth 走 PKCE）；
  密钥可写成 `$ENV_VAR` 或 `!command` 动态求值。
  <https://raw.githubusercontent.com/google-gemini/gemini-cli/main/docs/core/remote-agents.md>
- **Extensions 是自研工具的打包层**：一个扩展 = `gemini-extension.json` + `commands/`（TOML 自定义命令）+
  `hooks/hooks.json` + `skills/` + `agents/`（子 agent）+ `policies/`（贡献策略规则，tier 2）+ `themes/`。
  扩展不继承用户全部环境变量，只拿到标准安全变量与 manifest 里 `settings` 声明的 `envVar`。
  <https://raw.githubusercontent.com/google-gemini/gemini-cli/main/docs/extensions/reference.md>
- **共享状态**：`write_todos` 工具，**状态只属于当前会话**；同一时刻只允许一个任务 `in_progress`；
  `Ctrl+T` 展开完整清单。没有跨 agent 的板。
  <https://raw.githubusercontent.com/google-gemini/gemini-cli/main/docs/tools/todos.md>
- **可见性**：终端内 todo 面板在输入框上方实时更新；没有团队面板。

### 1.2 OpenCode — 形态 A（v1 `task` 工具 / v2 `subagent` 工具），带 worktree 与 Code Mode

- **v1**（`opencode.ai/docs/agents/`）：primary 与 subagent 两类；subagent 由主 agent 自动调用或用
  `@` 提及手动调用；内建 `general` / `explore` / `scout`，隐藏 `compaction` / `title` / `summary`。
  配置可写 `opencode.json`，也可写 Markdown 到 `.opencode/agents/` 或 `~/.config/opencode/agents/`。
  **本地笔记已有**（`notes/opencode-goose.md` §8、`coding-agent-features.md` §8），今天上游复核一致。
  <https://opencode.ai/docs/agents/>
- **v2（当前主线）**：`mode` 取 `primary` / `subagent` / `all`；`subagent` 工具启动一个子会话，
  **前台等结果，或 `background: true` 立刻返回、完成后通知父**；返回值里的 `sessionID` 可以继续
  那个子会话；**默认嵌套深度为一**。父的 `subagent` 权限项按 agent id 匹配来控制能起哪些子 agent。
  内建 `general` 明确「cannot launch more subagents」；v2 **没有** `scout`。
  <https://opencode.ai/v2/docs/agents/>
- **v2 的工具面新增两块与「团队」相关的东西**：
  - `execute`（Code Mode，跑 JS 编排工具调用）——文档明说它「is useful for parallel independent
    calls」，把并行与结果处理从模型上下文里移走；
  - **worktree 工具**：`worktree_create` / `worktree_list` / `worktree_remove`，与
    `session_move`（把会话搬进某个 worktree）配合。
    <https://opencode.ai/v2/docs/tools/>
- **权限继承**（v1，本地笔记已有）：子 agent **只继承父的 deny 与 `external_directory`**，
  `todowrite` 与 `task` 强制拒绝；`subagent_depth` 默认 1。
- **共享状态**：没有板。父子之间靠「子会话 + 父上下文里的结果」。

### 1.3 aider — 形态 D 的纯形态（同一上下文内的角色分工）

- **architect 模式是两次顺序的 LLM 请求，不是两个 agent**：主模型先作为 architect 提出方案，
  aider 再把请求发给「editor model」，让它把方案翻译成具体的编辑指令（用 `editor-diff` /
  `editor-whole` 这两种窄化 prompt 的 edit format）。
  <https://aider.chat/docs/usage/modes.html>
- **为什么值得记**：这是「分工但同一上下文」的最小实现 —— 没有子会话、没有共享板、没有隔离、
  父能看到全过程，代价是**多一次 LLM 请求**（更慢更贵）。文档明说：某些模型不能一次给出
  「方案 + 详细编辑」，配一个 editor model 会更好；配同一个模型跑两遍有时也更好。
  同上 URL。设计说明：<https://aider.chat/2024/09/26/architect.html>
- **本地笔记已有**（`notes/aider-openhands.md` §1.8）：aider **没有** subagent、**没有**并行编排；
  `--editor-model` / `--editor-edit-format` / `--auto-accept-architect`（默认 True）。今天上游复核一致。
- **形态判定**：D。同上下文、零隔离、串行、共享状态 = 无。

### 1.4 OpenHands — 形态 A 为主，运行时层（形态 C）做得最重

- **V1 SDK 的 `TaskToolSet` 明写是阻塞的**：「Each sub-agent runs **synchronously** — the parent
  blocks until the sub-agent finishes and returns its result.」子 agent 可以用 task ID 事后恢复
  （保留完整会话上下文）。
  <https://docs.openhands.dev/sdk/guides/task-tool-set>
- **并行靠 `tool_concurrency_limit`**，**默认 1（串行）**，且标为 experimental；文档明写它的三大
  用法之一就是「Enables parallel sub-agent delegation」，并警告并发会「race conditions」。
  <https://docs.openhands.dev/sdk/guides/parallel-tool-execution>
- **本地笔记已有**（`notes/aider-openhands.md` §2.2）：另有 `delegate` 工具（`DelegateAction`，
  command 枚举 `["spawn","delegate"]`）；子 agent 定义是磁盘上的 markdown + frontmatter
  （`tools` / `skills` / `mcp_config` / `permission_mode` / `max_iteration_per_run` /
  `max_budget_per_run` / `hooks` / `condenser` / `color`）；官方例子
  `examples/01_standalone_sdk/45_parallel_tool_execution.py` 用 `tool_concurrency_limit=4`
  搭「编排者并行委派多个子 agent、子 agent 内部再并行跑工具」。
- **本地笔记已有**（同文件 §2.1）：V0 的委派是**严格串行且有损**的 —— `self.delegate` 是单个属性，
  有 delegate 时父不能动；父**看不到**子轨迹（`_init_history` 删掉了 delegate 前后的全部事件），
  交接是一坨原始文本（源码里还留着 `# TODO: replace this with AI-generated summary (#2395)`）。
- **运行时边界（形态 C）**：V1 把沙箱从「强制」改为「可选」，改由 workspace 抽象承担
  （`LocalWorkspace` / `DockerWorkspace` / `ApptainerWorkspace` / `OpenHandsCloudWorkspace` /
  `RemoteAPIWorkspace`）；agent-server 提供 REST + WebSocket 多用户 API。
  **本地笔记已有**（`notes/aider-openhands.md` §2.0、§2.2）。
- **共享状态**：V0 是 **event stream 作为唯一真相源**（agent / runtime / 前端都订阅它）；
  V1 是不可变、只追加的事件日志 + `EventLog`。但这是**单个 conversation 内**的共享，不是跨 agent 的板。
  待办工具（V0 `task_tracker` / V1 `TaskTrackerTool`）也属于 conversation。

### 1.5 Cline — **本轮唯一的形态 B 先例**，外加最完整的形态 C 运行时

- **Agent Teams（只在 CLI / SDK / Kanban，IDE 扩展不支持）**：原文是
  「Agent teams let you break complex work across multiple agents that coordinate through a
  **shared task board**. One agent acts as the **coordinator**, delegating subtasks to specialist agents.」
  入口是 `/team`：`cline "/team Plan and implement user authentication with tests"`。
  <https://raw.githubusercontent.com/cline/cline/main/docs/cli/agent-teams.mdx>
- **团队状态 = task board + mailbox（agent 之间的消息）+ mission log**，持久化在
  `~/.cline/data/db/teams.db`，**按 session id 归档**；会话恢复时团队与成员一起回来。
  同上 URL + <https://raw.githubusercontent.com/cline/cline/main/docs/sdk/guides/multi-agent-teams.mdx>
- **协调者的工具面**（这是最值得抄的一份清单）：
  - 成员：`team_spawn_teammate` / `team_shutdown_teammate` / `team_status`
  - 任务板：`team_task`（create / list / claim / complete / block）
  - 跑任务：`team_run_task` / `team_list_runs` / `team_await_runs` / `team_cancel_run`
  - 消息：`team_send_message` / `team_broadcast` / `team_read_mailbox` / `team_mission_log`
  - 合并结果：`team_create_outcome` / `team_attach_outcome_fragment` / `team_review_outcome_fragment` /
    `team_finalize_outcome` / `team_list_outcomes`
  - 收尾：`team_cleanup`
  成员拿到除 `team_spawn_teammate` 外的同一套工具；只有协调者能关掉成员与清理团队。
  同上 URL。
- **Sub-Agents vs Teams 的对照表（官方给的判据，本轮最有价值的一段原文）**：

  | 特性 | Sub-Agents | Teams |
  |---|---|---|
  | 开启方式 | `enableSpawnAgent: true` | `enableAgentTeams: true` |
  | 持久化 | 仅会话内 | 会话恢复时回来 |
  | 协调 | 父-子 | **平级，带任务板** |
  | 共享状态 | **无** | 任务板、信箱、任务日志 |
  | 适合 | 一次性委派 | 需要跨天恢复的复杂项目 |

  同上 URL。
- **Sub-Agents（形态 A）**：独立上下文窗口；按当前模式拿工具（Act 可读写可执行命令，Plan 下拒绝
  会改文件的 shell 命令）；**子 agent 可以再起子 agent**；不拿 MCP 工具；主 agent 等它们跑完再继续；
  **批准 `spawn_agent` 就等于批准了它之后做的一切**（子 agent 内部不再逐次问）。
  <https://raw.githubusercontent.com/cline/cline/main/docs/features/subagents.mdx>
- **Kanban（形态 B 的另一面：共享看板 + worktree 隔离 + 依赖链）**：卡片 = 任务；
  点播放按钮就为该任务建一个 **ephemeral git worktree**（gitignore 的文件用符号链接回主仓）；
  **多个任务并行，各在自己的 worktree 里跑，因此不会互相产生 merge 冲突**；
  卡片之间可连成**依赖链**，前一张完成时下一张**自动开始**；卡片上显示 agent 的**最新一条消息或
  工具调用**；详情页给**完整的 agent TUI** 与该 worktree 相对基线分支的 diff（带 checkpoint 范围
  与行内评论回传给 agent）；每张卡有 resume ID。
  <https://raw.githubusercontent.com/cline/cline/main/docs/kanban/core-workflow.mdx>
- **Hub-spoke（形态 C）**：hub 守护进程（默认 `127.0.0.1:25463`）协调会话状态与事件路由，
  spoke 进程跑 agent loop，client（CLI / VS Code / JetBrains）以 WebSocket 挂上来。
  规则是「clients participate, spokes execute, the hub coordinates. No two roles should overlap.」
  能力中介：client 注册时申报能力（open-file / reveal-diff / run-build / shell / run-tests），
  hub 把请求路由给能处理的那个 client；会话存储是 `~/.cline/data/db/sessions.db` +
  `~/.cline/data/sessions/<id>/{metadata,messages}.json`，参与者角色有
  `creator` / `participant` / `observer`；client 掉线，spoke 继续跑，另一个 client 可以中途接上事件流。
  <https://raw.githubusercontent.com/cline/cline/main/docs/sdk/architecture/hub-spoke.mdx>
- **本地笔记已有**（`notes/cline-continue.md` §1.1、§1.8）：Focus Chain（todo 列表）已废弃且无替代；
  替代品是 `/deep-planning` 的 `implementation_plan.md` 与 Kanban 的任务板。

### 1.6 Continue — 无多 agent 形态（仓库冻结）

- **仓库已只读、停止维护**，最终版 2.0.0；README 原文「no longer actively maintained and is
  read-only for all users」。
  <https://raw.githubusercontent.com/continuedev/continue/main/README.md>
- **本地笔记已有**（`notes/cline-continue.md` §0）：同样的结论。
- **本轮补查**：官方文档导航里只有 Agent / Chat / Edit / Autocomplete 四种模式加 Continue CLI，
  **没有多 agent 或 subagent 页面**。搜索命中 `continuedev/continue` PR #9128「feat: subagents」，
  但**本轮未能从一手源确认它是否合入并随 2.0.0 发布** —— 标 **未证实**，不作为依据。
- **形态判定**：无。（本地笔记另有一条：Continue 的并行工具调用**只对只读工具开放**，且这条规则
  只写在 system prompt 里、不在文档里。）

### 1.7 Goose — 形态 A（`delegate`），子会话是持久化的一等公民；并有过平级消息的先例但已收掉

- **`summon` 平台扩展（默认启用）提供 `delegate` 与 `load` 两个工具**。`delegate` 的参数是
  `instructions` / `source` / `parameters` / `extensions` / `provider` / `model` / `temperature` /
  `max_turns` / `context` / `working_dir`。
  <https://raw.githubusercontent.com/aaif-goose/goose/main/crates/goose/src/agents/platform_extensions/summon.rs>
- **工具描述里写着协作纪律**（值得抄的原文）：
  「Delegates know only instructions + source content.」「**Delegates cannot coordinate.
  Same-file work = conflicts.**」「You get a delegate's result back once it finishes.」
  「Research (read-only): delegates explore and report back. **Work (writes): partition files strictly
  - no two delegates touch the same file.**」同上 URL。
- **子 agent = 一个持久化的子会话**：`SessionType::SubAgent`，用 `parent_session_id` 挂到父会话上；
  `working_dir` 必须在父会话工作目录之内；子 agent 继承父的 goose mode。
- **新增 `SourceType::Agent`**：goose 现在会扫描 `.goose/agents` / `.claude/agents` / `.agents/agents`
  （项目级）与 `~/.goose/agents` / `~/.agents/agents` / `<config>/agents` / `~/.claude/agents`（全局）
  里的 markdown + frontmatter（`name` / `description` / `model`），把它们变成可按名 `delegate` 的
  「命名子 agent」；用户消息里写 `@<name>` 会强制调用。同上 URL。
- **本地笔记已有**（`notes/opencode-goose.md` §8）：预算 `GOOSE_SUBAGENT_MAX_TURNS`（默认 25）、
  `GOOSE_MAX_BACKGROUND_TASKS`（默认 5）、5 分钟超时、并行时只有成功的子 agent 有结果。
- **今天上游复核的补充**：并发上限文档化为 `GOOSE_MAX_CONCURRENT_SUBAGENTS`（默认 5，**按父会话计**，
  多出来的排队等一个跑完）；子 agent 只能在 `auto` 权限模式下使用（`approve` / `smart_approve` /
  `chat` 下禁用）；子 agent **不能**再派子 agent、不能装卸 extension、不能动 schedule。
  <https://goose-docs.ai/docs/guides/context-engineering/subagents/>
- **⚠️ 修正一条本地笔记**：本地笔记记的 `orchestrator` 平台扩展（`list_sessions` / `view_session` /
  `start_agent` / `send_message` / `interrupt_agent`）**在今天的源码里已经不存在** ——
  `platform_extensions/mod.rs` 现在注册的扩展是 analyze / apps / chatrecall / code_execution /
  developer / ext_manager / scheduler / summon / summarize / todo / tom / skills，**没有 orchestrator**，
  旧的 `orchestrator.rs` 路径也返回 404。
  <https://raw.githubusercontent.com/aaif-goose/goose/main/crates/goose/src/agents/platform_extensions/mod.rs>
  （本地笔记对应的结论作废；**goose 曾经有过一套「起 agent / 给它发消息」的面板，后来收掉了** ——
  这条本身是「平级消息不一定留得住」的证据。）
- **形态 C 的一格：roaming agents** —— 基于 iroh（QUIC）的**认证过的 P2P ACP 传输**：
  `goose roam delegate <card> "<task>"` 是一次性委派；`goose roam bridge` 把远端 agent 暴露成本地
  ACP 端点。文档原话：每次委派是「a self-contained task with a bounded response, so this composes
  into multi-machine workflows **without any shared state**」。
  <https://goose-docs.ai/docs/guides/roaming-agents/>

### 1.8 Crush — 无多 agent 形态（但有「多客户端共享一个工作区」）

- **README（本轮抓取）列出的能力是：多模型、会话制、LSP、MCP（stdio/http/sse）、hooks（初步）、
  Agent Skills、桌面通知、指标、主题 —— 没有 subagent / 多 agent 能力。**
  <https://raw.githubusercontent.com/charmbracelet/crush/main/README.md>
- **有一处近似但不同**：多客户端共享后端时（两个 TUI 连同一个 `crush serve`），客户端按解析后的
  `--cwd` 分组进同一个 **workspace**，共享会话列表、消息历史、**权限队列**、LSP 与 MCP 状态。
  这是「多客户端看同一份状态」，不是「多个 agent 互相通信」。同上 URL。
- **形态判定**：无（README 层面的 verified absence；本轮未做全源码审计）。

### 1.9 Amp — 形态 A（专职 subagent），外加形态 C 的 orb / runner

- **专职 subagent：Search、Oracle、Librarian、Task、Read Thread。** 各自有独立上下文窗口与工具；
  **各自跑自己的模型，且模型选择独立于主 agent**（主 agent 在 `high` 档是 GPT-6 Astra，
  oracle 是 Claude Fable 5.1 —— 即「一个模型评审另一个模型的推理」）。
  <https://ampcode.com/docs/models-and-subagents>
- **隔离语义写得很死**：「They work in isolation, so they **can't communicate with each other**,
  you can't guide them mid-task, and they start with the instructions and context the main agent gives
  them rather than the full conversation. The main agent only receives their final summary.」
  同上 URL。**本地笔记已有**（`coding-agent-features.md` §8）。
- **`oracle` 工具**是一个「第二意见」模型，主 agent 可自主决定要不要问（文档明说不强制，
  因为更慢更贵）。<https://ampcode.com/docs/tools>
- **Orbs（形态 C）**：Amp agent 可以**启动别的 agent**、给它下指令、交换文件、把结果带回当前
  thread。新 thread 有自己的会话、上下文窗口、**working copy** 与 orb；
  「It does **not** share uncommitted files automatically」——文件要显式传。
  文档给了三种用法：side quest（另起一条线继续干当前活）、fan-out（并行起四条 low 档 thread）、
  跨项目/跨机器（另一个 Amp project、另一个 orb、或跑着 Amp **runner** 的另一台机器）。
  <https://ampcode.com/docs/orbs/agent-to-agent>
- **共享状态**：没有板。跨 agent 的「共同物」是**显式传递的文件**。

### 1.10 SWE-agent — 无多 agent 形态

- **README（本轮抓取）没有多 agent 能力**；项目本身已被 mini-swe-agent 取代，官方建议改用后者。
  其「batch mode」是 SWE-bench 批量评测运行器，不是 agent 编排。
  <https://raw.githubusercontent.com/SWE-agent/SWE-agent/main/README.md>
- **形态判定**：无。

---

## 2. 形态归纳表

| 形态 | 代表 | 原理 | 通信机制 | 共享状态落在哪 | 并发模型 | 权限与隔离 | 人怎么看见 | 代价 | 适合什么 |
|---|---|---|---|---|---|---|---|---|---|
| **A. 工具派生**（父 agent 用一个工具派生子 agent，摘要回来） | Gemini CLI subagents、OpenCode `subagent`、Cline `spawn_agent`、OpenHands `TaskToolSet`、Goose `delegate`、Amp 专职 subagent、**heng 的 Executor** | 父的工具调用创建一个子会话/子上下文；子跑完把最终报告作为 tool result 回到父的上下文 | **走父的上下文**（子结果以 tool result 形式回灌）；子与子之间**无通道** | 子 agent 的待办在**各自的上下文/会话**里（Gemini `write_todos` 明确「scoped to the current session」；opencode `todowrite` 默认对 subagent 关闭） | 父一轮内可发多个同型调用并行（Gemini 默认并发、OpenHands 默认 `tool_concurrency_limit=1`）；子之间**互不可见** | 权限多按「继承拒绝」实现：opencode 只继承父的 deny 与 `external_directory`；Gemini 用 `tools` 白名单 + 策略按 agent 名限定；Cline 批准 `spawn_agent` 即批准其一切；heng 继承拒绝与询问、不继承放行。**无 worktree 隔离** | 父的 TUI 里只看到「派了 N 个、各自什么结论」；opencode 能在父子 session 间导航；goose 把子 agent 的工具调用作为可展开段落显示 | 父看不到中间过程；子之间无法协调；派发质量全靠父的编排能力；写冲突要靠人或上层分配 | 一次性、有明确边界的子任务；上下文隔离（把大读取丢出去） |
| **B. 平级 agent + 共享板** | **Cline Agent Teams（唯一明确的一手先例）**；近似：Cline Kanban（共享看板 + worktree 隔离，但主要由人/侧边栏驱动） | 协调者把工作拆给成员；成员从**共享板**上 claim 任务、互相发消息、把结果分片贴回 | **不经过父上下文**：共享 mailbox / 广播；成员之间直接读板与信箱 | **一等持久化对象**：task board + mailbox + mission log，存在 `~/.cline/data/db/teams.db`，按 session id 归档，会话恢复时回来 | 成员真正并行；板上有 claim/complete/block 语义；Cline 另有 `team_run_task` 与 `team_await_runs` 把「派发」与「等结果」拆开，父不被阻塞 | 成员有独立的会话与上下文；只有协调者能关成员/清理；Kanban 侧每任务一个 **ephemeral git worktree** 消除写冲突 | 卡面显示最新一条消息或工具调用；详情页给完整 agent TUI + diff + 行内评论回传；CLI 里 `cline --id <session-id>` 恢复团队 | 要设计板的语义（claim / 冲突 / 生命周期）、持久化与恢复、消息投递与唤醒；写冲突要另解决 | 任务天然可拆、需要不同专长、且要**跨天/跨会话**恢复的大工程 |
| **C. 运行时 / 编排器调度**（每个 agent 独立进程 / 容器 / 远程服务，调度器决定谁做什么） | Cline hub-spoke + Kanban worktrees、OpenHands agent-server + workspace、Gemini CLI 的 A2A 远端子 agent、goose roaming（iroh QUIC 上的 ACP）、Amp orb / runner、opencode 的 server / SDK / ACP | 调度层持有会话与路由，agent 在隔离的运行时里执行；跨 agent 靠**协议**而不是共享状态 | **协议与流**：WebSocket 事件扇出（Cline hub）、REST + WebSocket（OpenHands agent-server）、A2A（Gemini 远端子 agent）、ACP（goose roam、opencode acp、cline acp）、MCP（goose extension、Gemini `mcpServers`）、显式文件传输（Amp orb） | **运行时状态**，不是任务板。OpenHands 的 event stream（单 conversation 内）、Cline 的 `sessions.db`、goose 的 `sessions.db`（`parent_session_id`）、goose 明说多机工作流「**without any shared state**」 | 真正的多进程 / 多机 / 多容器并行；调度器决定并发度 | 强：容器 / workspace / worktree / 独立 client；可跨机跨项目；A2A/ACP 有自己的认证（goose 用公钥互信、撤销数秒内生效） | 多端接入（CLI / IDE / Web / Slack）；事件流、看板、远端 TUI | 要进程/容器/网络基础设施、协议实现、认证与错误传播；调试链路长 | 跨机、跨项目、CI 批处理、需要进程级隔离与可恢复的长期作业 |
| **D. 同一上下文内的角色分工**（主模型 + 编辑模型 / 多角色轮流发言） | aider architect/editor、**heng 的讨论会话**、Cline plan/act 分模型、Amp 的 oracle（弱形式） | 同一段对话里换角色/换模型：一份上下文被两次（或多次）使用 | **就是父的上下文**——没有额外通道 | 与普通对话同一份上下文（aider 无 todo、无 subagent、无 fork） | 严格串行（aider architect→editor 是两次 LLM 请求） | **零隔离**：同一个模型、同一个上下文、同一份工作区 | 就是普通对话流 | 两次 LLM 请求（更慢更贵）；角色边界靠 prompt 维持 | 单模型既不擅长推理又不擅长编辑时；想用不同模型分工；上下文很小时 |

**关于形态 B 的明确结论**：本轮在九家上游文档与源码里找遍，**明确的「平级 agent + 共享板」一手先例只有
Cline 一家**。其余各家要么是形态 A（父-子，结果回灌），要么是形态 C（协议连接、显式无共享状态），
要么形态 D（同上下文）。唯一接近的第二个是 Cline Kanban —— 它有共享看板、依赖链与 worktree 隔离，
但**agent 之间不互发消息**，编排主要由侧边栏 chat 与人驱动。

---

## 3. 五个必答问题的横向答案

### 3.1 通信机制

| 路线 | 谁在用 | 载体 |
|---|---|---|
| 父的上下文回灌 | 形态 A 全部（Gemini、OpenCode、Cline `spawn_agent`、OpenHands TaskToolSet、goose `delegate`、Amp subagent） | 子结果作为 tool result 回到父 |
| 共享存储里的板与信箱 | **只有 Cline Agent Teams** | `~/.cline/data/db/teams.db` 的 task board + mailbox + mission log |
| 事件流 | OpenHands（event stream，单 conversation）；Cline hub（WebSocket 事件扇出到多个 client）；opencode（session 事件） | 追加日志 / WebSocket |
| 跨 agent 协议 | Gemini（A2A）、goose（ACP over iroh）、opencode / Cline（ACP）、goose / Gemini（MCP 扩展） | HTTP / QUIC / stdio JSON-RPC |
| 显式文件搬运 | Amp orb | 文件传输，不自动共享未提交改动 |

**通用观察**：**子 agent 之间几乎从不通消息。** 形态 A 的本质就是「子与子之间没有通道」——
Amp 明写「can't communicate with each other」，goose 明写「Delegates cannot coordinate」，
opencode 用 `subagent_depth` 默认 1 断掉递归，Cline 的 subagent 干脆不给 MCP 工具。
**唯一给了成员间通道的是 Cline 的 `team_send_message` / `team_broadcast` / `team_read_mailbox`。**

### 3.2 共享状态

| 落点 | 谁 | 事实 |
|---|---|---|
| 上下文 / 内存（单会话内的一份待办） | Gemini `write_todos`、OpenHands `TaskTrackerTool`、opencode `todowrite` | Gemini 官方写明「Persistence: Todo state is scoped to the current session」 |
| 事件流 / 会话库（父子关系靠外键表达） | OpenHands event log、goose `sessions.db` 的 `parent_session_id`、Cline `sessions.db` | 父子关系是**单向的**：能查到谁派了谁，但子不能给兄弟发消息 |
| 板 + 信箱（跨 agent 共享的一等对象） | **Cline Agent Teams** | task board + mailbox + mission log，按 session id 存在 `teams.db` |
| 文件 | Amp orb（显式传输）、Cline Kanban（worktree + 卡片） | 没有共享写 |
| **明确不共享** | goose roaming | 文档原文：多机工作流「without any shared state」 |

### 3.3 上下文：独立还是共享，产物怎么回灌

- **形态 A**：子 agent **独立上下文**（这是定义本身），产物 = **最终报告 / 摘要**，回灌进父的上下文。
  各家对「回灌多少」给了开关：goose 有 return mode（full / summary only）；
  Cline 让子 agent「returns its final reply to the main agent」；Amp 明写主 agent 只收到 final summary。
- **形态 B**：每个成员有自己的上下文与消息流，**共享的是板与信箱**；产物以「outcome fragment」的形式
  贴回（`team_attach_outcome_fragment` → `team_review_outcome_fragment` → `team_finalize_outcome`），
  由协调者合并。
- **形态 C**：每个 agent 独立进程/会话，跨 agent 的产物靠**协议回传**（A2A 的响应、ACP 的 prompt 往返、
  文件传输）。
- **形态 D**：**不隔离**，同一份上下文被角色复用。

### 3.4 可见性：人怎么看见团队

- **终端内联**：goose 把子 agent 的工具调用作为**可展开段落**显示在主会话里（工具名、参数、输出）；
  opencode 能在父子 session 之间用 `session_child_first` / `session_child_cycle` / `session_parent` 导航；
  Amp 只给主 agent 一句摘要。
- **看板**：Cline Kanban（卡面 = 最新一条消息或工具调用；详情页 = 完整 agent TUI + diff + 行内评论回传）。
- **多端 / 远端**：Cline hub（多个 client 挂同一 session，能力中介）；goose Desktop；
  opencode web / server API；goose roam 的 Web 客户端。
- **日志**：几乎每家都有一份只追加的事件流或会话文件（heng 已经是 JSONL + `outputs/`）。

### 3.5 权限与隔离

- **权限继承的共同规律：继承拒绝，不继承放行。**
  opencode：子只继承父的 deny 与 `external_directory`；
  heng：`ExecutorPort` 显式只取父策略的 mode、`outside_read` 与**可传播**的规则（`Allow` 不传播）；
  Gemini：靠 `tools` 白名单 + 策略里的 `subagent = "<name>"` 限定；
  Cline：批准 `spawn_agent` 等于一次性授权它全部行为（不做内部审批）。
- **子 agent 能不能写文件**：**能。** 这是形态 A 的常态 —— 没有任何一家默认让子 agent 只读
  （Gemini 的 `generalist` 明确继承父的工具集；OpenHands 的子 agent 有 `file_editor`；goose 的
  developer extension 给子 agent 同一套工具）。要只读得自己配（opencode 的 `explore`、
  Gemini 的 `tools: [read_file, grep_search]`）。
- **worktree 隔离**：**只有两种情况出现** —— Cline Kanban（每任务一个 ephemeral worktree）与
  opencode v2（`worktree_create` + `session_move`）。**形态 A 的普通 subagent 都不隔离。**
- **沙箱**：OpenHands V0 强制 Docker、V1 改为可选 workspace；Gemini CLI 有 seatbelt / container
  沙箱（且沙箱下 browser agent 被禁用）；Aider 无沙箱；Amp 默认**完全不询问**（本轮复核
  `ampcode.com/docs/tools` 仍如此）。
- **并发写的现实**：goose 给了最朴素也最可执行的规矩 —— **"no two delegates touch the same file"**；
  Cline 用 worktree 消除冲突；其余各家靠「别同时改同一个文件」这条不成文约定。

---

## 4. 对 heng 的启示

> 前提：heng 今天的 Executor 已经是形态 A 的标准实现 —— `task(brief)` 工具（`src/tools/task.rs`）
> 经 `ExecutorSpawner`（`src/tools/tool.rs`）派生出 `ExecutorPort`（`src/agent/executor.rs`），
> 执行者有自己的身份、自己的预算（`executor_max_iterations`）、自己的工具表（`Registry::for_executor`
> 去掉不可委派的 `task`，递归深度为一），跑完回一份报告；`task` 的 `Effect` 是 `ReadOnly`
> （派发不碰工作区），所以同一批多个 `task` 可以并行；权限上继承拒绝与询问、不继承放行。
> 这与 Gemini CLI / OpenCode / Cline / OpenHands 的形态 A **完全同构**。

**启示 1：光再加一种「派生方式」价值有限 —— 增量在「成员」这个身份。**
形态 A 的执行者**是一次工具调用**：调用结束，身份消失，只剩一份报告。它没有可寻址的名字、
不能被别人找到、不能被别人追问、不能活过父会话。形态 B 的成员是**持久的、可寻址的**：
Cline 的 teammate 有名字、有 mailbox、有板上的 claim 记录，父会话结束后团队还在
（`teams.db` 按 session id 归档，恢复会话时成员一起回来）。
**如果 heng 要做 agent teams，真正要新增的不是「再一个 spawn 工具」，而是「成员身份」**：
一个能跨会话存活、能被别人指名、能读写的会话内的实体。

**启示 2：通信通道的落点有三个候选，代价各不同 —— 对应 seed 里的分叉 2。**
- (a) 进收件方那一份 JSONL：可搬运、可重放、能进上下文（heng 的会话本来就是可搬运的目录）；
  但「板」放不进去 —— 板是**多写者**的，JSONL 是**单写者只追加**的，这是正面相撞的。
- (b) 另开一份团队级目录：板与信箱在一处，成员各持一份 JSONL 引用它。
  这正是 Cline 的做法（`teams.db` 与 `sessions.db` 分开，且用 session id 关联）。
- (c) 进程内总线 + 落盘快照：最省事，但跨进程/跨天就断了。
**证据支持 (b)**：本轮唯一活下来的形态 B 就是把团队状态放在**独立于会话的一个存储**里，
并用 session id 把它挂回会话。Cline 还额外做了一件事 —— 团队状态**随会话恢复**，
这正是 seed 里「团队要跨天」那条诉求的落点。

**启示 3：把「派出去」和「等结果」拆成两个动作，这是各家最值得抄的一个细节。**
Cline 的团队工具把 `team_run_task`（派）与 `team_await_runs`（等）分开；
而形态 A 的主流做法都是**父被阻塞**：goose 的 `delegate` 是同步的，
OpenHands 的 `TaskToolSet` 文档明写「the parent blocks until the sub-agent finishes」。
**拆分之后，父不必被阻塞，成员之间可以互相 await，消息投递也从「返回值」变成「事件」。**
这正好接住 seed 的分叉 3（收件方怎么被唤醒）：形状上它把「立刻打断 / 排到边界 / 只在空闲时投递」
这个选择从**投递机制**里剥出来，交给**调度点**决定 —— 打断仍是打断，只是现在可以先排队再打断。

**启示 4：共享板的第一版应该是「只读成员 + 写操作仍走 Executor」，而不是一上来做并发写。**
形态 B 的真实代价是并发写冲突，而各家的答案都很朴素：goose 要求「no two delegates touch the
same file」，Cline 用每任务一个 ephemeral git worktree。**heng 已经有路径锁**
（`src/tools/registry.rs` 的 `PathLocks`，执行者与父共用），
这解决了「同一文件不并发写」，但没解决「两个成员改同一份代码的不同部分、语义冲突」。
**建议的第一版切法**：团队成员只做**调研与提案**（共享板 + 共享上下文），
真正落盘仍由父用现有的 `task`/Executor 派发 —— 这样形态 B 带来的收益（协调、可见性、跨天恢复）
先落地，写冲突这条最难的路留到有实测需求时再走 worktree。

**启示 5：可见性照 Cline Kanban 的做法分层，不要并排渲染所有成员的完整流。**
卡片上只显示「最新一条消息或工具调用」，点开才给完整 TUI + diff。
heng 现在已有单会话的 TUI 与事件流（`src/render/`），多会话视图要新增的应该是
**一层摘要索引（每个成员一行：状态、最新动作、最后一句话）**，而不是 N 份完整流的并排。
seed 的分叉 5（团队关系画在哪一层）—— 各家的答案一致：**关系画在任务板/摘要层，不画在流里**。

**启示 6：编排纪律应该写进工具描述，而不是只写进系统提示。**
goose 的 `delegate` 工具描述里直接写着「Delegates cannot coordinate. Same-file work = conflicts.」
「partition files strictly - no two delegates touch the same file.」；
Cline 把纪律拆成工具（`team_task` 的 claim 语义、`team_review_outcome_fragment` 的复核）。
heng 现在把执行者纪律写在 `EXECUTOR_IDENTITY`（`src/agent/executor.rs`）里，
**但那段提示只有执行者自己看得到；父 agent 看不到**。做团队时，纪律要有一份**父侧可见**的副本
（放在 `task`/未来 `team_spawn` 的工具描述里），否则父会做出「让两个成员改同一个文件」这种事。

**启示 7：别把「共享状态」当成必需品。**
goose 在多机场景下明确选择「without any shared state」，Amp 选择「显式传文件」，
两者都能跑通多 agent 工作流。**共享板的收益是「跨天恢复 + 成员互查」，不是「干活本身」**。
如果 heng 的 agent teams 先按形态 A + 成员身份做（能指名、能追问、能跨会话存活），
共享板可以后置 —— 反过来先做板，很可能做出一个没人用的数据库。

---

## 5. 来源清单

**抓取日期一律为 2026-10-20。**

### Gemini CLI（`google-gemini/gemini-cli`）
- Subagents：<https://raw.githubusercontent.com/google-gemini/gemini-cli/main/docs/core/subagents.md>
  （官网对应 <https://geminicli.com/docs/core/subagents/>）
- Remote Subagents（A2A）：<https://raw.githubusercontent.com/google-gemini/gemini-cli/main/docs/core/remote-agents.md>
  （<https://geminicli.com/docs/core/remote-agents/>）
- Extensions 总览：<https://raw.githubusercontent.com/google-gemini/gemini-cli/main/docs/extensions/index.md>
- Extension reference（`gemini-extension.json` / `agents/` / `policies/`）：<https://raw.githubusercontent.com/google-gemini/gemini-cli/main/docs/extensions/reference.md>
- Todo 工具：<https://raw.githubusercontent.com/google-gemini/gemini-cli/main/docs/tools/todos.md>
- 任务规划教程：<https://raw.githubusercontent.com/google-gemini/gemini-cli/main/docs/cli/tutorials/task-planning.md>

### OpenCode（`anomalyco/opencode`）
- v1 Agents：<https://opencode.ai/docs/agents/>
- v2 Agents：<https://opencode.ai/v2/docs/agents/>
- v2 Tools（`subagent` / `execute` / worktree）：<https://opencode.ai/v2/docs/tools/>

### aider（`Aider-AI/aider`）
- Chat modes（architect / editor model）：<https://aider.chat/docs/usage/modes.html>
- architect/editor 设计说明：<https://aider.chat/2024/09/26/architect.html>

### OpenHands（`OpenHands/software-agent-sdk`、`OpenHands/OpenHands`）
- Task Tool Set：<https://docs.openhands.dev/sdk/guides/task-tool-set>
- Parallel Tool Execution：<https://docs.openhands.dev/sdk/guides/parallel-tool-execution>
- （V0 / V1 的其余事实来自本地笔记 `notes/aider-openhands.md`，今天以上游文档复核，未改笔记）

### Cline（`cline/cline`）
- Agent Teams（CLI）：<https://raw.githubusercontent.com/cline/cline/main/docs/cli/agent-teams.mdx>
- Multi-Agent Teams（SDK）：<https://raw.githubusercontent.com/cline/cline/main/docs/sdk/guides/multi-agent-teams.mdx>
- Subagents：<https://raw.githubusercontent.com/cline/cline/main/docs/features/subagents.mdx>
- Kanban Core Workflow：<https://raw.githubusercontent.com/cline/cline/main/docs/kanban/core-workflow.mdx>
- Hub-Spoke Architecture：<https://raw.githubusercontent.com/cline/cline/main/docs/sdk/architecture/hub-spoke.mdx>

### Continue（`continuedev/continue`）
- README（只读 / 停止维护）：<https://raw.githubusercontent.com/continuedev/continue/main/README.md>
- 文档首页（导航里无多 agent 页面）：<https://docs.continue.dev/>
- 线索未证实：PR #9128「feat: subagents」<https://github.com/continuedev/continue/pull/9128>

### Goose（`aaif-goose/goose`）
- Subagents 文档：<https://goose-docs.ai/docs/guides/context-engineering/subagents/>
- `summon` 扩展源码（`delegate` / `load`）：<https://raw.githubusercontent.com/aaif-goose/goose/main/crates/goose/src/agents/platform_extensions/summon.rs>
- 平台扩展注册表（**无 `orchestrator`**）：<https://raw.githubusercontent.com/aaif-goose/goose/main/crates/goose/src/agents/platform_extensions/mod.rs>
- Roaming Agents（P2P ACP）：<https://goose-docs.ai/docs/guides/roaming-agents/>

### Crush（`charmbracelet/crush`）
- README：<https://raw.githubusercontent.com/charmbracelet/crush/main/README.md>

### Amp（Sourcegraph，闭源，仅文档）
- Modes & Models（专职 subagent）：<https://ampcode.com/docs/models-and-subagents>
- Tools（oracle）：<https://ampcode.com/docs/tools>
- Orbs · Agent to Agent：<https://ampcode.com/docs/orbs/agent-to-agent>

### SWE-agent（`SWE-agent/SWE-agent`）
- README：<https://raw.githubusercontent.com/SWE-agent/SWE-agent/main/README.md>

### 本仓库本地材料（只读引用，未修改）
- `docs/research/coding-agent-features.md`
- `docs/research/notes/aider-openhands.md`
- `docs/research/notes/cline-continue.md`
- `docs/research/notes/opencode-goose.md`
- `docs/research/notes/shell-vs-first-class-tools.md`

---

## 附：本轮未能确认的事

- **Goose 的 `orchestrator` 扩展去哪了**：当前 `platform_extensions/mod.rs` 里没有它，旧路径 404。
  是删除、合并还是迁移到别处，本轮**无法从一手源判定**。只确认了「它现在不在那里」。
- **Continue 的 subagents（PR #9128）是否合入并发布**：**未证实**，本文不作为依据。
- **Crush 是否在源码里有未写进 README 的多 agent 能力**：本轮只读了 README，**未做全源码审计**，
  因此 Crush 那条是「README 层面未见」，不是「不存在」。
- **A2A / ACP 协议本身的细节**：本文只引用 Gemini CLI 与 goose 文档里对这两者的**用法**，
  未抓协议规范原文。
- **OpenCode v2 的 `subagent` 工具源码**：本轮只读了官方文档（`/v2/docs/tools/`、
  `/v2/docs/agents/`），未定位到 v2 的实现文件（`dev` 分支的旧路径已 404）。
