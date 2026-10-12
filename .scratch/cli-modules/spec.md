# `cli.rs` 拆成 module：一条子命令一个文件

Status: done（2026-10-12 由一轮架构回顾的候选 5 折成，维护者选定；没有拆票，spec 自己落地）

`src/cli.rs` 7026 行，五条子命令互不共享状态却同处一个编译单元：进程入口与分派、`interactive`
主循环、`discuss`、`probe` / `prune`、`sessions` 各占一段，被「命令行入口」这一件事粘在一起。
它对外只暴露 3 个 pub 符号（`run_sessions`、`root_refusal`、`finish_session` —— `tests/` 只用到
这 3 个），于是 7000 行里能穿测试的 seam 只有两条：内联 `mod tests`（1902 行，够得着的只是为可测
抽出来的纯函数）与那 3 个 pub。四个条目的体量本身就是证据：`effective_mode` 620 行、`interactive`
612 行、`parse_discuss` 297 行、`CliActor` 的 impl 268 行。

来源：2026-10-12 一轮架构回顾（`/improve-codebase-architecture` 的报告候选 5，维护者选定）。
`lib.rs` 顶部那张「十六个顶层模块，只向下依赖」的表里 `cli` 是最后一个；本 spec 把它的**内部**
照同一张表的精神排一遍。

**范围**：这一步只做**边界与落位**（哪些条目住哪个文件、谁看得见谁）。主循环的 interface
重设计不在内 —— 那是报告里另半句话（「喂一条输入或一条前端事件，收一次渲染副作用」），
本 spec 一个字不动它的行为。

## §1 目标形状

`src/cli.rs` → `src/cli/` 目录，一条子命令一个文件：

| 文件 | 收什么（`cli.rs` 里那些条目） |
| --- | --- |
| `cli/mod.rs` | 模块表、`main()`、子命令分派、`root_refusal`、`finish_session` |
| `cli/interactive.rs` | `interactive()` 主循环、`InteractiveArgs` / `parse_interactive` / `effective_mode`、`CliRequest` / `CliCommand` / `CliActor` / `front_to_request` / `forward_gesture`、`Submission` 一族、`/` 命令解析、模型与档位切换一族、`ExitRequest` / `exit_code_after` / `TurnStart` |
| `cli/loading.rs` | 票 34 的装载那一族（`AttachTarget` / `AttachPlan` / `attach` / `open_new` / `open_session` / `close_session`）、`AssemblyKit`、`effort_from_token` |
| `cli/goals.rs` | `GoalSetup` 一族（`goal_progress` / `goal_usage` / `goal_streams` / `run_goal_command` / `parse_goal_new_line` / `parse_loop_line` / `report_unknown_goal_ids` / `stop_goal_loop` / `Muted`） |
| `cli/discuss.rs` | `DiscussArgs` / `parse_discuss` / `parse_discuss_line` / `split_names` / `pick_debaters` / `advisory_lines` / `discussion_seed` / `discussion_participants` / `question` / `undo_receipt` / `retarget_to` / `Retargeted` / 选择器候选 |
| `cli/probe.rs` | `probe_prompt` / `ProbeArgs` / `parse_probe` / `ProbeError` / `PruneArgs` / `parse_prune` / `prune` / `load_config` / `web_service` / `tool_schemas` / `mcp_sandbox` / `probe_dir` / `last_usage` |
| `cli/sessions.rs` | `SessionsArgs` 一族、`run_sessions` / `choose_session` / `find_session` / `session_at` / `sessions_ls` / `sessions_title` / `sessions_show` / `sessions_replay` / `sessions_stats` / `render_entry` / `print_*` / `filter_changes` |
| `cli/tests.rs` | 现有 `#[cfg(test)] mod tests`（1902 行） |

`mod.rs` 只放：模块 doc、`mod` 声明、`main()` 与它的分派、以及被 `tests/` 用到的那几个 pub。
它不 re-export 子模块 —— 边界要真，代价是子模块之间用 `super::interactive::X` 这样的显式路径。

## §2 可见性与共享件

- 跨模块要用的项加 `pub(crate)`（`CliCommand`、`CliRequest`、`AssemblyKit`、`GoalSetup`、
  `ExitRequest`、`Submission`…），只在同目录内用得到的加 `pub(super)`。
- `tests/` 只认 `run_sessions`、`root_refusal`、`finish_session`（现状）—— 这三个保持 `pub`。
- 内联测试搬进 `cli/tests.rs`：它是 `cli` 的子模块，`use super::*` 加需要的那几个
  `use super::interactive::…`。测试**内容一条不改**（只改可见性与路径），它的通过数就是这次
  拆分的验收。

## §3 不动的

- **行为一个字不动**：这次只搬代码与改可见性，不改任何一条逻辑、文案与键位。
- 启动检查 `scripts/tui-startup-check.py`、`docs/tui-manual-checklist.md` 里引用的路径不改
  （它们看的是 `heng` 命令，不是文件布局）。
- `lib.rs` 的模块表只改一行：`pub mod cli;` 指向 `src/cli/mod.rs`。
- 不顺手改 `effective_mode` 那 620 行、不拆 `interactive()` 里那段（那是报告里另半句话）。

## 测试决定

现有 1850 条必须全绿，数字与拆分前**逐条相同**。拆分的验收就是这件事：没有新测试，也没有
测试被删。拆完补一条结构性检查的话，也只值一条 —— 本 spec 不新增。

## 落地记录（2026-10-12）

- **文件**：`src/cli/mod.rs`（143 行，入口与分派）、`interactive.rs`（2266）、`loading.rs`（808）、
  `sessions.rs`（909）、`discuss.rs`（643）、`probe.rs`（506）、`goals.rs`（217）、`tests.rs`（1911）——
  原来是 `src/cli.rs` 的 7026 行。
- **可见性**：跨模块要用的顶层项 `pub(crate)`，struct 字段同档（`AssemblyKit`、`CliActor`、
  `InteractiveArgs` 那几族）；`ExitRequest::code` / `AssemblyKit::notice` / `Muted::new` /
  `McpPromptEntry::new` 这几个方法也放开。**唯一对外保留的 pub 路径**是
  `heng::cli::{run_sessions, root_refusal, finish_session}`（`tests/` 用到的那三个，
  `run_sessions` 由 `mod.rs` 显式 `pub use`）。
- **子模块之间怎么看见对方**：每个文件顶部 `use super::*;` 加一组 `use super::<兄弟>::*;` ——
  显式列出依赖了谁，但**不是 re-export**（`cli` 的对外面没有多出东西）。编译期删掉了 26 条用不到的
  glob，剩下的那些就是真实依赖。
- 行为一个字没动：`cargo test` **1850 passed / 0 failed**，与拆分前逐条相同；`cargo fmt --check`
  干净、`cargo check --all-targets` 零警告。
