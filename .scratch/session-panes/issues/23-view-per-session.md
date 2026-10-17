# 23: `View` 每会话：投影常驻

Type: implement
Status: ready-for-agent
Part of: ../map.md
Blocked by: 19

**要建的：** 每会话一份投影，切会话不再重造。

- `TuiState.view: View`（单例）→ **按 `SessionId` 的 `View` 表**；`painted`、`block_at`、
  `trace_groups`、`turn_rail`、两个宽度、`colors`、`turn_totals` 全部每会话一份。
- 切会话 = 换读哪一个 `View`，**不再 `View::new` + 整条重放**；`SessionLight` 继续当轻状态。
- 平铺态每个格持有的是**它自己那个会话的投影**，不是同一份的裁剪。

## 验收

- [ ] 切走再切回一个会话：屏幕文本一致，且不经过整条重放（以重放次数或既有测试装置断言）。
- [ ] 平铺两个会话时，两格各画自己的转录（帧）。
- [ ] 新建会话只给新会话建一份投影，其余不动。
- [ ] 既有切会话测试全绿（外部行为不变）。

## 依赖

[19 集合生灭与满员拒绝](19-tiles-lifecycle-and-limit.md)。决定见
[`issues/10-grilling-per-session-view-state.md`](10-grilling-per-session-view-state.md)。
