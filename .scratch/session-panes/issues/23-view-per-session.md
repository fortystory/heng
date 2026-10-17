# 23: `View` 每会话：投影常驻

Type: implement
Status: done
Part of: ../map.md
Blocked by: 19

**要建的：** 每会话一份投影，切会话不再重造。

- `TuiState.view: View`（单例）→ **按 `SessionId` 的 `View` 表**；`painted`、`block_at`、
  `trace_groups`、`turn_rail`、两个宽度、`colors`、`turn_totals` 全部每会话一份。
- 切会话 = 换读哪一个 `View`，**不再 `View::new` + 整条重放**；`SessionLight` 继续当轻状态。
- 平铺态每个格持有的是**它自己那个会话的投影**，不是同一份的裁剪。

## 验收

- [x] 切走再切回一个会话：屏幕文本一致，且不经过整条重放（以重放次数或既有测试装置断言）。
- [x] 平铺两个会话时，两格各画自己的转录（帧）—— 多格绘制在票 17 的 `draw_tiles` 里落地，
  `render_layout` 的 `the_tiles_draw_each_sessions_own_transcript` 钉住「两格各是各的话」。
- [x] 新建会话只给新会话建一份投影，其余不动。
- [x] 既有切会话测试全绿（外部行为不变）。

## 依赖

[19 集合生灭与满员拒绝](19-tiles-lifecycle-and-limit.md)。决定见
[`issues/10-grilling-per-session-view-state.md`](10-grilling-per-session-view-state.md)。

## 落地注记（2026-10-17）

- **存储层**：`TuiState` 多了一张 `views: HashMap<SessionId, View>`；焦点会话那份仍是
  `TuiState.view`。切会话时把旧的 `mem::replace` 进表、把目标那份取回来；**只有第一次装载**
  才 `View::new` + 重放快照（`view_replays` 计数因此只在首次增长）。
- `SessionLight` 那张轻状态表一字未动：它管的是**不随投影重建**的东西（草稿、浮层、视口
  意图），与常驻投影各司其职。
- 关闭会话时顺手把那张投影移出表（否则关掉的会话还占一份 `CAP = 20_000` 的缓冲）。
- 判据：`pub fn view_replays()` 公开投影新建次数；`tests/session_switch.rs` 两条
  （切回不重建、只有新会话建投影）。
- **多格绘制留给票 17**：平铺要同时画几份投影，而绘制路径仍借用 `TuiState.view` 这一个
  入口；票 17 会把它改成按格取 `views` 里的那一份。
