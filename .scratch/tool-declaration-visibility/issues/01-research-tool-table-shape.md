# 工具表这份账：形状、规模与它今天露过面的地方

Type: research
Status: resolved
Part of: ../map.md
Blocked by: —

## 问题

请求的三样里 `tools` 那一半没有通道。补通道之前得先知道**那份账今天是什么样**：

1. **表是什么、由谁建**：装配链、注册点、会不会在会话中途变。
2. **规模**：多少条声明、每条的 `description` 与参数 schema 各多长 —— 这一条决定「画一行还是
   画 N 行」是不是真问题。
3. **今天抄给渲染层的是哪一部分**：抄取点在哪、抄本叫什么、读者有谁、字段的注释怎么写的。
4. **今天这份账在界面上露过面的地方**：全部列出来 —— 因为「唯一露头处」这个判断必须被核对过。
5. **什么时候变**：重放老会话看到的是哪张表。

**这是只读调研，不改任何代码。** 结论一律给 `文件:行号`。

## 产物

[`../research/01-tool-table-shape.md`](../research/01-tool-table-shape.md)。写完在票底给
`## 作答`。

## 作答

**表在组装期建好，此后只读** —— 全仓 `Registry::register` 的调用点只在 `src/tools/mod.rs` 里，
**没有运行时注册**；文档里同一条不变式见 `docs/lifecycle.md:488`（「工具表组装后不变」）。
装配链三处逐字同形（`src/cli/interactive.rs:223-229`、`src/cli/discuss.rs:180-186`、
`src/cli/loading.rs:121-127`）。**执行者那张是子集**（`Registry::for_executor`，
`src/tools/registry.rs:70-78`），所以主会话那张覆盖得了会话里出现的每一次调用。

**抄给渲染层的那本账只抄了 `parameters`** —— 抄取点只有一处（`src/cli/probe.rs:325-331`），
抄本 `tool_schemas` 是 `BTreeMap<String, serde_json::Value>`（`src/render/tui.rs:416`），
读者只有渲染层内部两处（`:8926`、`:15711`）。**字段自己的注释已经把 `parameters` 说成
「声明」**（`src/render/tui.rs:407-408`）—— `docs/render.md:544` 那句措辞失准就是它的下游。

**规模**：**19 条内建声明**（`src/tools/*.rs` 的 `spec()`），`description` 合计 **5881 字符**，
最长一条 `ask_user_question` **896 字符**、其次 `bash` 856；参数 schema 的源码字面量合计约
**11600 字符**（`pretty` 之后更大，精确值要跑一次才知道）。动态工具条数随配置而变、
**不受代码控制**；MCP 侧只有四个元工具，**server 自己的工具永远不进表**。

**今天露过面的地方只有一处**：工具调用详情覆盖层的 `Schema` 面（`FaceId::Schema`，
`src/render/tui.rs:14500-14502`，工具那六面的第三面）。它画的是**参数 schema**，不是完整声明
—— provider 收到的是 `name` + `description` + `parameters` 三件套
（`src/provider/openai.rs:441-455`）。`sessions show` 不打工具表，左栏各页不画它，
plain 与 headless 没有面这套东西。**`description` 在界面上一个字都读不到。**

**重放老会话看到的是今天的表**：装载那条路会照当前配置**重新建一张**
（`src/cli/loading.rs:121-139`），不是当时那张。

产物：[`../research/01-tool-table-shape.md`](../research/01-tool-table-shape.md)。
