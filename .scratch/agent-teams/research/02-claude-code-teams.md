# Claude Code 的多 agent 协作：subagents、agent teams、Agent SDK 范式

- 调研日期：**2026-10-20**（本文所有 URL 均于该日实际抓取；官方文档站是 JS 渲染的，**在页面 URL 后加 `.md` 可拿到纯 markdown 正文**，本文的文档正文都用 `curl` 取全文）
- 范围：为 `heng` 的「agent teams」意向（[`.scratch/agent-teams/seed.md`](../seed.md)）做上游事实底稿：Claude Code 的 subagents、**agent teams**、Claude Agent SDK 的多 agent 范式，以及 hooks / skills / MCP / worktrees 这些「像团队但机制不同」的协作原语。
- 判据：**一手源优先**。官方文档（`code.claude.com/docs`）、Anthropic 官方 GitHub（`anthropics/claude-code`、`claude-agent-sdk-typescript`、`claude-agent-sdk-python`、`claude-agent-sdk-demos`）、Anthropic 官方博客与官方 cookbook 算一手；二手文章只作线索，不单独支撑结论。
- **本文只报告事实与来源，不给 heng 的方案**（方案属于后续 spec / 票）。
- 与本地笔记的关系：见 §0。

---

## §0 与本地笔记的关系

本地笔记 [`docs/research/notes/claude-code-amp.md`](../../../../docs/research/notes/claude-code-amp.md)（2026-09 抓取）已经覆盖的部分，本文**不重复展开**，只点明「本地笔记已有」并补上这一轮的新增 / 修正：

| 主题 | 本地笔记已有（章节） | 本轮新增 / 修正 |
| --- | --- | --- |
| 派发工具名为 `Agent`（旧名 `Task`）、父只看到子 agent 的最终结果 | §1.2（行 84–87） | agent teams 复用**同一个** `Agent` 工具：带 `name` 就变成 teammate，**报告不走返回值，走团队消息**（见 §1.2.2） |
| subagent 定义是 `.claude/agents/*.md` + frontmatter、递归扫描、identity 只来自 `name` | §1.8（行 395–410） | 补齐 **frontmatter 全字段表**（18 个字段）、`permissionMode` 继承规则、两层工具过滤、并发/深度上限（见 §1.1） |
| subagent 上下文隔离、「只有最终摘要回来」 | §1.8（行 411–412） | 补齐「启动时装载了什么 / 什么**不**装载」的完整清单，含 **sibling roster** 系统提醒（见 §1.1.4） |
| 并行手段：agent teams（含 split-pane teammate 跑成独立进程）+ agent view | §1.8（行 413–417） | **本轮重点**：agent teams 的完整形态与原理（见 §1.2、§2） |
| hooks 事件清单（含 `TeammateIdle`） | §1.9（行 421–428） | 补 `TeammateIdle` / `TaskCreated` / `TaskCompleted` 的**输入字段与裁决语义**（见 §1.4.1） |
| todo / task 工具、模型门槛 | §1.7（行 383–391） | 补：团队里 task 工具是**共享任务板**，没有 task 工具时改用消息协调；`CLAUDE_CODE_TASK_LIST_ID` 可跨会话共享（见 §1.2.4） |
| agent loop、并发工具调用规则 | §1.1 | 本轮未变，仅补一句：这套循环在 SDK 里是**同一个**（见 §1.3.1） |

⚠️ **本地笔记的两处需要修正**：

1. 笔记 §1.8 写「agent teams（含 split-pane teammates running as separate processes）」—— 上游现在有**两种显示模式**：默认 **in-process**（teammate 跑在主终端的同一个 Claude Code 进程内），split-pane 才是独立进程 / 独立 tmux 或 iTerm2 pane（<https://code.claude.com/docs/en/agent-teams>，2026-10-20 抓取）。
2. 笔记把 agent teams 当作并列的一种并行手段列出，但**没有说清它当前是实验特性、默认关闭**，也没有说清它**不适用于 Agent SDK / `-p` 非交互模式**。这两点是本轮的核心发现（见 §1.2.1）。

---

## §1 形态：实体、工具、配置、UI

### 1.1 Claude Code 的 subagents

#### 1.1.1 定义文件与 frontmatter

subagent 是 **Markdown 文件 + YAML frontmatter**，`---` 之间的 frontmatter 是配置，剩余 Markdown 正文是 system prompt。**只有 `name` 和 `description` 必填**。多词字段用 camelCase（`maxTurns`、`disallowedTools`），**拼错的字段被静默忽略、不报错**（<https://code.claude.com/docs/en/sub-agents>，2026-10-20 抓取）。

完整字段表（同一 URL）：

| 字段 | 必填 | 含义（要点） |
| --- | :- | --- |
| `name` | 是 | 唯一标识，≤256 字符；**不能含 `:`**（那是 plugin 作用域标识符的保留字符，如 `my-plugin:reviewer`）；文件名不必与它一致 |
| `description` | 是 | 什么时候该委派给它。**所有 subagent 的 description 合计超过 15,000 token 会在启动时告警** |
| `tools` | 否 | 逗号分隔字符串或 YAML 列表，如 `Read, Grep, Bash`；省略则继承「subagent 可用的全部工具」 |
| `disallowedTools` | 否 | 黑名单；**先应用 `disallowedTools`，再在剩余池里解 `tools`**；同时出现在两边的被移除 |
| `model` | 否 | `sonnet` / `opus` / `haiku` / `fable` / 完整模型 ID（如 `claude-opus-5-5`）/ `inherit` |
| `permissionMode` | 否 | `default`（= Manual）/ `acceptEdits` / `auto` / `dontAsk` / `bypassPermissions` / `plan` / `manual`（`default` 的别名） |
| `maxTurns` | 否 | agentic 回合上限；到达时输出被标成 **partial**，Claude 可 resume 继续 |
| `autoCompactWindow` | 否 | 把该 subagent 的自动压缩窗口压到 `100000`–`1000000` token（v2.1.296+） |
| `skills` | 否 | 启动时预载的 skill（**注入全文，不只是描述**）；未列出的 skill 仍可用 `Skill` 工具调用 |
| `mcpServers` | 否 | 该 subagent 可用的 MCP server（名字引用已配置的，或内联完整配置） |
| `hooks` | 否 | 只作用于该 subagent 的生命周期 hook |
| `memory` | 否 | `user` / `project` / `local`，开跨会话记忆 |
| `background` | 否 | `true` 则即使 Claude 说要等也留在后台 |
| `omitClaudeMd` | 否 | 启动时不装 user/project/local 的 CLAUDE.md（managed policy 文件仍装；v2.1.271+） |
| `effort` | 否 | 该 subagent 活跃时的 effort 档：`low`/`medium`/`high`/`xhigh`/`max` |
| `isolation` | 否 | `worktree`：在临时 git worktree 里跑，从**默认分支**（不是父会话 HEAD）分叉；无改动则自动清理 |
| `color` | 否 | 任务列表与转录里的显示色（`red`/`blue`/…/`cyan`） |
| `initialPrompt` | 否 | 该 agent 作为**主会话 agent** 跑时自动提交的第一条 user 回合 |
| `experimental` | 否 | 目前只有 `cacheTtl`（`5m`/`1h`），控制该 subagent 请求的 prompt cache TTL（v2.1.248+） |

**plugin 作用域的 subagent 例外**：`hooks`、`mcpServers`、`permissionMode` 三个字段在 plugin 提供的 subagent 里被**忽略**（安全原因）。

CLI 也能用 `--agents '<json>'` 传同一套定义（字段名 `prompt` 代替正文）；非交互模式还可以 `--agents ./agents.json` 传文件路径（v2.1.281+）。

#### 1.1.2 内建 subagent

`Explore`（只读、代码库探索；主会话跑 Fable 时它在订阅 / Console / 网关下走 `opus` 档，在 Bedrock / Vertex / Foundry / Claude Platform on AWS / apps gateway 上留在主会话模型）、`Plan`（plan mode 下的只读研究）、`general-purpose`（全部工具，多步复杂任务）、`claude`（catch-all）、`statusline-setup`、`claude-code-guide`。**Explore 与 Plan 不装载 CLAUDE.md，也不带 git status 快照**（省 token）；其余内建与自定义 subagent 都装载两者。

可以关掉：`CLAUDE_CODE_DISABLE_EXPLORE_PLAN_AGENTS=1`（v2.1.198+）、`CLAUDE_AGENT_SDK_DISABLE_BUILTIN_AGENTS=1`（非交互与 SDK）。

#### 1.1.3 谁能调用它们

**只有主 agent 能派发**，通过 `Agent` 工具（旧名 `Task`，仍作别名接受）。有三种显式调用方式（<https://code.claude.com/docs/en/sub-agents>）：

1. **自然语言**：在 prompt 里点名（"Use the test-runner subagent to fix failing tests"），Claude 自行决定要不要委派。
2. **@-mention**：`@"code-reviewer (agent)" look at the auth changes` —— 这**保证**该 subagent 跑，但「控制的是跑哪个 subagent，不是给它什么 prompt」。
3. **整场会话当它跑**：`claude --agent code-reviewer` 或 `settings.json` 的 `"agent": "code-reviewer"`。

自动委派是常态：Claude 读各 subagent 的 `description` 来决定何时委派。`description` 里写 "use proactively" 会提高主动委派率。

**子 agent 能不能再派发**：默认**能**，但**最多三层**（`CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH`，默认 3；v2.1.219+ 起的值）。到深度上限时 `Agent` 工具被从 subagent 的工具表里**扣掉**（fork 例外：工具还列着，但调用返回错误）。深度历史：v2.1.172–2.1.216 默认 5 层且不可调；v2.1.217–2.1.218 默认 1 层；v2.1.219 起默认 3 层。

#### 1.1.4 跑在什么上下文里

非 fork 的 subagent **上下文窗口全新**，看不到父对话历史、已调过的 skill、已读的文件。它启动时装载的是（同一 URL 的 “What loads at startup” 一节）：

- 自己的 system prompt（自定义 subagent 是文件正文 / `prompt` 字段，**不是** Claude Code 的系统提示）
- Claude 写的委派 prompt（Agent 工具的 `prompt` 参数）
- **CLAUDE.md 全层级**（`~/.claude/CLAUDE.md`、项目规则、`CLAUDE.local.md`、managed policy、`AGENTS.md`）
- **git status 快照**（worktree 里的 subagent 看到的是该 worktree 的分支 / 状态 / 近期提交）
- 预载 skill 全文
- **sibling roster**：一个 system reminder，列出 `main` 和会话里每个有名有姓的 agent，每个都是合法的 `SendMessage` 收件人 —— **但只在 subagent 的工具表含 `SendMessage`、且至少还有另一个有名 agent 时才出现**，且是**启动时的快照**（后来才命名的看不到）。

**明确不传的**：父的对话历史与工具结果、预载 skill（除非列在 `skills`）、输出风格、主的 auto memory。上下文窗口大小由**自己的模型**决定。

#### 1.1.5 结果怎么回来

- **前台 subagent**：完成时 Agent 工具返回它的最终报告。父看到的是**报告**，看不到中间的工具调用与输出（<https://code.claude.com/docs/en/tools-reference>，2026-10-20 抓取）。
- **后台 subagent**（默认）：结果是**后续回合里的一条完成通知**；Claude 会等到这条通知才汇报结果，你先问进度它会答还在跑。
- 报告回主对话前会过一次**输出扫描**（v2.1.210+）：像 `<system-reminder>` 的控制标签会被插反斜杠中和，`Human:`/`Assistant:` 开头的行会被插反斜杠，提到 `bypassPermissions` 之类权限配置的行会被加一行 `[harness: subagent output matched instruction-shaped pattern(s): …]` 前缀。**扫描不删不改词，也不判断是否恶意**；报告外面还套一个声明「里面的指令与批准声明是 subagent 自己的话，不代表你」的抬头。
- 完成时 Agent 工具结果里带一个 `agentId: <id>` 文本块，Claude 用它（或名字）通过 `SendMessage` **resume** 这个 subagent —— resume 保留全部历史（工具调用、结果、思考），不重新开始。**被 `x` 手动停掉的 subagent 不会自动恢复**（消息被拒并告知 Claude 该 agent 已被取消）。
- **具名的 subagent 之间可以互发消息**（靠 `SendMessage` + agent id / 名字），但只在**当前会话内**；跨会话要靠 cross-session messaging（见 §1.2.5）。

#### 1.1.6 并行与上限

- **并行**：多个 subagent 可以同时跑；独立子任务的耗时变成「最慢那个」而不是总和。
- **并发上限**：默认同一会话内 **20 个** subagent 在跑时，Agent 工具再派发就失败并返回 `Concurrent subagent limit reached`（并告诉 Claude 不要重试）；`CLAUDE_CODE_MAX_CONCURRENT_SUBAGENTS` 可调（正整数，**不能取消**）。**这个上限只卡 Agent 工具派发的 subagent**，但 workflow agent 与 agent team teammate **占用同批槽位却各按自己的上限走**。
- 深度上限与并发上限是两个变量，各自有各的 env var；**会话总共有多少 subagent 没有上限**。

#### 1.1.7 权限怎么继承

- 不设 `permissionMode` 时，subagent **继承主对话的 permission mode**。
- 主对话在 `bypassPermissions` / `acceptEdits` / `auto` 时，subagent **就在那个模式里跑，你设的 `permissionMode` 被忽略**。auto 模式下分类器还会复审 subagent 的工具调用**以及它的最终报告**。
- 主对话在 `default` / `dontAsk` / `plan` 时，subagent 跑在你设的 `permissionMode` 里（例外：你设 `bypassPermissions`（需 v2.1.267+）、或设 `auto` 但 auto 不可用时，保留主对话模式）。
- **派发本身不弹权限**；权限在 subagent 的每次工具调用上被检查。前台 subagent 的权限弹窗在主对话里原样出现；后台 subagent 的弹窗浮到主会话里，并**点名是哪个 subagent 在问**（Esc 可以只否掉那一次调用而不停掉 subagent）。
- 工具池有**两层过滤**：第一层从每个 subagent 手里扣掉 `Agent`（到深度上限时）、`AskUserQuestion`、`EndConversation`、`EnterPlanMode`、`ExitPlanMode`（除非 `permissionMode: plan`）、`ScheduleWakeup`、`WaitForMcpServers`、`Workflow`；第二层对**后台** subagent 收紧成一份白名单（`Read`、`Grep`、`Glob`、`LSP`、`Bash`、`PowerShell`、`Edit`、`Write`、`NotebookEdit`、`WebFetch`、`WebSearch`、`TodoWrite`、`Skill`、`ToolSearch`、`EnterWorktree`、`ExitWorktree`、`Monitor`、`TaskStop`、`SendMessage`、`Artifact`，外加 auto 模式下的 `SubagentHandback`）。**同一定义在前台与后台解出不同的工具集**。

### 1.2 Claude Code 的 agent teams（重点）

**结论先说：Claude Code 确实有 agent teams，而且是一个真实存在、有专门文档页的特性 —— 但它是实验特性，默认关闭，且只在交互式会话里生效。**

#### 1.2.1 名字、开启方式、适用范围

- 官方页面：<https://code.claude.com/docs/en/agent-teams>，标题 “Orchestrate teams of Claude Code sessions”。
- **开启方式只有一个**：环境变量 `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`，可写在 shell 环境或 `settings.json` 的 `env` 块里：

  ```json
  {
    "env": {
      "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS": "1"
    }
  }
  ```

  不设这个变量时：**会话启动时不建团队、不写团队目录、Claude 不派发也不提议 teammate**（同一 URL 的 Warning 段）。设成 `0` 可关掉（且**不需要重启**：Claude Code 每次派发 subagent 时重读这个变量）。
- **不设默认档位要求**：当前文档只要求这个 env var，**没有**说 Pro/Max/Team/Enterprise 哪一档才有。⚠️ 但 changelog v2.1.33 有一条 “Fixed warnings about agent teams not being available on your current plan”，说明**历史上存在过档位门槛**。这一轮**查不到**当前（2026-10-20）文档里的档位要求，见 §4。
- **只在交互式会话里生效**：非交互模式（`-p`）**包括 Agent SDK 会话**，Claude 不派发 teammate；即使开了 env var，Claude 命名的 subagent 也照旧当普通 subagent 跑（<https://code.claude.com/docs/en/agent-teams>，2026-10-20 抓取）。
- **版本史**（`anthropics/claude-code` 的 CHANGELOG.md，各 tag 原文，2026-10-20 抓取）：
  - **v2.1.32**：“Added research preview agent teams feature for multi-agent collaboration (token-intensive feature, requires setting `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`)”（tag `v2.1.45` 的 CHANGELOG 里该条目位于 `## 2.1.32` 段下）
  - **v2.1.33** 同段：“Added `TeammateIdle` and `TaskCompleted` hook events for multi-agent workflows”
  - **v2.1.178**：“Agent teams: removed the `TeamCreate` and `TeamDelete` tools. With `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1` set, every session now has one implicit team — spawn teammates directly with the Agent tool's `name` parameter, no setup step needed. The `team_name` parameter on the Agent tool is still accepted but ignored.”（tag `v2.1.183` 的 CHANGELOG，`## 2.1.178` 段）
  - 也就是说：**早期版本有显式的「建团队 / 删团队」工具，v2.1.178 起改为「每个会话隐含一个团队」**。`Agent` 工具的 `team_name` 参数在 SDK 类型定义里仍留着，标 `// Deprecated; ignored`（<https://code.claude.com/docs/en/agent-sdk/typescript>，2026-10-20 抓取）。
  - 后续版本持续修 bug：v2.1.234 “Fixed agent teams: a teammate that dies on an API error now reports ‘failed’ to the lead, and messaging a stuck teammate wakes it to retry immediately”、v2.1.234 “Fixed a crash loop in agent teams where a malformed teammate mailbox message caused repeated errors every second until the mailbox file was manually deleted”。

#### 1.2.2 成员怎么创建

**没有「创建团队」这一步了**（v2.1.178 起）。创建 teammate 的机制是：

> “To start a team, ask Claude for teammates. Claude launches a teammate when it calls the Agent tool with a `name` while agent teams are enabled, unless the call is a fork or passes `isolation` on the call itself. Claude Code doesn't ask you to confirm the launch.”

即：**主 agent 在 `Agent` 工具调用上带 `name` 参数 ⇒ 派出来的是 teammate 而不是 subagent**。同一 URL 明确说 “Claude Code doesn't ask you to confirm the launch”。

⚠️ **副作用**：开了 agent teams 之后，**普通委派也会变形** —— Claude 可以自己给 subagent 命名（为的是之后能 `SendMessage` 它），而「被命名的 subagent 就成了 teammate」。文档说 “teams can form even when you didn't ask for one”。

用户侧的控制手段全是自然语言：

```text
I'm designing a CLI tool that helps developers track TODO comments across
their codebase. Spawn three teammates to explore this from different angles:
one on UX, one on technical architecture, one playing devil's advocate.
```

以及 “Spawn 4 teammates to refactor these modules in parallel. Use Sonnet for each teammate.”（同一 URL）。

**teammate 可以复用 subagent 定义**：spawn 时说 “Spawn a teammate using the security-reviewer agent type to audit the auth module.”，Claude Code 会把该定义的这些部分套上去（同一 URL “Use subagent definitions for teammates”）：

- `tools`：限制 teammate 的工具。**in-process teammate 会被额外加上 `SendMessage`**；会话有 Task 工具时还加 `TaskCreate`/`TaskGet`/`TaskList`/`TaskUpdate`。
- `model`：两种显示模式都生效。
- `disallowedTools`：**只对 in-process teammate 生效**；加上的 `SendMessage` 与 Task 工具不受它影响。
- `effort`、`autoCompactWindow`：**只对 in-process teammate 生效**。
- **正文**：in-process 模式下**追加**到默认 system prompt 之后；split-pane 模式下**替换**默认 system prompt。
- `skills`：**两种模式都不套用**，teammate 从项目与用户设置里加载 skill。
- `mcpServers`：**只对 split-pane teammate 生效**；in-process 忽略它。

#### 1.2.3 团队由什么构成

官方架构表（同一 URL）：

| 组件 | 角色 |
| --- | --- |
| **Team lead** | 派发 teammate、协调工作、汇总结果的主 Claude Code 会话 |
| **Teammates** | 各自独立工作的独立 Claude Code 实例 |
| **Task list** | 共享的待办清单，teammate 认领并完成 |
| **Mailbox** | agent 之间的消息系统 |

**一个会话只有一个团队**（作用域限于该会话），不能建额外的具名团队，也不能跨会话共享团队；**lead 固定**（主会话终身是 lead，不能提拔或移交）；**不能嵌套团队**（teammate 不能派 teammate）。

#### 1.2.4 任务板在哪

**共享任务列表**，用 Task 工具操作（`TaskCreate`/`TaskGet`/`TaskList`/`TaskUpdate`）。三种状态：pending / in progress / completed。**任务可以依赖别的任务** —— 有未解依赖的 pending 任务不能被认领。

**存储位置**：`~/.claude/tasks/{team-name}/`，其中 `team-name` 是 `session-` + 会话 ID 前 8 位。这个目录**会话结束后仍留在盘上**，保留期由 `cleanupPeriodDays` 控制（默认 30 天）。

- **认领用文件锁**，防止多个 teammate 同时认领同一个任务。
- lead 可以显式指派；**也可以让 teammate 自领**：干完一个任务后自己挑下一个未指派、无阻塞的任务。
- **依赖自动解**：teammate 完成了被依赖的任务，依赖它的任务自动解锁，不需要人干预。
- **没有 Task 工具的 agent 改用消息协调**，不走共享任务板（同一 URL）。
- 跨会话共享一份任务列表：设 `CLAUDE_CODE_TASK_LIST_ID=<id>`，用 `~/.claude/tasks/` 下的具名目录（<https://code.claude.com/docs/en/interactive-mode>，2026-10-20 抓取）。

#### 1.2.5 消息怎么发

**唯一的发消息工具是 `SendMessage`**（<https://code.claude.com/docs/en/tools-reference>，2026-10-20 抓取）：可发给 agent team teammate、它 resume 的 subagent（按 agent ID 或名字）、或你机器上 / 更远的其它 Claude Code 会话。可选 `summary` 输入（一般 5–10 个词）作为一行预览；不给时用消息首行当 summary，超过 200 字符截断。

四种通信方式（agent-teams 页 “Context and communication”）：

1. **自动投递**：teammate 发出的消息自动送到收件人，**lead 不需要轮询**。
2. **空闲通知**：teammate 干完停下时**自动通知 lead，并把最终答案带在通知里**；若是 API 错误收尾，则通知 lead 失败并附错误文本。
3. **共享任务列表**：有 Task 工具的 agent 能看见任务状态并认领工作。
4. **点名消息**：按名字给某一个 teammate 发；**要发给所有人就得一人一条**。

**信箱在磁盘上**：每个 agent 的 mailbox 是一个 JSON 文件，路径 `~/.claude/teams/{team-name}/inboxes/{agent-name}.json`。Claude Code 读信箱时**逐条校验**，不合消息格式的条目**报为错误并从文件里删掉**，合规的消息照送。

**投递语义很硬**（同一 URL）：

> “Claude Code reports a message as sent only when the write to the recipient's mailbox file succeeds, whether the message is plain text or a structured protocol message such as a plan approval or shutdown request. When the write fails, for example because the disk is full or the mailbox directory isn't writable, the sending agent receives an error and nothing was sent.”

（v2.1.224 之前，写失败也报“已发送”。）

**团队协议消息**是结构化的，与普通文本消息不同。已知种类（从错误消息文档与 sub-agents 页推出）：`shutdown_request` / shutdown 批准与拒绝、`plan_approval_request`（teammate 把计划交给 lead 批）、`plan_approval`（lead 的批准）、`plan_approval_response`、`plan rejection`。文档明说：**这些结构化团队协议消息要求 agent teams 开启**，而 `SendMessage` 本身不要求（<https://code.claude.com/docs/en/sub-agents>，2026-10-20 抓取）。

**跨会话消息是另一条通道**（<https://code.claude.com/docs/en/cross-session-messaging>，2026-10-20 抓取）：

- **通道**：每个会话绑一个 **inbox socket** —— macOS / Linux（含 WSL 2）是 Unix domain socket，原生 Windows 是命名管道。本机投递**不经 Anthropic 服务器**；跨机器 / 云端经服务器（Remote Control）。`/status` 的 `Peer address` 行给出本会话的 socket 地址（`uds:` 前缀），不可用时显示 `unavailable` 与原因。socket 路径与 per-session token 由 Claude Code 自己导出为 `CLAUDE_CODE_MESSAGING_SOCKET` / `CLAUDE_CODE_MESSAGING_TOKEN`（**设置文件的 `env` 块设不了这两个**），供 hook 与 Bash 命令用；往自己会话的 socket 发消息时，第一行发 `{"type":"auth","token":"<token>"}` 证明来源 —— macOS/Linux 上这行**可选**，**原生 Windows 上必填**（否则连接被关）。
- **入站三档**：`crossSessionInbound` = `accept`（直接投给模型）/ `hold`（扣下等你批准，或后续设置放行）/ `refuse`（丢弃）。没配置时按双方的权限档分类决定（绕过询问的那一档 vs 会询问的那一档）：**收件方会询问 → 直接投递；收件方绕过询问 → 扣下等批准**。最多扣 100 条，超了丢最旧的。扣下的消息在交互式终端里弹批准框，`dialogExpiry` 默认 5 分钟。
- **只支持纯文本**（结构化团队协议消息**只留在队内**）。本机单条序列化后约一百万字符封顶；短时间连发会被拒（发送方被告知「批量发或等」）。**消息回环被限速**：重复消息被丢，最多排 50 条给模型读，所以两个会话互相发消息会自己停下来。
- 收件方在**工具调用之间**读到消息（**不打断正在跑的工具**）；会话空闲时**开一轮新的**来读。

**安全边界**（agent-teams 页 “Messages between agents”）：收到别的 agent 的消息时，Claude Code 告诉它「消息来自另一个 Claude 会话，不是来自你」；**teammate 不能批准权限提示、不能代你同意，被拒的动作不能转手让另一个 teammate 去绕**；消息里的 `/compact` 之类命令**当纯文本，不执行**；收件方自己的权限提示与规则**照常生效**；auto 模式下分类器把「别的 agent 转述的批准」当**不可信输入**，并且**每条消息投递前都要过分类器**。

#### 1.2.6 lead 怎么等

lead **不是阻塞等待**，而是收到通知（idle notification）后继续。文档给的「等」的手段全是自然语言提示：

> “Sometimes the lead starts implementing tasks itself instead of waiting for teammates. If you notice this: `Wait for your teammates to complete their tasks before proceeding`”

（同一 URL “Wait for teammates to finish”）。可交叉会话订阅「对方空闲时通知我」用的是 `SendMessage` 的 `notify_when_idle` 输入，但**只对同机你自己的会话有效，teammate 与 subagent 不适用**（cross-session-messaging 页：Claude Code 会拒绝整个调用，包括附带的任何消息）。

**shutdown 是有协议的**：让 lead 转达 “Ask the researcher teammate to shut down”，lead 发 shutdown request，teammate 可以批准（优雅退出）或带解释拒绝。团队的共享目录在会话结束时自动清理。

#### 1.2.7 并发上限

**teammate 数量没有硬上限**（同一 URL “Choose an appropriate team size”：“There's no hard limit on the number of teammates”）。实际约束是三条软约束：token 成本随人数**线性增长**、协调开销随人数上升、收益递减。官方建议**多数工作流从 3–5 个 teammate 起步**；3 个聚焦的 teammate 常常胜过 5 个散的。

⚠️ teammate **不占** subagent 的 20 并发槽位（sub-agents 页：“The limit blocks only subagents Claude spawns with the Agent tool, but other runs occupy the same slots… Agents that other features run, such as workflow agents and agent team teammates, follow their own limits instead.”）。

**in-process teammate 不能有后台 subagent**：teammate 自己的 subagent 只能前台跑（因为 teammate 的后台工作活不过 lead 的进程）；定义里设 `background: true` 会报错，`run_in_background: true` 也会失败（要么报错，要么静默前台跑）。

#### 1.2.8 UI 上怎么显示团队

**两种显示模式**（`teammateMode` 设置 + `--teammate-mode` flag；<https://code.claude.com/docs/en/settings-reference>，2026-10-20 抓取）：

| 取值 | 含义 |
| --- | --- |
| `"in-process"`（**默认**） | teammate 跑在主终端同一个 Claude Code 进程内，任何终端都行 |
| `"auto"` | 在 tmux 里、或 iTerm2 且 `it2` 在 PATH 时用 split pane，否则退回 in-process |
| `"tmux"` | 强制 split pane，用 tmux 或 iTerm2（按终端自动判断） |
| `"iterm2"` | 强制 iTerm2 原生 split pane，需要 `it2` CLI |

`--teammate-mode` 是实验 flag，不出现在 `claude --help` 里。split pane 模式**不支持** VS Code 集成终端、Windows Terminal、Ghostty。

**agent panel**（提示框下方的面板）：

- 上下键选 teammate，**Enter 打开该 teammate 的转录并直接对它说话**，Esc 取消选择（正在看某个 teammate 的转录时，Esc 打断它当前回合）。
- 空闲行在**整个面板都空闲后 30 秒**隐藏，下次该 teammate 回合时重新出现；**teammate 仍然在跑、仍然可寻址**。
- 空闲 teammate 超过 3 个时，多出来的行折叠成一行计数（如 `2 idle agents`），Enter 展开、Esc 折叠。在工作的、失败的、以及你正在看的 teammate 永远保留自己的行。
- 面板里 subagent 与 teammate **混在一起显示**，所以「面板里有行」不能证明组成了团队（agent-teams 页明确说了这点）。
- 在 in-process teammate 视图里，纯文本与 skill 发给**该 teammate**；内建命令发给 **lead**。`/compact`、`/clear`、`/rewind` 作用于 lead 的对话（会先要求确认）；`/model` 与 `/fast` 改 lead 的，不在 teammate 视图里生效。teammate 的模型与 fast mode **在 spawn 时固定**。

**成本**：teammate 各自占一个上下文窗口，token 用量随活跃 teammate 数与运行时长增长；官方估「teammate 跑在 plan mode 时大约是普通会话的 7 倍 token」（<https://code.claude.com/docs/en/costs>，2026-10-20 抓取）。in-process teammate 的请求落在主对话的 cache TTL 桶之外，cache 默认 5 分钟（`subagentPromptCacheTtl` 可改 1h）。

#### 1.2.9 官方自陈的九条限制

（同一 URL “Limitations”，**是九条，不止七条**）：

1. **in-process teammate 不可随 `/resume` / `/rewind` 恢复** —— 恢复后 lead 可能去喊已经不存在的成员。
2. **任务状态会滞后** —— 成员有时不把任务标完成，挡住依赖它的任务；官方让用户手动改或让 lead 去催。
3. **关闭慢** —— 成员要把当前请求或工具调用做完才退。
4. **一会话一队** —— 不能另建命名队、不能跨会话共享队。
5. **不能嵌套** —— teammate 不能再生 teammate，只有 lead 管队。
6. **in-process teammate 不能跑后台 subagent** —— 后台活不过 lead 的进程。
7. **lead 固定** —— 不能把 teammate 提为 lead、不能移交。
8. **权限在 spawn 时定下** —— 之后能改单个 teammate 的权限档，但**不能在 spawn 时逐个设**。
9. **split pane 要 tmux 或 iTerm2** —— 默认 in-process 在任何终端都能用。

### 1.3 Claude Agent SDK 里的多 agent 范式

#### 1.3.1 SDK 是什么、能做什么

Agent SDK 是把 Claude Code 的 agent loop 做成库（TS + Python）。**它 bundle 了一个原生 Claude Code 二进制**，所以 SDK 里的循环**就是** Claude Code 的循环：收到 prompt → 模型决定 → 执行工具 → 结果回灌，直到没有工具调用为止（<https://code.claude.com/docs/en/agent-sdk/agent-loop>，2026-10-20 抓取）。本仓库的本地笔记 §1.1 记的 turn 定义、停机条件、并发工具调用规则，在 SDK 里原样成立。

SDK 暴露的能力里与多 agent 相关的是：**Subagents**（派专职 agent）、**Workflow**（脚本编排大量 agent）、**Hooks**、**MCP**、**Permissions**、**Sessions**（含 resume / fork）。

⚠️ **Agent SDK 里没有 agent teams**：teammate 依赖交互式会话，`-p` 与 SDK 会话都不派发 teammate（agent-teams 页）。SDK 类型里能看到的团队痕迹只是**被动类型**：`origin: { kind: "coordinator" }`（团队协调者的消息）、`origin: { kind: "peer" }`（teammate 或跨会话 peer，含 `from` / `name` / `body` / `fromSession` / `verifiedPeerPid` / `senderTaskId` 等字段）、`TeammateIdleHookInput`、`ExitPlanMode` 结果里的 `awaitingLeaderApproval`（teammate 把计划交给 lead 批）。**没有 `TeamConfig`、没有建团队 API、没有 `SendMessage` 工具定义**。

#### 1.3.2 SDK 里的多 agent 范式（官方文档口径）

| 范式 | 结构（谁调谁） | 上下文怎么切 | 结果怎么聚合 |
| --- | --- | --- | --- |
| **orchestrator-workers** | 一个中心 LLM 动态拆任务、委派给 worker、汇总 | 每个 worker 独立上下文 | worker 的报告回到 orchestrator，由它合成 |
| **parallelization** | 独立子任务并发 | 每个并行分支独立上下文 | 输出**程序化**聚合；两种变体：**sectioning**（拆成独立子任务）与 **voting**（同一任务跑多次取多数） |
| **evaluator-optimizer** | 一个调用生成、另一个调用评估反馈，成环 | 两个调用各自的上下文 | 评估者的反馈回到生成者，循环 |
| **advisor**（≈「agents-as-tools」） | 顾问模型**在主 agent 工作时**被咨询 | 顾问有自己的调用，但工作仍由主 agent 做 | 顾问的判断回主 agent；只有主线程能咨询，且按顾问模型价计费 |

前三者来自 Anthropic 工程博客《Building effective agents》（2024-12-19，<https://www.anthropic.com/engineering/building-effective-agents>，2026-10-20 抓取），原文的 “Workflow: Prompt chaining / Routing / Parallelization / Orchestrator-workers / Evaluator-optimizer” 五个标题可逐字查到。原文对 orchestrator-workers 的界定：“a central LLM dynamically breaks down tasks, delegates them to worker LLMs, and synthesizes their results”，并指出它与 parallelization 的关键差别是**子任务不是预先定义的，而是 orchestrator 依输入当场定的**。该文还给出可运行的 parallelization / routing 实现（<https://platform.claude.com/cookbook/patterns-agents-basic-workflows>，2026-10-20 抓取）。

第四种（advisor）来自 Managed Agents 官方文档《Multiagent orchestration》（<https://platform.claude.com/docs/en/managed-agents/multiagent-orchestration>，2026-10-20 抓取），那张对照表的三个是：**Subagents**（agent 自己委派、读每个 subagent 的报告）、**Dynamic workflows**（agent 写一个程序编排 agent，后台跑成一个 workflow run）、**Give the session an advisor**。该文档明说：

> “With subagents, the agent itself determines what happens next. A subagent's thread stays available until you archive it, so the agent can send it follow-up messages. With dynamic workflows, Claude writes a program to orchestrate agents without Claude's direct involvement. Context and results are passed programmatically from one agent to another, freeing up the main session thread… The agent can't send follow-up messages to a run's threads, and the server archives each one by the end of its run.”

并给出一条**重要的量级限制**：“Delegation is one level deep, and a session can have at most **25 child threads** at a time, idle ones included. Advisor threads and a workflow run's threads don't count.” 同一文档还给出 “`{"type": "self"}`” 这种「把自己当 subagent 用」的配置，以及 `subagents.predefined_agents`（点名册）与 `subagents.inline_agents`（要不要允许 agent 自造 subagent）两条开关。

⚠️ **“agents-as-tools” 这个名字在上游查不到**（见 §4）；最接近的是上表的 advisor，以及官方 `skills` 仓库里那份 `shared/managed-agents-multiagent.md` 的建议：“**start with `self`, then add cheaper workers**”（<https://raw.githubusercontent.com/anthropics/skills/main/skills/claude-api/shared/managed-agents-multiagent.md>，2026-10-20 抓取）。

#### 1.3.3 官方示例代码的形状

**A. SDK 里的 subagent（Python，官方 `examples/agents.py` 原文，2026-10-20 抓取）**

```python
options = ClaudeAgentOptions(
    agents={
        "code-reviewer": AgentDefinition(
            description="Reviews code for best practices and potential issues",
            prompt="You are a code reviewer. Analyze code for bugs, performance issues, "
            "security vulnerabilities, and adherence to best practices. "
            "Provide constructive feedback.",
            tools=["Read", "Grep"],
            model="sonnet",
        ),
    },
)
```

`AgentDefinition` 的字段表（官方 SDK 参考，2026-10-20 抓取）：

- **TypeScript**（<https://code.claude.com/docs/en/agent-sdk/typescript>）：`description`、`tools?`、`disallowedTools?`、`prompt`、`model?`、`mcpServers?`、`skills?`、`initialPrompt?`、`maxTurns?`、`autoCompactWindow?`、`background?`、`omitClaudeMd?`、`memory?`、`effort?`、`permissionMode?`、`criticalSystemReminder_EXPERIMENTAL?`。
- **Python**（<https://code.claude.com/docs/en/agent-sdk/python>）：`description`、`prompt`、`tools?`、`disallowedTools?`、`model?`、`skills?`、`memory?`、`mcpServers?`、`initialPrompt?`、`maxTurns?`、`background?`、`effort?`、`permissionMode?`（**没有** `autoCompactWindow`、`omitClaudeMd`）。Python 的 `AgentDefinition` **字段名保持 camelCase**（`disallowedTools`、`mcpServers`、`permissionMode`、`maxTurns`），因为它们直接对应 wire 格式；用 snake_case 关键字构造会抛 `TypeError`。

**B. 「orchestrator-workers」的官方实现形状**：官方 demo `claude-agent-sdk-demos/research-agent`（2026-10-20 抓取）是一个 lead + 三个专职 subagent（`researcher` / `data-analyst` / `report-writer`）的编排器：`ClaudeAgentOptions(agents={...}, hooks={PreToolUse/PostToolUse: [HookMatcher(...)]}, allowed_tools=["Task"])`，lead 的 system prompt 从 `prompts/lead_agent.txt` 读，subagent 的从 `prompts/researcher.txt` 等读；`SubagentTracker` 用 **hooks + 消息流**两路追踪 subagent 的工具调用（<https://raw.githubusercontent.com/anthropics/claude-agent-sdk-demos/main/research-agent/research_agent/utils/subagent_tracker.py>）。这个 demo 的**产物走文件系统**：`files/research_notes/`、`files/charts/`、`files/data/`、`files/reports/` —— subagent 写盘，别的 agent 读盘，而不是把全文塞回对话。

**C. 「fixed N-agent team + 对等消息」的官方实现形状**：Claude Cookbook 的《Async multi-agent orchestration》（<https://platform.claude.com/cookbook/patterns-agents-async-multi-agent-orchestration>，2026-10-20 抓取）给出两个模式的**裸机制**：
- **message hub**：每个 agent 一个 inbox list + 一个 `asyncio.Event`；`post(sender, recipients, content)` 投递并 `event[rid].set()`；`drain(name)` 取走并清空；`render(msgs)` 格式化成 `<agent-message from="...">` 文本。
- **两个工具**：`send_message`（`recipient_ids` + `content`；描述里明说 “This is the ONLY way to reach other agents — plain text in your turn goes nowhere.”）与 `wait_for_message`（阻塞到有人发消息；描述里说消息也会自动附在任何其他工具调用的结果上，所以只在无事可做时用）。
- **关键机制**：`run_agent` 在每次工具调用后 `hub.drain(name)`，把收到的消息**追加到最后一条 tool result 的内容里** —— “so agents never poll; messages arrive inline”。这是**消息投递的机制层**，比 Claude Code 的 mailbox 文件更轻。
- 第二个模式是**动态派生 subagent**：lead 拿到 spawn / status / collect / kill 四件套，spawn 立即返回，lead 用 `get_status` 查进度、用 `wait_for_message` 收报告、最后 `kill_subagents` 收场。

**D. Managed Agents 的「协调专家团队」**（Cookbook，2026-10-20 抓取）：coordinator 跑三个各有工具集的专家（web-search researcher / file-reading librarian / rules-based pricer），外加一个 advisor；子 agent 用 `send_to_parent` 形状回报；事件流里有 `session.thread_created`（`agent_name`）、`agent.thread_message_received`（`from_agent_name`）、`session.status_idle` 三种。

**E. 动态 workflow 的脚本形状**（<https://code.claude.com/docs/en/workflows>，2026-10-20 抓取）：

```javascript
export const meta = {
  name: 'audit-routes',
  description: 'Audit every route handler for missing auth checks',
}

const found = await agent('List every .ts file under src/routes/.', {
  schema: { type: 'object', required: ['files'], properties: { files: { type: 'array', items: { type: 'string' } } } },
})

const audits = await pipeline(found.files, file =>
  agent(`Audit ${file} for missing authentication checks.`, { label: file }),
)

return audits.filter(Boolean)
```

运行时原语：`agent()`（起一个 subagent）、`pipeline()`（对列表逐项并发）、`parallel()`（一组同时跑并等全部）、`phase()`（分组）、`log()`。**中间结果留在脚本变量里，不进 Claude 的上下文**；每次 run 把脚本写到 `~/.claude/projects/` 下的会话目录里，Claude 拿到路径。`Workflow` 工具在 TypeScript Agent SDK v0.3.149+ 可用。

#### 1.3.4 SDK 侧的三条上限

（<https://code.claude.com/docs/en/agent-sdk/subagents>，2026-10-20 抓取）深度 `CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH`（默认 3）、并发 `CLAUDE_CODE_MAX_CONCURRENT_SUBAGENTS`（默认 20）、花费 `maxBudgetUsd` / `max_budget_usd`（**含 subagent 的花费**；到顶时拒派新 subagent、停掉还在跑的后台 subagent、以 `error_max_budget_usd` 结束）。**两个 SDK 对 `env` 的处理不同**：TS **替换**子进程环境（要 `...process.env` 保留 `PATH`），Python **合并**进继承环境。

#### 1.3.5 SDK 里的 resume 形状（官方给的完整代码）

（同一 URL “Resume subagents”）SDK 侧 resume 一个 subagent 要三步：**抓 `session_id`** → **从 Agent 工具结果里正则抓 `agentId: <id>`** → **第二次 `query()` 带 `resume: sessionId` 并在 prompt 里带上 agent id**。官方示例（Python）：

```python
def extract_agent_id(block: ToolResultBlock) -> str | None:
    parts = block.content if isinstance(block.content, list) else [{"text": block.content}]
    for part in parts:
        if match := re.search(r"agentId:\s*([\w-]+)", part.get("text") or ""):
            return match.group(1)
    return None
```

注意：**`Explore` 与 `Plan` 是 one-shot，不返回 agentId**，所以要 resume 就得用 `general-purpose` 或自定义 subagent。

### 1.4 Claude Code 的其它协作原语（像团队但机制不同）

#### 1.4.1 hooks

hooks 是「在生命周期某点自动执行的 shell 命令 / HTTP 端点 / MCP 工具调用 / LLM prompt / **subagent**」。跟团队相关的三个事件（<https://code.claude.com/docs/en/hooks>，2026-10-20 抓取）：

| 事件 | 触发 | 额外输入字段 | 裁决 |
| --- | --- | --- | --- |
| `TeammateIdle` | teammate 干完要进入空闲时 | `teammate_name`、`team_name` | exit 2 ⇒ 把 stderr 当反馈回给它、让它继续干；`{"continue": false, "stopReason": "…"}` ⇒ 彻底停掉它 |
| `TaskCreated` | 任务被创建时 | `task_id`、`task_subject`、可选 `task_description` / `teammate_name` / `team_name` | exit 2 或 `decision: "block"` ⇒ 取消这次创建并把消息回给 Claude |
| `TaskCompleted` | 任务被标记完成时（**包括 teammate 回合结束时还有 in-progress 任务**） | 同上 | exit 2 ⇒ 阻止完成；`{"continue": false}` 在「队友回合结束触发」时会停掉 teammate，但**在 `TaskUpdate` 工具触发时被忽略** |

这三个是**质量门**：teammate 交活前必须过 `TeammateIdle`，任务必须过 `TaskCompleted` 才能关。⚠️ `team_name` 在 SDK 类型里已标 `@deprecated since v2.1.178`（团队名不再有意义，因为只剩一个隐含团队）。

`TeammateIdle` 也有 **mod 钩子**对应物：`agent.spawn` 事件（teammate 即将启动时 `e.isTeammate` 为 true，可 `next({...e, model})` 改模型或 `{deny: reason}` 拒绝）。

#### 1.4.2 skills

skill 是「按需加载的指令包」，通过 `Skill` 工具执行，**不是**独立的 agent 实体。subagent 可以预载 skill（`skills` 字段注入全文）。**team 场景下 `skills` 字段不套用到 teammate**（teammate 从项目与用户设置自行加载）。

#### 1.4.3 MCP server 作为「外部 agent」参与

**上游没有「把 MCP server 当团队成员」的机制**。官方口径是：“In every approach the workers are Claude sessions. To involve a different tool, expose it to Claude as an MCP server.”（<https://code.claude.com/docs/en/agents>，2026-10-20 抓取）。也就是说 MCP 是**工具来源**，不是 agent。**“外部 agent 通过 MCP 加入团队”这一形态在上游查不到**（见 §4）。

反方向倒是有一个：**channels** —— 一个 MCP server 把外部事件（CI 结果、聊天消息）**推进**运行中的 Claude Code 会话，双向（Claude 可以回话）。研究预览、只支持 claude.ai 或 Console API key、不支持 Bedrock / Vertex / Foundry，Team / Enterprise 组织要显式开启（<https://code.claude.com/docs/en/channels>，2026-10-20 抓取）。

#### 1.4.4 worktrees 隔离

**worktree 是文件级隔离，不是通信机制**。`claude --worktree <name>` 在 `.claude/worktrees/<name>/` 建独立检出（分支 `worktree-<name>`）；`EnterWorktree` 工具让 Claude 自己在会话里建；subagent 可用 `isolation: worktree` 前缀声明。**agent view 派发的后台会话在改文件前会先移进自己的 worktree**（`worktree.bgIsolation: "none"` 可关）。官方明确它与团队是**两件事**：“Worktrees give each session a separate git checkout… Use them for sessions you run yourself.”（<https://code.claude.com/docs/en/agents>）。

#### 1.4.5 官方「五种并行方式」总表

（<https://code.claude.com/docs/en/agents>，2026-10-20 抓取）

| 方式 | 给你的东西 | 什么时候用 |
| --- | --- | --- |
| Subagents | 一个会话内的委派 worker，各自独立上下文，**回一份摘要** | 副任务会把主对话灌满搜索结果 / 日志 / 文件内容 |
| Agent view | 一个屏幕派发与监控后台会话（`claude agents`），**研究预览** | 你有若干独立任务，想交出去、瞥一眼状态、只在需要时插手 |
| **Agent teams** | 多个协调的会话，**共享任务列表 + agent 间消息**，由 lead 管；**实验特性、默认关闭** | 你要 Claude 把项目拆开、分派、并让 worker 之间保持同步 |
| Projects | claude.ai/code 或桌面端的一段长对话，Claude 起并行的 **thread**（云端或经 Remote Control 跑在你机器上）；Pro / Max 公开 beta | 活儿跨天跨周、你更愿意描述一次而不是派发与盯梢 |
| Dynamic workflows | 跑大量 subagent 并交叉验证结果的**脚本** | 一个活儿超出「几个 subagent」的量级，或你要结果被交叉核对 |

同页还点名三样「支撑但不是并行方式」的东西：**worktrees**（文件隔离）、**cross-session messaging**（你自己跑的会话之间传话）、**`/batch`**（一个 skill，把一次大改拆成 5–30 个 worktree 隔离的 subagent）。

### 1.5 形态对照表（速查）

| 维度 | subagent | agent team teammate | SDK subagent | workflow agent | Managed Agents subagent |
| --- | --- | --- | --- | --- | --- |
| 谁是实体 | 同一进程内的一个上下文 | 独立 Claude Code 会话（in-process 则是同进程的独立上下文） | 同 subagent | 脚本起的 subagent | server 侧的 session thread |
| 谁调它 | 主 agent 的 `Agent` 工具 | 同上（带 `name`） | 同上 | 脚本的 `agent()` | 服务端按 `multiagent` 配置 |
| 上下文 | 全新（fork 除外） | 全新，**不继承 lead 的对话历史** | 全新 | 全新 | 独立 thread，**线程持久**（可追问） |
| 结果回传 | Agent 工具返回值 / 后台完成通知 | **团队消息**（不是返回值） | 同 subagent | 脚本变量（**不进 Claude 上下文**） | 程序化传递 |
| 能互发消息 | 不能（除 resume / 具名互发） | **能**（`SendMessage`，peer-to-peer） | 能（`SendMessage` resume） | 脚本自己传 | `send_to_parent` / 事件流 |
| 上限 | 深度 3、并发 20 | 无硬上限 | 同左 | 数十到数百 agent/run | 25 个 child thread |
| 状态落在哪 | 内存 + `~/.claude/projects/…/subagents/` | **磁盘**：`~/.claude/teams/{team}/config.json`、`inboxes/*.json`、`~/.claude/tasks/{team}/` | 内存 + transcript | 脚本变量 + 落盘的脚本文件 | 服务端 |

---

## §2 原理：消息怎么走、上下文怎么切、状态落在哪

### 2.1 通信是「工具调用」还是「运行时调度」？

**两者都有，但分工明确。**

- **「agent 通过工具调 agent」是 Claude Code 这一线的主形态**。subagent 是 `Agent` 工具调出来的；teammate 也是 `Agent` 工具调出来的（同一个工具，`name` 参数决定是哪一个）；消息是 `SendMessage` 工具发出去的。**没有「运行时替我调度另一个 agent」这种 API**。
- **运行时调度只出现在两处**：
  1. **动态 workflow**：脚本（由 Claude 写）被运行时执行，运行时替脚本并行地跑 `agent()` / `pipeline()` / `parallel()`。调度者是**代码**，不是模型。
  2. **Managed Agents**：服务端按 agent 定义里的 `multiagent` 块调度，Claude 写 workflow 时由服务器跑。
- 因此「运行时调度」在 Claude 这条线上**不是独立机制，而是「把调度从模型手里挪到代码 / 服务端手里」的一个变体**。

### 2.2 共享状态落在哪

按层次分三种（全部有据）：

1. **文件（团队级）** —— agent teams 的团队状态**全在磁盘**：
   - `~/.claude/teams/{team-name}/config.json`：**团队配置**，含 `members` 数组（每个成员的名字与 agent ID；**lead 的项固定带 agent type `team-lead`**；teammate 的项带 lead 命名时用的 agent type，lead 没命名就省掉这个字段）。里面还有 session ID、tmux pane ID 等运行时状态，**不要手改**，下次状态更新会覆盖。**会话结束时整个团队配置目录被删**。**没有项目级等价物** —— 项目里的 `.claude/teams/teams.json` 不被识别。
   - `~/.claude/teams/{team-name}/inboxes/{agent-name}.json`：**信箱**，每个 agent 一个 JSON 文件。
   - `~/.claude/tasks/{team-name}/`：**共享任务板**，会话结束后仍在盘上，由 `cleanupPeriodDays` 决定保留多久。
   - `team-name` = `session-` + 会话 ID 前 8 位。
2. **文件（会话级）** —— subagent 的转录在 `~/.claude/projects/{project}/{sessionId}/subagents/agent-{agentId}.jsonl`；主会话压缩不影响它们；随 `cleanupPeriodDays` 清理。
3. **内存 / 上下文** —— workflow 的中间结果在**脚本变量**里（刻意不进 Claude 上下文）；subagent 的中间步骤在它自己的上下文里（**不落盘**），只有最终报告与转录文件留下。

**一个更一般的模式**（Anthropic 工程博客《How we built our multi-agent research system》，2025-06-13，<https://www.anthropic.com/engineering/multi-agent-research-system>，2026-10-20 抓取）叫 **“Subagent output to a filesystem to minimize the ‘game of telephone.’”** 原文：

> “Rather than requiring subagents to communicate everything through the lead agent, implement artifact systems where specialized agents can create outputs that persist independently. Subagents call tools to store their work in external systems, then pass lightweight references back to the coordinator. This prevents information loss during multi-stage processing and reduces token overhead from copying large outputs through conversation history.”

官方 demo `research-agent` 就是这个形状（`files/research_notes/`、`files/charts/`、`files/reports/`）。

### 2.3 子 agent 的产物怎么回到父的上下文

**这是「摘要」，不是全文。** 三条独立证据：

1. subagent 页：父只看得到最终结果，看不到中间的工具调用与输出。
2. SDK 页：父收到 subagent 的最终报告，**但可能在自己的回答里再摘要一次**；要原文出现在面向用户的回答里，得在 prompt 或 `systemPrompt` 里明说。
3. Anthropic 官方文章《Building multi-agent systems: When and how to use them》（2026-01-23，<https://claude.com/resources/articles/building-multi-agent-systems-when-and-how-to-use-them>，2026-10-20 抓取）把这条讲成一条通用经验：

> “multi-agent implementations typically use 3-10x more tokens than single-agent approaches for equivalent tasks. This overhead stems from duplicating context across agents, coordination messages between agents, and summarizing results for handoffs.”

同文还给出反模式：按问题类型切 agent 会造成 **“telephone game”**，每交接一次保真度掉一截；实测里那些 agent **花在协调上的 token 比花在正事上的还多**。

**在 agent teams 里，产物有两条回的路**：

- **走消息**：teammate 完成时**自动通知 lead，通知里带最终答案**（§1.2.5）。
- **走盘**：subagent 写文件，只把轻量引用交回协调者（上面那条 filesystem 模式）。

### 2.4 上下文怎么切

三条不同的切法（**不是同一条**）：

| 切法 | 机制 | 谁拿得到什么 |
| --- | --- | --- |
| **「全新的窗口」**（subagent / teammate / workflow agent） | 每个 agent 一个独立上下文窗口 | 只有：自己的 system prompt + 委派 prompt + CLAUDE.md + git status + 预载 skill（+ 有 `SendMessage` 时的 sibling roster）。**没有**父的历史、预载 skill、输出风格、auto memory |
| **「复制一份」**（fork） | fork 继承父的**完整对话** | 父的历史 + 父的工具集（跳过两层过滤）；fork 模式下 fork 在后台跑 |
| **「同一份流，多个发言人」**（heng 讨论会话那种） | Claude Code 里**没有**这种形态 | —— |

⚠️ Anthropic 官方文章明确：**「轮次化的多 agent 讨论」在 API 层面不可能**，因为调用无状态；能做的只是每轮把发言重放给每个人。这与 heng 既有的「讨论会话 = 单流多 speaker」设计同源（heng 自己的 `multi-agent-architecture` 图也已经写下了这条）。

### 2.5 官方对多 agent 何时划算的判断

三篇 Anthropic 官方材料的口径一致，值得单列（这是 heng 做决策时最该引用的部分）：

- **《How we built our multi-agent research system》**（2025-06-13）：多 agent 系统 excels 于 **breadth-first 查询**；Opus 4 lead + Sonnet 4 subagents 在内部 research eval 上比单 agent Opus 4 高 **90.2%**。**方差归因**：token 用量单独解释 **80%** 的方差，工具调用次数与模型选择是另外两个因素。但代价是：agent 约用 **4×** chat 的 token，多 agent 系统约 **15×**。且明确写：**“most coding tasks involve fewer truly parallelizable tasks than research, and LLM agents are not yet great at coordinating and delegating to other agents in real time.”**
- 同文还记了三个具体的协作教训：**(1) 教 orchestrator 怎么委派** —— 每个 subagent 需要目标、输出格式、工具与来源指引、清晰的任务边界；指令含糊会导致重复劳动与留白。**(2) 把 effort 规则写进 prompt** —— 简单事实查找 1 个 agent 3–10 次工具调用；直接比较 2–4 个 subagent、每个 10–15 次；复杂研究 10+ 个 subagent、职责清晰切分。**(3) 让 orchestrator 与 subagent 都用 extended thinking**，thinking 当可控草稿纸。以及一条**当下仍未解**的：**“Currently, our lead agents execute subagents synchronously… the lead agent can't steer subagents, subagents can't coordinate, and the entire system can be blocked while waiting for a single subagent to finish.”**
- **《Building multi-agent systems: When and how to use them》**（2026-01-23）：多 agent 值得投入的三个场景是**上下文保护**（子任务产生大量不相关上下文）、**并行化**（主要收益是**覆盖度而非速度**，多 agent 常常总耗时更长）、**专用化**（工具集 / 系统提示 / 领域知识分家）。同文也点名了「能力型系统」「消息总线架构」「agent swarm」这些别的协调形态，但说自己只讲 orchestrator-subagent 层级模式。

**综合起来，Claude 这条线对 heng 的直接结论是**：官方自己都认为**「agent 之间实时互相协调」在 2026 年仍是未解问题**，Claude Code 的 agent teams 是**用磁盘信箱 + 共享任务板 + 显式消息工具**去逼近它，而不是声称解决了。

---

## §3 与 heng 的差异点与可移植部分

### 3.1 heng 今天的位置（据 [`.scratch/agent-teams/seed.md`](../seed.md) 与仓库代码核实）

| 维度 | heng 现状 | Claude Code 对应物 |
| --- | --- | --- |
| 多会话 | 同一进程内多个会话，各一个 `Harness` + 一个 tokio task（`src/session/host.rs`） | in-process teammate / agent view 的后台会话 |
| 会话间通道 | **没有** | `SendMessage` + 磁盘信箱 |
| 共享待办 | **没有** | `~/.claude/tasks/{team}/` + Task 工具 |
| 派发 | `task` 工具派**执行者**，嵌套在一次 `run_turn` 里、共用同一份事件流（`ExecutorSpawned`/`ExecutorFinished`）、**不可再派发**（`src/tools/task.rs` 的 `delegable()` 返回 false） | `Agent` 工具派 subagent，**独立上下文**，最深 3 层 |
| 模型自己的计划 | `todo(items)` 工具，**整份列表存在调用参数里**，replace-all，模型所有，没有人在看（`src/tools/todo.rs`） | Task 工具组（`TaskCreate`/`TaskGet`/`TaskList`/`TaskUpdate`） |
| 「同目录」的判据 | 会话**桶**（按 cwd slug，`src/session/store.rs`）或**检出**（按 git common dir）—— 取哪个是待定决策 | 团队按**会话**划（一个会话一个隐含团队）；跨会话共享靠 `CLAUDE_CODE_TASK_LIST_ID` 显式指一个目录 |
| 已有 hooks | `src/hooks.rs` 已有机制 | 三十多个 hook 事件，含 `TeammateIdle` / `TaskCreated` / `TaskCompleted` |

### 3.2 机制层面的差异（不是「抄不抄」，是「同一个问题的不同解法」）

| 问题 | Claude Code 的解法 | heng 的现状 | 差异要点 |
| --- | --- | --- | --- |
| 消息落在哪 | **收件方的信箱文件**（`inboxes/{name}.json`），发件方写、收件方读 | 待定（进收件方 JSONL 流 vs 另开团队目录） | Claude 的信箱是**团队级旁路**，不进任何会话的转录；heng 的流是**可搬运的唯一真相源**，两者正面冲突 |
| 消息怎么被读到 | 投递时**自动**送达（发件方写文件后，收件方在下一轮读到） | 待定（与 `interjection-flow` 相接：排队 / 打断 / 只在空闲时投递） | Claude 的「空闲通知」是**单向、只在空闲时**触发；不打断正在跑的回合 |
| 状态落在哪 | 团队状态**全在磁盘**（`config.json` + `inboxes/` + `tasks/`），进程内不持有 | 会话 = 一个可搬运目录（JSONL + `outputs/`） | Claude 的团队状态**不可搬运**（会话结束即删 `config.json`）；heng 的会话**必须可搬运** |
| 认领冲突 | **文件锁** | 无（无共享任务板） | 文件锁是磁盘原语，Rust 侧已有 `PathLocks`（`src/agent/executor.rs` 已 import）可复用 |
| 权限怎么隔离 | teammate 继承 lead 的 permission mode，**权限弹窗浮到 lead**；agent 之间的消息**永远不算用户的批准** | 四个权限模式（`readonly`/`ask`/`auto`/`plan`），单会话内 | 「消息不是批准」这条在 heng 里**必须显式建模**，否则跨会话的提示注入等于提权 |
| 团队范围 | 一个会话一个团队，**不可跨会话共享**，lead 固定 | 待定（桶 vs 检出） | Claude 用「一个会话 = 一个团队」回避了「团队边界」这个问题；heng 如果要跨会话共享团队，就得自己解决成员发现与 lead 归属 |
| 并行上限 | teammate 无硬上限；subagent 并发 20、深度 3 | 执行者**不可再派发**（递归深度为一） | Claude 的深度上限是**可调 env var**（默认 3），heng 是编译期固定 |
| 成员的可见性 | agent panel：上下键选、Enter 进转录直接发消息、空闲 30 秒隐藏、超 3 个折叠 | 已有平铺格与左栏多页；缺的是「关系」这一层 | Claude 的面板**把 subagent 与 teammate 混在一起**，且明说「面板有行 ≠ 组成了团队」 |

### 3.3 可复刻（有公开规格，可照抄形状）

以下机制的**规格是公开的**（文档写到了字段级），heng 可以复刻：

1. **信箱 = 每成员一个文件 + 消息条目数组**（`{from, content}` 之类），**发送成功 = 写文件成功**，写失败就是没发出去并向发件方报错。规格：`~/.claude/teams/{team}/inboxes/{agent}.json`，读到时逐条校验，坏条目剔除并报错。
2. **共享任务板 = 一个目录 + Task 四件套工具**（create/get/list/update），带**三态**（pending / in progress / completed）与**依赖边**，**依赖自动解锁**，**认领用文件锁**。任务板目录名可以是一个**与具体会话解耦的具名 id**（Claude 的 `CLAUDE_CODE_TASK_LIST_ID` 正是这个作用）。
3. **消息投递的三态收件策略**：`accept`（直接投）/ `hold`（扣下等批准）/ `refuse`（丢弃）。以及**跨机器的隔离键**（`isolatePeerMachines`）。
4. **「agent 之间的消息不是用户的批准」**这条信任边界，以及 auto / 分类器模式下对消息的额外复审。
5. **三个质量门 hook 事件**：`TeammateIdle`（teammate 要交活时）、`TaskCreated`、`TaskCompleted`（含「teammate 回合结束时仍有 in-progress 任务」这一触发时机）。**每条都能用 exit code 2 + stderr 反馈回给 agent 让它继续干** —— 这是把「评审」做成机制而不是提示词的做法。
6. **subagent 的上下文切分清单**（装什么 / 不装什么）—— 尤其「**不装**父的历史、输出风格、auto memory」与「装 CLAUDE.md 全层级 + git status 快照」这条线，以及 `omitClaudeMd` 这个开关。
7. **两层工具过滤**（全局扣件清单 + 后台子集白名单），以及**同一定义在前台 / 后台解出不同工具集**这个设计。
8. **深度 / 并发 / 花费三道闸**，各自一个 env var；花费闸含 subagent 的花费。
9. **「subagent 写盘、协调者只收轻量引用」**这条 artifact 模式（Anthropic 官方研究博客与官方 demo 都用它）。
10. **可复用的模型委派指引**：把 effort 规则写进 prompt（简单 1 agent / 比较 2–4 / 复杂 10+）。
11. **“同伴名单以 system reminder 进上下文”**：让每个 agent 知道有哪些可寻址的同伴，各是合法的收件人。

### 3.4 Claude 特有、看不到实现的部分

- **Claude Code 本体是闭源的**。`anthropics/claude-code` 这个 GitHub 仓库里**没有 CLI 源码** —— 只有 `CHANGELOG.md`、issue 模板、`.claude/` 命令、插件样例与 `demo.gif`（用 jsDelivr 列过仓库树，确认无 `src/`）。所以「信箱写入怎么做原子」「任务板怎么存」「面板怎么渲染」这些实现细节**无从复刻，只能照文档的外部行为造**。
- **in-process teammate 与 split-pane teammate 的差异**：文档只描述**行为差异**（哪些 frontmatter 字段生效、system prompt 是追加还是替换、模型与 effort 是否继承），**没有说它们是不是同一套代码路径**。in-process teammate 显然不是「另起一个 Claude Code 进程」，但它与主会话共享多少状态（prompt cache？进程？事件流？）**查不到**。
- **lead 与 teammate 的消息投递路径**：文档说消息落在磁盘信箱上，但**信箱被读进模型上下文时的确切形态**（是一次 user 消息？一个 system reminder？一个 tool result？）**没有文档**。SDK 侧只给出 `origin: { kind: "peer" }` 这个**接收方可见的分类**。
- **提示词层面的实现**：Claude Code 的系统提示词不在公开仓库里（`CLAUDE_CODE_SIMPLE` 只是把它剥掉），所以「lead 知道自己是 lead」这件事**完全靠文档描述的现象**反推。

### 3.5 值得 heng 抄的三个机制（本文只给候选，不给方案）

1. **信箱 = 每成员一个文件，发送成功 = 写成功**。这条把「消息有没有送到」从「模型觉得自己发了」变成**可核验的事实**（Claude 明确说 v2.1.224 之前写失败也报成功）。heng 的流是唯一真相源，**不能**照抄「另开一份团队目录」；但**「投递成功与否要有一个可核验的判据」**这条原则可以直接落到 heng 的事件流上（一条 `MessageDelivered` 事件，而不是只有一条 `MessageSent`）。
2. **共享任务板 = 带依赖边的清单 + 文件锁认领 + 依赖自动解锁**。heng 现在的 `todo(items)` 是**模型自己的计划**（整份列表在调用参数里，replace-all，模型所有，没有人在看）；Claude 的 Task 四件套是**团队共享的、可被他人认领的、可被 hook 把关的**。这两个是不同的东西 —— 如果 heng 要做团队板，**不是扩展 `todo`，是另立一个带 ID 的共享实体**。
3. **三个质量门 hook 事件（`TeammateIdle` / `TaskCreated` / `TaskCompleted`）+「exit 2 把反馈回给 agent 让它继续干」**。这是把「评审」从提示词变成**机制**的最小代价做法，且天然适配 heng 已有的 `hooks.rs`。其中「**teammate 回合结束时仍有 in-progress 任务**」这个触发时机尤其值得抄 —— 它正好对应 heng 讨论里「谁负责把任务标完成」这个悬着的问题。

另外两条**值得 heng 注意但不必抄**的：

- **磁盘信箱与「可搬运的会话目录」正面冲突**（Claude 的团队状态会话结束即删、不可搬运）。heng 若要跨会话共享团队，要么接受「团队状态不可搬运」，要么另找落点 —— 这是 §1.2.3 那个「一个会话一个团队」的简化给 heng 留的余地。
- **官方自认「agent 实时互相协调仍未解」**（Anthropic 2025-06 博客原话）。heng 若把团队当成**主要卖点**，要准备好它的收益主要来自**覆盖度与上下文保护**，而不是速度。

---

## §4 查不到 / 不确定的地方

按任务要求逐条列出**查过哪些地方**。

| 问题 | 状态 | 查过的地方 |
| --- | --- | --- |
| **agent teams 的订阅档位要求** | ❓ **查不到当前要求** | `code.claude.com/docs/en/agent-teams`（全文搜 plan / subscription / Max / Pro，只命中 plan mode）、`/docs/en/costs`（agent team token costs 一节只提 env var）、`/docs/en/env-vars`（只有 `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS`）、`/docs/en/settings-reference`（`teammateMode` 一节不提档位）、`support.claude.com` 的 Team/Enterprise 与 Pro/Max 两篇（只讲席位与额度）。**反证**：`anthropics/claude-code` CHANGELOG v2.1.33 段有 “Fixed warnings about agent teams not being available on your current plan” —— 说明**历史上确实有档位门槛**。当前是否还在，文档没写。二手来源（`claudelogai.space`、日本媒体）提到过「不在套餐上就隐藏」，但那是二手，不作依据。 |
| **Claude Code 是否有公开源码** | ✅ **确认没有** | jsDelivr 列 `anthropics/claude-code@main` 仓库树：只有 `CHANGELOG.md`、issue 模板、`.claude/commands/`、`plugins/`、`demo.gif`、`README.md`、`SECURITY.md`，**无 `src/`**。所以 agent teams 的实现**不可读**。 |
| **teammate 数量是否真有硬上限** | ✅ **没有**（文档明说） | agent-teams 页 “Choose an appropriate team size”：“There's no hard limit on the number of teammates”。sub-agents 页确认 teammate **不受** 20 并发上限约束。 |
| **agent teams 是否进过正式发布（GA）** | ❓ **未见** | 文档 Warning 仍写 “experimental and disabled by default”；changelog 从 v2.1.32（研究预览）到 v2.1.304 一直在修 bug，**未见 GA 公告**。`claude.com/blog` 与 `anthropic.com/news` 本轮未检到 agent teams 的发布博文。 |
| **「agents-as-tools」是不是官方范式名** | ❌ **上游无此名** | 搜遍 `code.claude.com/docs/en/agent-sdk/*`、`anthropic.com/engineering/building-effective-agents`（五个 workflow 标题：prompt chaining / routing / parallelization / orchestrator-workers / evaluator-optimizer）、`platform.claude.com/docs/en/managed-agents/multiagent-orchestration`（三行表：subagents / dynamic workflows / advisor）。**最接近的是 advisor**：只有主线程能咨询、按顾问模型价计费。Claude Code 侧对应的是 advisor tool（server 侧工具，非 Claude Code 实现）。 |
| **MCP server 能否作为「团队成员」参与** | ❌ **上游无此形态** | `code.claude.com/docs/en/agents` 明说 “In every approach the workers are Claude sessions. To involve a different tool, expose it to Claude as an MCP server.”；`/docs/en/mcp` 与 `/docs/en/channels` 里的 MCP 角色是**工具 / 事件源**。反方向（把外部事件推进会话）由 channels 提供，但那不是 agent。 |
| **teammate 消息进模型上下文的形态** | ❓ **查不到** | 文档只说「自动投递」与磁盘信箱路径；hooks 与 SDK 类型里只暴露 `origin.kind: "peer"` 与 `teammate_name` / `team_name` 字段，**没有**描述消息以何种消息类型进入模型。 |
| **in-process teammate 与主会话共享什么** | ❓ **查不到** | 文档只给行为差异（哪些 frontmatter 生效、system prompt 追加 vs 替换、模型与 effort 是否继承、cache TTL 桶不同）。进程内共享、事件流、prompt cache 的具体关系未写。 |
| **lead 如何「等」teammate** | ⚠️ **没有阻塞原语** | 文档给的全是自然语言提示（“Wait for your teammates…”）与**自动空闲通知**；`notify_when_idle` 订阅**明确不适用于 teammate**（只对同机自己的会话）。 |
| **claude-code 仓库里有没有 agent teams 相关的可读代码** | ❌ **没有** | 仓库树已列；`plugins/` 里只有 security-guidance 等示例插件，与 team 无关。 |
| **Agent SDK 能不能开 agent teams** | ✅ **不能** | agent-teams 页：“Spawning teammates also requires an interactive session. In non-interactive mode with the `-p` flag, including Agent SDK sessions, Claude doesn't spawn teammates.”；SDK 参考里 `AgentInput.team_name` 标 `// Deprecated; ignored`。 |
| **本地笔记提到的「split-pane teammates running as separate processes」是否仍准确** | ⚠️ **需修正** | 现在默认是 in-process；split-pane 才是独立进程（tmux / iTerm2），且依赖 `it2` / `tmux`。见 §0。 |
| **上游文档描述的版本与 changelog 的对应** | ⚠️ **对不上，无妨** | 文档提到 v2.1.296 / v2.1.304 级别的特性，而 `claude-code` CHANGELOG 仓库 HEAD 是 2.1.304（2026-10-19）；本地笔记（2026-09）提到的版本区间（v2.1.17x–v2.1.26x）已明显过时。**所有带版本号的说法按「2026-10-20 抓取时的文档」理解。** |

---

## §5 来源清单

全部于 **2026-10-20** 实际抓取。`📄` = 官方文档（`code.claude.com/docs`，Mintlify 的 `.md` 原文）；`🔧` = 官方 GitHub 源码 / CHANGELOG（`raw.githubusercontent.com` / `api.github.com` / jsDelivr 树）；`📝` = Anthropic 官方博客 / cookbook / 工程文章；`📘` = 本地笔记。

### 5.1 一手源 —— 官方文档

| # | 来源 | URL | 抓取方式 | 本文引用处 |
| :- | --- | --- | --- | --- |
| 1 | 📄 **Agent teams**（本文重点） | <https://code.claude.com/docs/en/agent-teams> | curl `.md` 原文（495 行） | §1.2 全部、§3.2 |
| 2 | 📄 **Create custom subagents** | <https://code.claude.com/docs/en/sub-agents> | curl `.md` 原文（1469 行） | §1.1 全部 |
| 3 | 📄 **Run agents in parallel**（五种方式总表） | <https://code.claude.com/docs/en/agents> | curl `.md` | §1.4.4、§1.4.5 |
| 4 | 📄 **Message your other Claude Code sessions** | <https://code.claude.com/docs/en/cross-session-messaging> | curl `.md`（346 行） | §1.2.5、§2.2 |
| 5 | 📄 **Orchestrate subagents at scale with dynamic workflows** | <https://code.claude.com/docs/en/workflows> | curl `.md`（502 行） | §1.3.3-E、§2.2 |
| 6 | 📄 **Hooks reference** | <https://code.claude.com/docs/en/hooks> | curl `.md` | §1.4.1 |
| 7 | 📄 **Tools reference** | <https://code.claude.com/docs/en/tools-reference> | curl `.md` | §1.1.5、§1.2.5 |
| 8 | 📄 **Subagents in the SDK** | <https://code.claude.com/docs/en/agent-sdk/subagents> | curl `.md`（764 行） | §1.3.1、§1.3.3、§1.3.4、§1.3.5 |
| 9 | 📄 **Agent SDK reference - TypeScript** | <https://code.claude.com/docs/en/agent-sdk/typescript> | curl `.md` | §1.2.1、§1.3.3-A |
| 10 | 📄 **Agent SDK reference - Python** | <https://code.claude.com/docs/en/agent-sdk/python> | curl `.md` | §1.3.3-A |
| 11 | 📄 **How the agent loop works** | <https://code.claude.com/docs/en/agent-sdk/agent-loop> | curl `.md` | §1.3.1 |
| 12 | 📄 **Agent SDK overview** | <https://code.claude.com/docs/en/agent-sdk/overview> | curl `.md` | §1.3.1 |
| 13 | 📄 **Examples**（SDK 示例入口） | <https://code.claude.com/docs/en/agent-sdk/examples> | web_fetch | §1.3.3 |
| 14 | 📄 **Environment variables** | <https://code.claude.com/docs/en/env-vars> | curl `.md` | §1.2.1、§1.1.6、§1.2.5 |
| 15 | 📄 **All settings**（`teammateMode`） | <https://code.claude.com/docs/en/settings-reference> | curl `.md` | §1.2.8 |
| 16 | 📄 **Manage costs effectively** | <https://code.claude.com/docs/en/costs> | curl `.md` | §1.2.8 |
| 17 | 📄 **Error reference**（teammate inbox 错误） | <https://code.claude.com/docs/en/errors> | curl `.md` | §1.2.5 |
| 18 | 📄 **Interactive mode**（任务列表 / `CLAUDE_CODE_TASK_LIST_ID`） | <https://code.claude.com/docs/en/interactive-mode> | curl `.md` | §1.2.4 |
| 19 | 📄 **Explore the .claude directory**（清理策略） | <https://code.claude.com/docs/en/claude-directory> | curl `.md` | §2.2 |
| 20 | 📄 **Manage agents with agent view** | <https://code.claude.com/docs/en/agent-view> | curl `.md` | §1.4.4、§1.4.5 |
| 21 | 📄 **Run parallel sessions with worktrees** | <https://code.claude.com/docs/en/worktrees> | curl `.md` | §1.4.4 |
| 22 | 📄 **Push events into a running session with channels** | <https://code.claude.com/docs/en/channels> | web_fetch | §1.4.3 |
| 23 | 📄 **Mods reference**（`agent.spawn` 事件） | <https://code.claude.com/docs/en/plugins/mods/reference> | curl `.md` | §1.4.1 |
| 24 | 📄 **Let Claude coordinate ongoing work with Projects** | <https://code.claude.com/docs/en/claude-projects> | web_fetch | §1.4.5 |
| 25 | 📄 **Claude Code changelog**（文档站生成版） | <https://code.claude.com/docs/en/changelog> | web_fetch | §1.2.1 |
| 26 | 📄 文档索引 `llms.txt` | <https://code.claude.com/docs/llms.txt> | web_fetch | 全文检索用 |

### 5.2 一手源 —— 官方 GitHub

| # | 来源 | URL | 抓取方式 | 本文引用处 |
| :- | --- | --- | --- | --- |
| 27 | 🔧 `anthropics/claude-code` **CHANGELOG.md**（main） | <https://raw.githubusercontent.com/anthropics/claude-code/main/CHANGELOG.md> | curl | §1.2.1（近期修复条目） |
| 28 | 🔧 同上，tag `v2.1.183`（含 v2.1.178/2.1.234 段） | <https://raw.githubusercontent.com/anthropics/claude-code/v2.1.183/CHANGELOG.md> | curl | §1.2.1（`TeamCreate`/`TeamDelete` 移除） |
| 29 | 🔧 同上，tag `v2.1.45`（含 v2.1.32/2.1.33 段） | <https://raw.githubusercontent.com/anthropics/claude-code/v2.1.45/CHANGELOG.md> | curl | §1.2.1（研究预览首发、`TeammateIdle` 引入） |
| 30 | 🔧 同上，tag `v2.1.234` / `v2.1.190` / `v2.1.150` / `v2.1.105` | 对应 `raw.githubusercontent.com` 路径 | curl | §1.2.1（bug 修复史） |
| 31 | 🔧 `anthropics/claude-code` **仓库树**（证明无 CLI 源码） | <https://data.jsdelivr.com/v1/packages/gh/anthropics/claude-code@main?structure=flat> | web_fetch | §3.4、§4 |
| 32 | 🔧 `anthropics/claude-agent-sdk-python` `examples/agents.py` | <https://raw.githubusercontent.com/anthropics/claude-agent-sdk-python/main/examples/agents.py> | web_fetch | §1.3.3-A |
| 33 | 🔧 `anthropics/claude-agent-sdk-demos` `research-agent/agent.py` | <https://raw.githubusercontent.com/anthropics/claude-agent-sdk-demos/main/research-agent/research_agent/agent.py> | web_fetch | §1.3.3-B |
| 34 | 🔧 同上 `utils/subagent_tracker.py`（hooks + 流两路追踪） | <https://raw.githubusercontent.com/anthropics/claude-agent-sdk-demos/main/research-agent/research_agent/utils/subagent_tracker.py> | web_fetch | §1.3.3-B |
| 35 | 🔧 `anthropics/claude-agent-sdk-demos` 仓库树 | <https://data.jsdelivr.com/v1/packages/gh/anthropics/claude-agent-sdk-demos@main?structure=flat> | web_fetch | §1.3.3 |
| 36 | 🔧 `anthropics/claude-agent-sdk-typescript` README | <https://raw.githubusercontent.com/anthropics/claude-agent-sdk-typescript/main/README.md> | web_fetch | §1.3（包名与迁移说明） |
| 37 | 🔧 `anthropics/agent-sdk-workshop` `docs/CHEATSHEET.md` | <https://raw.githubusercontent.com/anthropics/agent-sdk-workshop/main/docs/CHEATSHEET.md> | web_fetch | §1.3.3-A（工具 / subagent / hook 的最小形状） |
| 38 | 🔧 `anthropics/skills` `shared/managed-agents-multiagent.md` | <https://raw.githubusercontent.com/anthropics/skills/main/skills/claude-api/shared/managed-agents-multiagent.md> | web_fetch | §1.3.2（“start with self, then add cheaper workers”） |

### 5.3 一手源 —— Anthropic 官方博客 / cookbook

| # | 来源 | URL | 抓取方式 | 本文引用处 |
| :- | --- | --- | --- | --- |
| 39 | 📝 **Building effective agents**（2024-12-19）—— 五个 workflow 模式原文 | <https://www.anthropic.com/engineering/building-effective-agents> | web_fetch + curl 转纯文本 | §1.3.2 |
| 40 | 📝 **How we built our multi-agent research system**（2025-06-13） | <https://www.anthropic.com/engineering/multi-agent-research-system> | web_fetch + curl 转纯文本 | §2.2、§2.3、§2.5 |
| 41 | 📝 **Building multi-agent systems: When and how to use them**（2026-01-23） | <https://claude.com/resources/articles/building-multi-agent-systems-when-and-how-to-use-them> | web_fetch + curl 转纯文本 | §2.3、§2.5 |
| 42 | 📝 **Agent harness design: 3 patterns for harnessing Claude's intelligence**（2026-04-02） | <https://claude.com/resources/articles/harnessing-claudes-intelligence> | web_fetch | §2.5（harness 三模式的前置） |
| 43 | 📝 **A harness for every task: dynamic workflows in Claude Code**（2026-06-02） | <https://claude.dev/blog/a-harness-for-every-task-dynamic-workflows-in-claude-code> | web_fetch | §1.3.3-E |
| 44 | 📝 **Multiagent orchestration**（Managed Agents 官方文档） | <https://platform.claude.com/docs/en/managed-agents/multiagent-orchestration> | curl `.md` | §1.3.2、§2.2 |
| 45 | 📝 Cookbook：**Async multi-agent orchestration**（fixed N-agent team + 对等消息） | <https://platform.claude.com/cookbook/patterns-agents-async-multi-agent-orchestration> | web_fetch | §1.3.3-C、§2.2 |
| 46 | 📝 Cookbook：**Multiagent: coordinate a specialist team** | <https://platform.claude.com/cookbook/managed-agents-cma-coordinate-specialist-team> | web_fetch | §1.3.3-D |
| 47 | 📝 Cookbook：**Basic workflows**（prompt chaining / parallelization / routing 的可运行实现） | <https://platform.claude.com/cookbook/patterns-agents-basic-workflows> | web_fetch | §1.3.2 |
| 48 | 📝 **Claude Code 插件官方仓库**（channels 用的 telegram/discord/imessage 插件） | <https://github.com/anthropics/claude-plugins-official> | 引用自 /docs/en/channels | §1.4.3 |

### 5.4 本地笔记（**未修改**）

| # | 来源 | 路径 | 引用处 |
| :- | --- | --- | --- |
| 49 | 📘 **Claude Code vs. Amp 调研笔记**（2026-09） | [`docs/research/notes/claude-code-amp.md`](../../../../docs/research/notes/claude-code-amp.md) | §0 全表；具体章节：§1.1（行 24–53）、§1.2（行 55–106）、§1.7（行 361–391）、§1.8（行 393–417）、§1.9（行 419–428）、§3 跨工具差异表（行 1456–1477） |
| 50 | 📘 **heng 的 agent-teams 种子材料** | [`.scratch/agent-teams/seed.md`](../seed.md) | §3.1（heng 现状） |

### 5.5 二手（只作线索，未用于支撑任何结论）

- `claudelogai.space`（2.1.33 的「agent teams 计划警告修复」条目）—— 用于定位 changelog 里的对应条目，结论仍以 changelog 原文为准。
- 日本媒体（AI 革命株式会社 `ai-revolution.co.jp` 关于 Agent Teams 定价）与 `claude-code-log.com` 的 `teammateMode` 页 —— 用于确认「档位门槛」这个疑问**存在过**，**未**用于断言当前要求。
- `developer.aliyun.com`、`tech.dentsusoken.com`、HuggingFace 上的第三方研究报告 —— 未引用。

---

## §6 给下游的一页速查

1. **Claude Code 有 agent teams**：是。文档页 <https://code.claude.com/docs/en/agent-teams>，实验特性、默认关闭、开法是 `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`、**只对交互式会话生效**（`-p` 与 Agent SDK 不派发 teammate）。
2. **成员怎么来**：主 agent 在 `Agent` 工具调用上带 `name` ⇒ 派出的是 teammate，不是 subagent。v2.1.178 起**没有建团队步骤**（`TeamCreate`/`TeamDelete` 已删），一个会话一个隐含团队。
3. **状态落在哪**：**全在磁盘** —— `~/.claude/teams/{team}/config.json`（成员表）、`inboxes/{name}.json`（信箱）、`~/.claude/tasks/{team}/`（共享任务板）。`team-name` = `session-` + 会话 ID 前 8 位。
4. **消息怎么走**：`SendMessage` 工具 ⇒ 写收件人的信箱文件 ⇒ **写成功才算发出**。teammate 完成时自动通知 lead（带最终答案）。团队协议消息（shutdown / plan approval）是结构化的，且**要求 agent teams 开启**。
5. **任务板**：Task 四件套（`TaskCreate`/`TaskGet`/`TaskList`/`TaskUpdate`），三态 + 依赖边 + 依赖自动解锁 + **文件锁认领**。没有 Task 工具的 agent 改用消息协调。跨会话共享靠 `CLAUDE_CODE_TASK_LIST_ID`。
6. **并发**：teammate **无硬上限**（官方建议 3–5 个起步）；subagent 并发 20、深度 3，两个 env var 可调；花费闸 `maxBudgetUsd` 含 subagent。
7. **UI**：默认 **in-process**（同一终端同一进程）；`teammateMode` 可切 `auto`/`tmux`/`iterm2` 的 split pane。面板在提示框下方，↑↓ 选、Enter 进转录并直接说话，空闲 30 秒自动隐藏、超过 3 个折叠成计数行。
8. **子 agent 产物怎么回父**：**摘要**，不是全文。官方三处独立确认；想保真就让子 agent **写盘**、只回轻量引用。
9. **原理定性**：通信**主要是「agent 通过工具调 agent」**；运行时调度只出现在动态 workflow（脚本）与 Managed Agents（服务端编排）两处。子 agent 上下文**全新**（fork 除外），装载 CLAUDE.md + git status + 预载 skill，**不装载**父的历史/输出风格/auto memory。
10. **对 heng 最值得抄的三条**：**投递成功要可核验**（写成功才算发出）、**共享任务板是带依赖与锁的独立实体**（不是扩展现有 `todo`）、**质量门 hook 事件**（`TeammateIdle`/`TaskCreated`/`TaskCompleted` + exit 2 回喂反馈）。
11. **官方自认未解**：2025-06 的 Anthropic 工程博客明说「LLM agents are not yet great at coordinating and delegating to other agents in real time」，且 lead 目前**同步**等 subagent。多 agent 的主要收益是**覆盖度与上下文保护**，不是速度。
12. **查不到的三件事**：当前**订阅档位要求**（历史上有过，changelog v2.1.33 有修「not available on your current plan」警告的记录）、**消息进模型上下文的形态**、**in-process teammate 与主会话共享什么**。
