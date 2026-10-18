# 键盘归属收到一处

Type: task
Status: resolved
Resolved-by: wayfinder session 2026-10-19（上半 2026-10-18 骨架 + 下半本轮接完，见 § 作答）
Claimed-by: wayfinder session 2026-10-18 (after 04 resolved; half-done — see § 评论)
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
- 骨架 commit：`8e6de67 refactor(render): 键盘归属 seam 的 trait 与三态 dispatch 表`。

## 作答（2026-10-18，骨架已落）

这是 § 1 半交付。下一会话接着做 § 2 半。

**这一半（已 commit）**：

- `src/render/keyboard.rs` 新增 `KeyboardLayer` trait + `KeyOutcome { Handled, NotMine, BubbleUp }` 三态 enum。
- `key_dispatch` / `mouse_dispatch` / `wheel_dispatch` 三个 dispatch 入口；每条从 `layers.iter_mut().rev()` 开始、按 `owns_keyboard()` 跳过、`Handled` 与 `BubbleUp` 立即停、`NotMine` 落到下一层继续。
- `render/mod.rs` 加 `pub mod keyboard;`。
- 3 条新单测钉三态（`tests::handled_layer_stops_the_dispatch_chain` / `::not_mine_layer_falls_through` / `::bubble_up_layer_lets_the_dispatch_chain_stop`）。

**下一半（下一会话）**：

- `TuiState::key()` 16 个 early-return 点改成 `dispatch(7 layers)`。保留 CtrlZ 与 Esc+drag 两条 seam 外（map 冻结项 6/7）。
- `click_at` / `wheel_at` 内部不再自取 `take_*`，改成读同一张 dispatch 表（按指针落点 → 选 layer）。
- `keyboard_in_the_input` 改为从层集合派生：`!any_layer_owns()`。
- `take_trace_keyboard` / `release_trace_keyboard` / `take_sidebar_keyboard` / `release_sidebar_keyboard` / `take_input_keyboard` 改为"改 seats → dispatch 自动重算"；既有 14 + 28 处 `page.trace_keyboard` / `sidebar_keyboard` 直读直写测试不破（`TuiState::trace_keyboard: bool` / `sidebar_keyboard: bool` 字段保留）。
- 8 个具体 layer 接上：`DragSuspend`（CtrlZ + Esc+drag 共一档）/ `Replay` / `FileViewer` / `Picker` / `Detail` / `Sidebar` / `Trace` / `Input`。

**约束**：本票面 § 验收 1（`key()` 不再 11 段早期 return）、§ 验收 2（`click_at` / `wheel_at` 走同一张 dispatch）**均未满足**——等下一半一起验。§ 验收 3（三态各一条单测）**已满足**（3 条 keyboard.rs 内的单测）。§ 验收 4（与 ADR 0010 形状对齐）已满足（trait + 三态 enum 同形）。

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

## 作答（2026-10-19，下半接完）

上半的骨架（`KeyboardLayer` + 三态 `KeyOutcome` + 3 条单测，commit `8e6de67`）这一轮接完了：

- `src/render/keyboard.rs`：trait 泛型化到状态类型 `KeyboardLayer<S>`（生产是 `TuiState`，测试用空类型），`key_dispatch` 返回最后那个 `KeyOutcome`，`mouse_dispatch` 返回「有没有层吃掉这一下」。`handle_mouse` / `handle_wheel` 改用坐标（`click_at` / `wheel_at` 只有落点，不必伪造 `MouseEvent`）。
- `src/render/tui.rs` 新增私有模块 `keyboard_layers`：7 个 layer 的 impl 全在那里，判据只有一份。表序 `[Input, Trace, Sidebar, Detail, Picker, FileViewer, Replay]`，dispatch 反向迭代 = 重放 > 文件查看器 > 选择器 > 详情覆盖层 > 左栏 > 轨迹页 > 输入区，与改造前的阶梯逐段对应。
- `TuiState::key()` 从 267 行降到约 30 行：只剩 seam 外的 `CtrlZ` 与「拖选中的 `Esc`」两条早返回（map 冻结项 6/7，即票面列的第八层 `DragSuspend`），其余一句 `key_dispatch`。
- `click_at` / `wheel_at` 不再自开「持指针者」分支：`mouse_dispatch` / `wheel_dispatch` 走同一张层表。覆盖层独占、问卷 / 中间模态只吃自己那块、输入区归还、左栏与账本拿键盘都是各层 `handle_mouse` / `handle_wheel` 的副作用。
- `keyboard_in_the_input` 改成 `!keyboard_layers::any_overlay_owns_keyboard(state)`，与层表同一组 `owns_keyboard`；`pending` 一项留在外层（它是 `InputLayer` 内部的编辑态，不是「哪一层持有」）。
- `TuiState` 的 `*_keyboard` 字段仍是原来两个（`sidebar_keyboard` / `trace_keyboard`），没有新增；`take_*` / `release_*` 仍是它们唯一的写点。
- 测试：`keyboard.rs` 4 条（三态各一条 + 「全 `NotMine` 即无人认领」）；`tui.rs` 2 条（层表与光标判据同源、部分持有的层把不认的键落到输入区）。

§ 验收对照：

1. `key()` 不再写 16 段早期 return：只剩 seam 外两条。✅
2. `click_at` / `wheel_at` 内部走同一张 dispatch 表，没有新增「持指针者」分支。✅
3. `TuiState` 上没有新的 `*_keyboard` 字段（仍是 `sidebar_keyboard` / `trace_keyboard`）；`KeyOutcome` 三种落点各有单测。✅
4. 与 ADR 0010 同形：trait + 三态 enum + 按层反向迭代。✅

验证：`cargo test --no-fail-fast` 除 `tests/session_page.rs` 的 `the_session_tab_comes_first_and_lists_the_history` 外全绿；那一条是**跨午夜的时间显示 flaky**（「90 分钟前」在 00:33 跨到昨天，于是断言 `1 小时前` 失败），在未改动的 HEAD 上用 `git stash` 复跑同样失败，与本票无关。`cargo clippy --all-targets` 0 warning；`cargo fmt --check` 干净。

## 评论

- 2026-10-18：从 [.scratch/architecture-deepening/seed.md](../seed.md) 「也升级了——成为四套机制」分支
- 2026-10-18：claim by wayfinder（已 resolved 04）；未在本会话落地，规模比 04 大、`key()` 改写面是 1006 行业务，留给下个会话 / `/implement` 实做。本会话把勘探备忘留在 § 勘探备忘。
- 2026-10-19：骨架 commit `8e6de67` 落下 `KeyboardLayer` trait + 三态 enum + 3 条单测（§ 验收 3、4 满足；§ 验收 1、2 留给下一会话）。