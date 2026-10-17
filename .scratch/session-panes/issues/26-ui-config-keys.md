# 26: `[ui]` 两个新键

Type: implement
Status: ready-for-agent
Part of: ../map.md
Blocked by: 19

**要建的：** `[ui]` 段多两个可配项，写错了拒绝启动。

- **`tile_limit`**：整数 `2..=4`，默认 `4`；集合上限（票 19 的满员回执从此读它）。
- **`sidebar`**：`auto` / `always` / `never`，默认 `auto`；给左栏启动初值（`auto` 的检测在票 27）。
- 非法值（越界、非整数、未知字面量）**一律拒绝启动**，错误消息照 `number_style` 那套写法。

## 验收

- [ ] `tile_limit = 2` 时集合上限 2（第 3 个被拒并提示「最多 2 个」）。
- [ ] `tile_limit = 1` / `5` / 非整数 → 启动失败并指名该项。
- [ ] `sidebar = "never"` 启动时左栏不画；`"always"` 在 herdr 里也画。
- [ ] `sidebar = "sometimes"` → 启动失败并列出合法字面量。
- [ ] config 单测覆盖上述各档，错误消息逐字对齐 `number_style` 的测试写法。

## 依赖

[19 集合生灭与满员拒绝](19-tiles-lifecycle-and-limit.md)（回执要读 `tile_limit`）。决定见
[`issues/11-grilling-limits-config-and-herdr.md`](11-grilling-limits-config-and-herdr.md)。
