# `RunState` 是边沿：去重发送，询问撤回归宿主三态

Type: implement
Status: done
Blocked by: —

> 规格：`.scratch/run-state-edge/spec.md`（§1–§3）。
> 来源：2026-10-19 维护者报告，附会话记录 `20261019T181141Z-b58c9df0`（两次 `ask_user_question`
> 分别在 75ms / 82ms 后失败，界面上没有操作区域）。

## 目标

`ask_user_question` 的问卷在 TUI 里能一直立着，直到用户作答、或者提出它的那次运行真的结束。
同时把 `RunState` 收回它名字里说的那样：一次运行的边沿，而不是每 80ms 一次的电平。

## 具体行为

1. **发送端去重**（`src/render/input.rs`）：`ConsoleHandle` 加一个 `Arc<AtomicI8>`，初值 `-1`
   （没推过）；`set_running` 只在值真的变了时才推 `RunState`。主循环在等输入时每轮照旧调用
   `console.set_running(false)`（`src/cli/interactive.rs` 的 loop 顶部注释同步说明）。
2. **撤回挪走**（`src/render/tui.rs`）：`ConsoleNotify::RunState` 分支只保留 `running` / `pulse` /
   `running_tool`；在飞询问的撤回搬到 `apply_host_facts`，判据是**前台会话**从非空闲（投影里
   `Some`）变空闲（`None`），条件仍是 `Pending::Loop | Pending::Questionnaire`，丢掉 sender。
3. **票 31 的语义保留**：运行结束（正常收尾或取消）时宿主三态的 `RunningTurn → Idle` 边沿仍在
   同一个采样周期内到达，所以「取消后权限模态留在屏上」不会回归。
4. **测试**：新增三条（见 spec 的「测试决定」），两条既有测试改用 `Facts(Idle)` 表达运行结束。
5. **切回已经收尾的会话**（`TuiState::switch_session`）：从轻状态恢复询问时先看目标会话的三态，
   已是空闲就丢掉（drop sender），不让一个后台结束的运行把陈旧模态带回来 —— 修复前它靠
   `RunState` 的每 80ms 电平碰巧被撤掉，信号换掉之后必须在恢复处补上这把尺。

## 落地记录

- `src/render/input.rs`：`ConsoleHandle.running: Arc<AtomicI8>`、`set_running` 去重、`console()`
  初始化 `-1`；注释写清为什么去重放在发送端。
- `src/render/tui.rs`：`ConsoleNotify::RunState` 只做忙闲与脉冲；`apply_host_facts` 记录更新前的
  `front_state()` 并在末尾比较；`switch_session` 恢复询问时按目标会话三态决定要不要带回来；
  新增 `TuiState::front_state()` 辅助。
- `src/cli/interactive.rs`：loop 顶部 `set_running(false)` 的注释指向 `run-state-edge` 票 01。
- `tests/render_console.rs`：`run_state_is_pushed_only_when_it_changes`。
- `tests/ask_user_question_tui.rs`：`a_repeated_run_state_does_not_withdraw_the_questionnaire`、
  `the_run_ending_withdraws_the_questionnaire`。
- `tests/render_tui.rs` / `tests/render_layout.rs`：两条既有用例的结束动作改成 `Facts(Idle)`。
- `tests/session_switch.rs`：`a_question_from_a_run_that_ended_in_the_background_does_not_come_back`。
- `docs/tui-manual-checklist.md`：㉕ 加第 12 条（问卷立得住）、⑯ 第 9 条补一句这次修的归零时机。
- `.scratch/fs-agent-v1/issues/31-modal-outlives-the-run-it-belongs-to.md`：追加补记，指明实现
  信号已由本票替换。

## 验证

`cargo test --all-targets`、`cargo clippy --all-targets`、`cargo fmt --check` 与两道文档护栏的
结果记在提交信息的 `验证：` 一行。

- `cargo test --all-targets` **1835 passed / 0 failed**；`cargo clippy --all-targets` 无警告；
  `cargo fmt --check` 干净；`scripts/check-doc-size.py` / `scripts/check-language.py` 绿；
  `python3 -m unittest discover -s scripts/tests` 37 tests OK。
- `scripts/tui-startup-check.py` 在本提交上 4/6 red，但用 `HEAD~1`（6689656）另编一个二进制
  逐条对比后**完全相同**：提示行新增 `ctrl-v` / `ctrl-r` / `ctrl-o` 之后，脚本的 `STATUS_ANCHOR`
  会匹配到提示行而不是状态行；suspend 那一项报横幅到达两次。都是既有问题，与这次的状态信号
  改动无关。
- 变异检验：把 `switch_session` 的恢复守卫改回 `light.pending`，
  `a_question_from_a_run_that_ended_in_the_background_does_not_come_back` 转红。
