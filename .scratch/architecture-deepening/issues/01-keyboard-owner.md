# 键盘归属收到一处

Type: task
Status: ready-for-agent
Part of: ../map.md
Blocked by: —

## 背景

[report-2026-10-18.md](../report-2026-10-18.md) **首选**候选。`src/render/tui.rs` 的 `key()`（L7593–L8599，1006 行）是 11 段早期 return；`trace_keyboard` 的注释（`tui.rs:2639`）自陈「仓库里**第四套**「键盘在谁那里」的机制」—— 之前的左栏布尔、两页焦点索引、问卷反显还没退场。`click_at` / `wheel_at` / `keyboard_in_the_input`（`tui.rs:15133`）各持一份字段子集：同一个 keyboard 归属焦点三处拼。

## 想要的形态

seam 放在「层」。每层自己声明 `owns_keyboard()` 与 `handle_key()`，`sync_keyboard_owner()` 一处算。`click_at` / `wheel_at` 走同一张 dispatch 表。先例：[ADR 0010](../../docs/adr/0010-questionnaire-keys-dispatch-by-zone.md)（问卷的「按区分发」）。

## 候选接口草图

```rust
// seam 放在「层」：每个持键盘的视图自己声明
trait KeyboardLayer {
    fn owns_keyboard(&self) -> bool;
    fn handle_key(&mut self, key: Key) -> KeyOutcome;
}

enum KeyOutcome {
    Handled,
    NotMine,         // 落到下一层
    BubbleUp,        // 上交主循环
}
```

## 验收

1. `key()` 不再写 11 段早期 return；改 `for layer in layers.iter().rev() { if layer.owns_keyboard() ... }`。
2. `click_at` / `wheel_at` 内部走同一张 dispatch 表（不是再开一份「持指针者」分支）。
3. 加单元测试：`TuiState` 上不出现新的 `*_keyboard` 字段；`KeyOutcome` 三种落点各有一个测试。
4. 与 [ADR 0010](../../docs/adr/0010-questionnaire-keys-dispatch-by-zone.md) 形状对齐（评审时拿两段对照）。

## 关联

- 来源：[report-2026-10-18.md §首选 · 把键盘归属收到一处](../report-2026-10-18.md)。
- 先例：[ADR 0010](../../docs/adr/0010-questionnaire-keys-dispatch-by-zone.md)。
- HTML：[report-2026-10-18.html 第 1 张卡片](../report-2026-10-18.html)（before / after Mermaid）。

## 评论

- 2026-10-18：从 [.scratch/architecture-deepening/seed.md](../seed.md) 「也升级了——成为四套机制」分支