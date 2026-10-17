# 22: 详情覆盖层与拖选

Type: implement
Status: ready-for-agent
Part of: ../map.md
Blocked by: 19

**要建的：** 浮层的归属在平铺之后仍然清楚。

- `detail_opener` / `detail_rect` / `detail_serial` 从单例**搬进每会话**（今天只有 `detail` 进了
  `SessionLight`，切回旧会话再关可能按错档还原）。
- 覆盖层照旧**整屏居中**（宽度对着整屏量，不缩进格 / 不缩进右栏）；**已开着的覆盖层不因切焦点而
  换归属**；关掉时**按打开方还原**（右栏开的还原给右栏，格内开的还原给那个格）。
- **拖选**仍夹在按下那一刻所属的文本区域里，不跨格；OSC 52 与终端原生选择照旧。

## 验收

- [ ] 右栏里点开一条详情，关掉后滚动位置还原给右栏（pty）。
- [ ] 格内点开的详情在切焦点之后仍属于原来那个格；关掉时还原给那个格。
- [ ] 覆盖层宽度不随格宽变（帧）。
- [ ] 拖选不跨越格边界（单元 / pty）。
- [ ] 既有详情覆盖层测试全绿。

## 依赖

[19 集合生灭与满员拒绝](19-tiles-lifecycle-and-limit.md)。决定见
[`issues/06-grilling-single-instance-surfaces.md`](06-grilling-single-instance-surfaces.md)。
