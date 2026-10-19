# 一次运行的结束：`RunState` 是边沿，询问的撤回归宿主三态

Status: 1/1 done

- **来源**：2026-10-19 维护者报告「heng 在向用户提问时没有显示操作区域」，附会话记录
  `/home/forty/.local/share/heng/sessions/-home-forty-code-fortystory-heng-be4950a197ebe1f7/20261019T181141Z-b58c9df0`。
  那场 `/wayfinder` 里两次 `ask_user_question`（seq 57、77）都在 **75ms / 82ms** 后以
  「这份问卷没被作答就搁下了」失败，人没有见过问卷。
- **与旧决议的关系**：**替换** [`.scratch/fs-agent-v1/issues/31-modal-outlives-the-run-it-belongs-to.md`](../fs-agent-v1/issues/31-modal-outlives-the-run-it-belongs-to.md)
  的实现信号（`RunState { running: false }` 撤在飞询问）。票 31 的**语义**不变：询问属于提出它的
  那次运行，运行结束就是它变馊的时刻。
- **落点**：`src/render/input.rs`（`ConsoleHandle::set_running` 去重）、`src/render/tui.rs`
  （撤回挪到 `apply_host_facts`、`switch_session` 的恢复守卫）、`src/cli/interactive.rs`（注释）、
  `tests/render_console.rs`、`tests/ask_user_question_tui.rs`、`tests/render_tui.rs`、
  `tests/render_layout.rs`、`tests/session_switch.rs`。

## 问题陈述

1. **问卷一出现就被撤下。** 主循环在等输入时每 80ms 被采样分支唤醒一次，每轮顶部都会执行
   `console.set_running(false)` —— 这条「回到等一行」于是成了每 80ms 重复一次的电平。TUI 把它
   读成「一次运行结束」的边沿，清掉 `Pending::Loop | Pending::Questionnaire`，reply sender 被
   丢，`ask_user_question` 读到 oneshot 取消，返回「这份问卷没被作答就搁下了」。人看到的是
   问卷最多亮了 80ms。
2. **同一个缺口对权限询问成立。** `Pending::Loop` 在同一条 `matches!` 里；只是本机常用
   `workspace` 模式自动放行，没暴露。
3. **脉冲被每 80ms 归零。** `RunState` 分支里的 `self.pulse = 0` 也按电平执行，侧栏扫光 /
   月相 / 光标闪烁在每个采样周期被重置回帧 0。

## 根因

票 31 选择 `RunState` 撤询问时的推导是「询问只可能在运行内部发出，所以运行结束 ⇒ 没有活着的
询问」。推导的前提是**运行结束的边沿真的存在**。但主循环在 `Send` 一次提交之后立刻回到等输入
（会话在宿主表的 task 里跑，主循环不 await 它），所以 `RunState` 的 false 表达的是「主循环在不在
处理一次提交」，而不是「会话的那次回合结没结束」。每 80ms 的重复 false 让这个缺口变成必然：
任何在飞询问都会在下一次采样时被撤。

## 决定

### §1 `RunState` 是边沿通知

去重放在发送端（`ConsoleHandle::set_running`）：只在值真的变了时才推。初值 `-1`（没推过），
所以第一次 `set_running(false)` 仍会到达前端。调用点不需要各自记住上一次的值 —— 主循环在
等输入时照旧每轮说一遍「回到等一行」。

### §2 询问的撤回归宿主三态

前台会话从「跑着 / 等人」回到空闲（`HostFacts` 里 `SessionState::Idle`，投影里就是 `None`），
是「提出询问的那次动作收尾」的边沿。撤回在 `TuiState::apply_host_facts` 里做，条件与票 31
相同：只撤 `Pending::Loop | Pending::Questionnaire`，丢掉 sender 而不是替它作答。最多晚一个
80ms 采样周期，与票 31 已接受的窗口同量级。

切走又切回的那个会话走同一条尺：从轻状态恢复询问（`switch_session`）时，目标会话的三态已经
是空闲就说明那次运行在后台结束了 —— 收起来的询问作废，不恢复。仍在跑或仍在等人的会话则原样
带回。

### §3 票 29 的忙闲语义不动

`RunState` 仍由主循环在「取到一行」置真、「回到等一行」置假；`busy()` 仍并上宿主三态。
`console.set_running(false)` 留在 loop 顶部，只是因为去重后它不再制造重复电平。

## 测试决定

- `run_state_is_pushed_only_when_it_changes`（`tests/render_console.rs`）：六次调用只收到
  `false → true → false` 三条，之后 50ms 内不再有第四条。
- `a_repeated_run_state_does_not_withdraw_the_questionnaire`（`tests/ask_user_question_tui.rs`）：
  问卷建立后连推两条 `RunState { running: false }`，题面还在、reply 未关闭。
- `the_run_ending_withdraws_the_questionnaire`（同文件）：`Facts(RunningTurn)` → 问卷 →
  `Facts(Idle)`，题面消失、reply 被丢。
- 既有两条改为走三态边沿：`tests/render_tui.rs` 的
  `a_cancelled_run_takes_its_unanswered_question_with_it`、`tests/render_layout.rs` 的
  `a_cancelled_run_leaves_no_overlay_behind`。
- `a_question_from_a_run_that_ended_in_the_background_does_not_come_back`
  （`tests/session_switch.rs`）：切走时收起的询问，在后台运行结束之后切回来不再出现、reply 被丢
  —— 变异检验（把恢复语句改回 `light.pending`）转红。

## 明确不做

- **不改主循环结构**：不 await 会话回合、不为「运行结束」另造一条命令。
- **不动渲染器自己的那两类确认**（超长粘贴、`Esc` 清空草稿）：它们不是循环的问题，只可能在
  空闲时出现（票 31 已定）。
- **不给 `RunState` 加 id 或作废通道**：宿主三态已经是那条边沿，再加一层只是多一个要维护的
  生命周期。
