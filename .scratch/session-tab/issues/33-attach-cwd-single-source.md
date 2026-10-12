# 33: 装载的 cwd 只有一个来源

Type: implement
Status: done
Blocked by: —
Part of: ../map.md

**要打通的行为**：从会话页点开一条盘上的会话时，**入表记下的 cwd** 与**装载用的 cwd** 必须是同一个值，而且只能来自流上记着的那个工作区。今天这两个值在同一个函数里分叉：`open_session` 用 `titles::read_prefix` 读出真 cwd 交给 `crate::load`，却把 `StoredSession.dir`（会话目录，`src/session/store.rs:64`）交给 `host.adopt_with`（`src/cli.rs:2178`）。而 `HostEntry.cwd` 正是「新建 / 翻页继承哪个 cwd」的唯一来源（`src/cli.rs:735`、`:749`）—— 于是**在装载来的会话上按新建或翻页，新会话的 cwd 会是会话目录**。

**来源**：2026-10-12 一轮架构访谈（`/improve-codebase-architecture` 的探索加 Q1–Q26 的 grilling）。它把票 [18](18-attach-path.md) 的「一条共用的装载函数」拆成两票：本票先让 cwd 只有一个来源，[票 34](34-attach-one-action.md) 再把装载收成一个动作。

## 冻结的验收

- [x] 新增一处 cwd 的来源函数（落在 `cli.rs` 的装载区，名字 `attach_cwd`）：只认 `SessionStarted.cwd`，读不出时**拒装**
- [x] 装载路径不再把 `stored.dir` 当 cwd：`attach::check` 的 `cwd` 参数与 `adopt_with` 的 cwd 参数取同一个值
- [x] 读不出 cwd 不静默回退会话目录；回执一句话（`wording::session_cwd_missing`）
- [x] `titles::read_prefix` 不再被装载路径用来取 cwd（列表页那条前缀轻路径照旧）
- [x] 测试：`attach_cwd` 在流上有 `SessionStarted.cwd` 时给流上那个、空串时给 `None`（入表那一步的端到端留给票 34 的计划值 —— 那一层今天还不存在）
- [x] 不动的：`-c <id>` 启动路径的组装、只读预览那两档、`store::session_cwd` 的签名

## 说明

**为什么不是 `read_prefix`**：它读流的头 8 行（`src/session/titles.rs:33`），服务的是列表页那条轻路径；装载路径要的是「这个会话的工作区」这一条事实，它的家在 `started_cwd`（`src/session/store.rs:38`），`sessions` 子命令也是这么读的（`src/cli.rs:4273`）。

**为什么不上新类型**（`SessionDir` / `WorkCwd`）：`HostEntry.cwd` 的消费者只有新建与翻页两处（`src/cli.rs:737`、`:749`），而新类型会渗进 `SessionFacts.session_dir`、`observe`、`listing`、`prune` 十几处；本票用一条测试钉住，以后再漂再上类型。

## 进度

**2026-10-12 落地**（两刀 TDD）：

- `attach_cwd(&[Event]) -> Option<PathBuf>`（`src/cli.rs`，`open_session` 之前）：读 `store::started_cwd` 并滤掉空串 —— 空串与读不出都是 `None`。两条单元测试：`cli::tests::attaching_asks_the_stream_for_the_workspace`、`cli::tests::an_empty_recorded_workspace_is_no_workspace`。
- **与票面的一处偏差**：验收写的是 `store::session_cwd(&StoredSession)`，落地时改成吃**已读到的 `events`**（`open_session` 本来就先读一次流）—— 同一个事实、少一次读盘，而 `started_cwd` 正是 `session_cwd` 内部调的那个。
- 接线：`open_session` 的 cwd 来自 `attach_cwd`；`attach::check` 拿 `cwd.as_deref()`、`adopt_with` 拿同一个变量（`cwd` 在 load 前 clone 一次）；`None` 时在 check 那三档都放行之后回一句「这条会话没记下工作目录，装不起来」并拒装。`read_prefix` 从装载路径消失。
- `cargo test` 全绿：0 failed（lib 231 条，其余各集成 crate 同样全过）。
