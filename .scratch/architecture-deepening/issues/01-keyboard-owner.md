# 键盘归属收到一处

Type: task
Status: claimed
Claimed-by: wayfinder session 2026-10-18 (after 04 resolved)
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

## 勘探备忘（2026-10-18，wayfinder 会话、claim 后）

落地的已知工作量（探到 impl 之前的判据，未动代码）：

- **`key()` 11 段早期 return**（`src/render/tui.rs:7593-`）按现在的顺序：CtrlZ（suspend）/ Esc+drag / replay / file_viewer / picker / detail / sidebar_keyboard / trace_keyboard / CtrlT / CtrlLeft-Right / CtrlDigit / CtrlR / CtrlO / CtrlV / 清举手 + CtrlC-D 走 exit_key / Esc 末端。**不是 11，是约 16 个 early-return 点** —— 其中 CtrlR/CtrlO/CtrlV/CtrlT/CtrlLeft/CtrlDigit 与「层持有」无关，本质是同一档全局手势。
- **`trace_keyboard` 字段读点 28 处全在 `src/render/tui.rs`**（字段 1 处、helper 2 处、读 11 处、测试 14 处）；其他模块无引用。`sidebar_keyboard` 也是同样 28 处全在本文件。
- **`keyboard_in_the_input`**（`:15133`）的反向判据与 `key()` 11 段**不严格同源**（差 `right_wanted && trace_keyboard`、差 `pending` 处理）。这两套"键盘在哪一层"判据从同一状态派生，是 seam 必须合并的源头——合并点要单立一处。
- **`click_at` / `wheel_at`**（`:4957/5024`）各按"指针落在哪儿 → take_input_keyboard / take_sidebar_keyboard / take_trace_keyboard / release_trace_keyboard"分派，是与 `key()` 平行的"另一张 dispatch 表"——票面要求两张表共用，落地时**先**抽一个 dispatch 物件，让 `key()` 与 `click_at` / `wheel_at` 都从它读。
- **`trace_keyboard` 字段名保留**（既有 14 处测试直读）—— `TuiState::trace_keyboard: bool` 保留，但其值改成"派生自 dispatch 状态"，不直接被任何 `take_*` 写。配套做法：内部方法 `refresh_keyboard_owner()`（或一层），从 seats 算并设这一位。
- **`Picker` / `Detail` / `FileViewer` / `Replay`** 各是已经存在的"已按层持有"的实体——它们内部已有自己的 `*_key` 方法；seam 不重写它们，但要给它们包一个 `KeyboardLayer` adapter。
- **`CtrlZ` 与 `Esc+drag`**：已经在 `key()` 的最前两条早返回，被 map 冻结项 6/7 写在「seam 外」、不进 dispatch。seam 可以保留这两条作为 `key()` 的开头（与 `note_attention_seen` / `sync_tokens` 同层），中间是 dispatch。
- **`KeyOutcome { Handled, NotMine, BubbleUp }` 三态** —— 现有 16 处每处要么 return、要么"被它不动地落到下面"：

  - `Handled`：replay / file_viewer / picker 整块吃掉；CtrlZ 副作用（seam 外的硬控）；detail / sidebar / trace 命中自己那一组键。
  - `NotMine`：detail / sidebar / trace 落下的未命中的键（漏给下一层）。
  - `BubbleUp`：**主循环挂在 `key()` 上**（`-> ()`）；"上交"语义当前只在 `Ctrl-C` / `Ctrl-D`（走 exit_key）那一档出现，已是 dispatch 之外的另两条。
  - 输入区那一档（pending、questionnaire、muted、token_menu、editor）原本是 `key()` 末段的 match 阶梯，**是 dispatcher 之下的"输入区内部"**——seam 改 `key()` 之后，把这一段挪到 `InputLayer::handle_key` 之内。

- **提案**：`KeyboardLayer` trait（不是 enum）—— 加新覆盖层时只多一个 impl；本仓库覆盖层数有限、trait 单态化余地足够。先做 7 个 layer：`DragSuspend`（CtrlZ + Esc+drag 一起包） / `Modal`（replay / file_viewer / picker 三块同质，但每块自己一个 layer） / `Detail` / `Sidebar` / `Trace` / `Input` 共 7。先写能跑通票面验收 1、2 的最小 seam，把 `keyboard_in_the_input` 改成从 dispatch 派生，加验收 3 的 3 条单测。

落地需决策的两件事：

1. **`picker` 与 `Sidebar` 的关系**：票面验收 2 说"走同一张 dispatch 表（不是再开一份『持指针者』分支）"。`click_at` 命中 `picker` 那一档时是 `Picker::click_at` 自己处理、不进 `take_*` 那一支——seam 把它并入 dispatch 时，picker 这一层既出现在 `key()` 表，也出现在 `click_at` 表，共用一层。
2. **三态的语义**：参考 04 落地，把 `KeyOutcome` 与两侧的"按列 / 按字符"那种"两把尺子"并列——seam 自己用一处的判据，三种落点各一根尺子可换。本会话留、下个会话做决策。

下一步会话要做的：

1. `src/render/tui.rs:7593-` 11 段改写成 dispatch 迭代；保留 `CtrlZ` 与 `Esc+drag` 两条 seam 外早返回。
2. 把 `keyboard_in_the_input` 反向判据改为从 layer 集合派生。
3. `take_trace_keyboard` / `release_trace_keyboard` / `take_sidebar_keyboard` / `release_sidebar_keyboard` / `take_input_keyboard` 改成"改 seats → dispatch 自动重算"。
4. `click_at` / `wheel_at` 内部不再自取 `take_*`，改成读同一张 dispatch 表（按指针落点读对应层）。
5. 加 seam 自身的 3 条单测：`Handled` / `NotMine` / `BubbleUp` 三态各一条。
6. 既有 14 处 `page.trace_keyboard` 直读直写的测试照旧（`TuiState::trace_keyboard: bool` 保留、由 dispatch 派生）。

## 评论

- 2026-10-18：从 [.scratch/architecture-deepening/seed.md](../seed.md) 「也升级了——成为四套机制」分支