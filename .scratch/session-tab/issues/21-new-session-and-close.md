# 21: 新建、关闭与空宿主

Type: implement
Status: in-progress
Blocked by: 20
Part of: ../map.md

**要打通的行为**：会话页页顶的两个入口真的能用 —— `新建` 开一场空会话并切过去（旧的留着），`关闭` 把一场从内存里卸下来（盘上留着）；把最后一场关掉之后，界面进入一个说得清的**空宿主**态。

**来源**：[spec.md](../spec.md) §3、§5；决策票 [`07-new-session-and-unload.md`](07-new-session-and-unload.md)。

## 验收

- [ ] `新建`：同 cwd 再加载一场空会话（空宿主时 cwd = 进程启动 cwd）、**立即成为前台**；继承前台的模型 / effort / 权限模式，沙箱与技能按同 cwd 重探；`carried_tokens` 清 0
- [ ] `新建` = `/clear` = 阈值 rollover 是**同一个动作**；旧的留宿主表里、不写收尾事件
- [ ] `关闭` 四条规则：前台自己 → **禁止**（回执「先切到别的会话再关」）；那一场正在跑回合 → **拒绝**（回执「先在那一场按 ctrl-c 取消」）；空闲会话 → **单击即关、不二次确认**；只读预览会话 → **能关**
- [ ] 关闭 = 从宿主表移除（发 `Close` → await `JoinHandle`）；**不写盘不删盘**
- [ ] **空宿主态**：页顶 `0 场` + 右端只剩 `新建`；页区 `没有会话`；输入区禁用、提交时回 `没有会话，先新建一场`
- [ ] `/clear` 的回执文案改成「新开一场，旧会话留在列表里」的口径
- [ ] 关闭非前台会话时前台不动，只重画会话页并删掉轻状态表里那一行
- [ ] 测试（宿主接缝）：四条关闭规则各一条 + 关掉最后一场进入空宿主、再 `新建` 回来

## 进度（2026-10-12）

**已落地**：

- **`新建`**：`FrontEndEvent::NewSession` → `CliRequest::New` → `open_new` —— 同 cwd
  （`store.create`）再加载一场空会话、**继承当前前台的模型 / 档位 / 权限模式**、入表、切前台、
  发一条 `SwitchSession` 重建，旧的留在表里。回执「新开一场会话；刚才那场还在会话页里」。
- **`/clear` = 页签 `新建`**：`Submission::Clear` 改成发同一条 `CliRequest::New`，
  **`clear_while_looping` 闸门取消**（旧会话不再被换掉，拦它没有理由）。
- **`关闭` 四条规则**（`session::close_check`，库层、可测）：前台自己 → `Foreground`；正在跑
  回合 → `Running`；空闲非前台 → 放行（单击即关、不二次确认）；只读预览 → 放行。
  `close_session` 用 `close_with(Close)` 卸下并 await，**不写盘不删盘**。
- **空宿主态**：`ConsoleRequest::SessionClosed { id, empty }` —— 渲染器删掉轻状态表那一行、
  置 `host_empty`、禁用输入区（`muted`）；`HostCount` 让页顶 `N 场` 读的是**宿主表**的场数
  （票 17 原来读的是 store 条数，一并修正）；提交时循环回「没有会话了：先在会话页按 `新建`
  开一场」。
- 关非前台会话时前台不动（`switch` 没被碰），只重画会话页。
- 测试：`tests/session_host.rs` 新增 2 条（四条关闭规则一条、关掉最后一场进空宿主再新建回来
  一条），共 16 条。

**还没做**：**阈值 rollover 那一条**。`ThresholdStep::Compact` 仍调
`harness.compact_and_rollover(&stored)`（`src/cli.rs` 的 `run_goal_loop` 里）—— 那是「换掉这一
场」的老语义。改成「新建一场」不只换指针：翻页要把**额度**带过去（`carry_usage`）、还要在新
会话里**重新认领目标**（`select_goal`）并重摆清单。这值得单独一轮，所以这一票停在 in-progress。
