# 工具表这份账：形状、规模与它今天露过面的地方

> 产物：[`../issues/01-research-tool-table-shape.md`](../issues/01-research-tool-table-shape.md)（research，AFK）
> 只读调研。结论一律给 `文件:行号`。**没有跑过代码** —— 规模那一节是读源码字面量数的，
> 末尾标了「要精确值就跑一次」。

## 结论先说

1. **工具表在组装期建好，此后只读。** 全仓没有运行时的注册点，所以「一张会话级的表」这件事
   在衡里是成立的 —— 方向 2 的可行性前提就在这一条上。
2. **抄给渲染层的那本账只抄了 `parameters`。** 抄取点只有一处（`src/cli/probe.rs:325`），
   抄本叫 `tool_schemas`，存成 `BTreeMap<String, serde_json::Value>`。
3. **这本账今天在界面上只露过一处**：工具调用详情覆盖层的 `Schema` 面。它画的是**参数 schema**，
   不是完整声明 —— provider 收到的是 `name` + `description` + `parameters` 三件套
   （`src/provider/mod.rs:96-100`、`src/provider/openai.rs:441-455`）。
4. **规模**：19 处内建声明；`description` 合计 **5881 字符**，最长一条 `ask_user_question`
   **896 字符**；参数 schema 的源码字面量合计约 **1.16 万字符**（`pretty` 打印之后更大）。
   动态工具的条数随配置而变，**不受代码控制**。
5. **重放老会话看到的是今天的表**，不是当时那张 —— 装载那条路会**重新建一张**。

## 1. 这张表是什么、由谁建

- `Registry`（`src/tools/registry.rs:47`）是**会话携带的一个值**，不是全局静态。
- `Registry::specs()`（`src/tools/registry.rs:91`）按稳定的名字顺序给出 `Vec<ToolSpec>`；
  顺序稳定是因为「工具数组是缓存前缀的一部分」（同文件 `:88-90`）。
- `ToolSpec` 三件套：`name` / `description` / `parameters`（`src/provider/mod.rs:96-100`）。
- 装配链三处**逐字同形**：
  - 单 agent 那条路：`src/cli/interactive.rs:223-229`；
  - 讨论 CLI 那条路：`src/cli/discuss.rs:180-186`；
  - 装载老会话那条路：`src/cli/loading.rs:121-127`。
  三处都是 `tools::with_mcp(tools::with_web(tools::with_dynamic(...)))`。
- **组装后不变**：全仓 `Registry::register` 的调用点**只在** `src/tools/mod.rs` 里
  （`:95-108` 的内建十三项 —— `ask_user_question` 按 `can_ask` 条件挂载、`:121-124` 的动态工具、
  `:136-145` 的联网两项、`:159-172` 的 MCP 四个元工具）。**没有任何一处运行时注册**。
  文档里同一条不变式：`docs/lifecycle.md:488`（「工具表组装后不变」）。
- **执行者那张是子集**：`Registry::for_executor()`（`src/tools/registry.rs:70-78`）滤掉
  不可委派的那些，所以主会话那张覆盖得了这个会话里出现的每一次调用
  （`src/render/tui.rs:414-415` 已经这么写了）。

## 2. 表里有多少东西

内建声明逐条（`description` 的字符数按源码字面量数，`bash` 那条含 `SANDBOX_NOTE` 134 字符与
`ESCALATION_NOTE` 226 字符两段附注，已算进去）：

| 工具 | 文件 | `description` 字符 |
| --- | --- | --- |
| `ask_user_question` | `src/tools/ask_user.rs:125` | 896 |
| `bash` | `src/tools/bash.rs:83` | 856 |
| `git` | `src/tools/git.rs` | 532 |
| `grep` | `src/tools/grep.rs` | 472 |
| `todo` | `src/tools/todo.rs` | 447 |
| `read_file` | `src/tools/file.rs:173` | 443 |
| `glob` | `src/tools/glob.rs` | 361 |
| `web_fetch` | `src/tools/web_fetch.rs` | 234 |
| `repo_map` | `src/tools/repo_map.rs` | 221 |
| `goal_note` | `src/tools/goal_note.rs` | 218 |
| `task` | `src/tools/task.rs` | 217 |
| `mcp_list` | `src/tools/mcp_list.rs` | 173 |
| `mcp_resources` | `src/tools/mcp_resources.rs` | 167 |
| `web_search` | `src/tools/web_search.rs` | 158 |
| `mcp_call` | `src/tools/mcp_call.rs` | 154 |
| `mcp_read` | `src/tools/mcp_resources.rs` | 106 |
| `edit_file` | `src/tools/file.rs:339` | 90 |
| `skill` | `src/tools/skill.rs` | 73 |
| `write_file` | `src/tools/file.rs:267` | 63 |

**合计 19 条、5881 字符。**

参数 schema 的源码字面量合计约 **11600 字符**；`serde_json::to_string_pretty` 之后还要大
（换行与缩进都算进去）。**精确值要跑一次才知道**（本票是只读调研，没有跑）。

另有两条不在代码控制里的：

- **动态工具**：`[tools.<namespace>.<tool>]`（`src/config.rs:1591`、`:1694`），条数随配置而变；
  它的 `description` 与 `parameters` **整份来自用户配置**（`src/tools/custom.rs:67-69`）。
- **MCP 的四个元工具**：`mcp_list` / `mcp_call` / `mcp_resources` / `mcp_read`
  （`src/tools/mod.rs:167-172`）。**server 自己的工具永远不进表**（`src/events.rs:230-233`
  与 `CONTEXT.md` 的「元工具」词条）—— 所以表里那一段永远缺 server 侧的东西，
  界面上的「全表」永远是**这一张**、不是「外面所有的工具」。

## 3. 今天抄进渲染层的是哪一部分

- 抄取点只有一处：**`tool_schemas()`（`src/cli/probe.rs:325-331`）**：

  ```rust
  registry.specs().into_iter().map(|spec| (spec.name, spec.parameters)).collect()
  ```

  它抄的是 `parameters` 那一个字段 —— **用法说明（`description`）在这一步就丢了**。

- 抄本的形状：`BTreeMap<String, serde_json::Value>`（`src/render/tui.rs:416`），字段名
  `tool_schemas`。
- **字段的文档注释已经把 `parameters` 说成「声明」**（`src/render/tui.rs:407-408`：「这个会话的
  工具表里，每个工具的**声明 schema**」）。措辞失准的源头就在这里，而
  `docs/render.md:544` 那句「声明（发给 provider 的那份 JSON Schema）」是它的下游。
- **读者只有两处**，都在渲染层内部：
  - `src/render/tui.rs:8926`：宽度重放之后把面重排一遍；
  - `src/render/tui.rs:15711`：打开一块详情时造那几面。
  两处都把它传给 `detail_faces(...)` → `face_sections` → `tool_schema_rows`
  （`src/render/tui.rs:14996-15009`）。
- 查不到的时候写一句「工具表里没有「<名字>」，这次调用的声明查不到」
  （`src/render/wording.rs:549`），**不画空面、不编一份 schema 出来**。

## 4. 这本账今天在界面上露过面的地方（只有一处）

- **工具调用详情覆盖层的 `Schema` 面**：`FaceId::Schema`（`src/render/tui.rs:14500-14502`），
  工具那一行的**第三面**（`DETAIL_FACES`，`src/render/tui.rs:14554-14564`，六面：参数 / 输出 /
  Schema / 计时 / 来源 / 概述）。
- 它画的是 `parameters` 那一份，`pretty` 打印后按正文宽折行，与参数面同一套排法
  （`src/render/tui.rs:14996-15009`）。
- **别的出口都没有**：
  - `sessions show` 不打工具表（`src/cli/sessions.rs` 里没有 `tool_schemas` 的读者）；
  - 左栏（用量 / 文件 / 改动 / 待办各页）不画它；
  - 三个渲染器里只有 TUI 有详情覆盖层，plain 与 headless 都没有「面」这套东西。
- 所以种子那句话是对的：**这个面上没有任何地方能读到 `description`**。
  `ask_user_question` 的用法说明（「何时问、答案 JSON 怎么读、题面宜短、(推荐) 放第一个」，
  `src/tools/ask_user.rs:125-139`）在界面上一个字都读不到。

## 5. 这张表在什么时候变（决定「看到的是哪张表」）

- **组装期一次**：见第 1 节。表建好之后只读。
- **装载老会话时重新建一张**：`src/cli/loading.rs:121-139` 照着**当前**的配置重建工具表，
  再抄成 `tool_schemas`。于是 `--continue` 重开一个老会话，界面上看到的是**今天**那张表，
  **不是当时那张**。
- **讨论 CLI 那条路建的是同一张表**（`src/cli/discuss.rs:180-187` 与单 agent 那条路逐字同形），
  讨论者与合成器共享它（`docs/discussion.md:17`）。
- **执行者那张是子集**（`for_executor`），见第 1 节。

## 6. 给决策票用的结论

1. **数据管道是一处**：`src/cli/probe.rs:325`。三个方向里有两个要先动它
   （补 `description` 到 Schema 面 / 会话级记录），第三个（只改文档）不动。
2. **表在会话中途不变** —— 方向 2「会话级记录」的可行性前提，而且它成立。
3. **规模底数**：19 条内建 + N 条动态；`description` 合计约 5.9k 字符，最长一条 896 字符；
   参数 schema `pretty` 之后在 **1.5–2 万字符量级**（估，要精确值就跑一次）。
   所以「一次看全表」**不是一页纸**，而是一页多 —— 这一条决定了「画一行还是画 N 行」是真问题。
4. **重放语义**：重放老会话看到的是今天的表。方向 2 若走「不进流的会话级记录」这条通道，
   **它自己得把这句话说出来**（系统提示词那条记录就是这么做的，
   `src/render/wording.rs:1566`）；方向 1 走 per-call 的面，同样得说（那张面查到的是**今天**
   的表，工具名查不到时已经有一句「查不到」了）。
5. **两个 `SessionFacts` 同名**：`src/render/tui.rs:372` 那个带 `tool_schemas`，
   `src/session/facts.rs:18` 那个不带（它是模型 / provider / 思考档 / 权限模式 / 沙箱档五样）。
   「改一下 `SessionFacts` 的形状」这句话有两种读法，票里要写死是哪一种。
