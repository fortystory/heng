# host → cli → tui 的事实投影只算一处

Type: task
Status: ready-for-agent
Part of: ../map.md
Blocked by: —

## 背景

[report-2026-10-18.md](../report-2026-10-18.md) **Worth exploring**（10-18 重写）。「宿主表现在长什么样」算成三处 ——

- `cli/interactive.rs:572` 的 80ms 轮询算上一帧快照与边沿判定；
- `ConsoleHandle`（`src/render/input.rs:311`）12+ 个 `pub fn` 推命令；
- `ConsoleRequest` 22 变体；`TuiState` 散落 `HashMap` 或字段承接。

10-12 的 seed.md 那条「宿主表的投影只算一处」升级而成。渲染层要认的领域事实从 6 种涨到 10+ 种，每种一条 push 命令。三条规则（未读的寿命、`N` 只算后台、单会话标题不变）只能靠真机走查（[tui-manual-checklist.md](../../docs/tui-manual-checklist.md)）。

## 想要的形态

seam 放在「host 视图」。一个 `HostFacts` module 算「宿主表现在长什么样」，`cli` 与 `tui` 都订阅它的 diff。12+ 个 `ConsoleRequest` 收成两条「现在」、「事件」。80ms 轮询降到一处。

## 验收

1. `HostFacts` module 单元测试钉死三条规则（未读寿命、`N` 只算后台、单会话标题不变）。
2. `ConsoleRequest` 变体从 22 个收成 ≤ 5 个。
3. `cli/interactive.rs` 的 80ms 轮询合并到一处（不再散在两个文件）。

## 关联

- 来源：[report-2026-10-18.md §host → cli → tui 的事实投影只算一处](../report-2026-10-18.md)。
- HTML：[report-2026-10-18.html 第 4 张卡片](../report-2026-10-18.html)。
- 真机清单：[docs/tui-manual-checklist.md](../../docs/tui-manual-checklist.md) ㊶。

## 注意

与「单会话」默认行为耦合：把订阅改写前，先在 spec 钉死「宿主表空、单会话、多会话」三态的读数。

## 评论

- 2026-10-18：从 [.scratch/architecture-deepening/seed.md](../seed.md) 「事实投影只算一处（10-18 重写）」分支