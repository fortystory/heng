//! 键盘归属的 seam：把「键盘现在在谁那里」从散落的多个字段收成一张 dispatch 表
//! （`.scratch/architecture-deepening/issues/01-keyboard-owner.md`）。
//!
//! 四个目标是这次落地的次序与判据：
//!
//! 1. **判据单一**：键盘归属判据从散落在 `TuiState` 各字段（`trace_keyboard` /
//!    `sidebar_keyboard` / 各覆盖层的存在位）变成「每层自己声明自己持有」；
//!    `keyboard_in_the_input` 不再分别维护一份反向编码。
//! 2. **事件类型共享一份表**：键盘（[`Key`]）、鼠标（点击落点）、滚轮（落点 + 方向）
//!    都从 [`key_dispatch`] / [`mouse_dispatch`] / [`wheel_dispatch`] 里读，不再各自开
//!    「持指针者」分支。先例是
//!    [ADR 0010](../../docs/adr/0010-questionnaire-keys-dispatch-by-zone.md) 的按区域
//!    分派（问卷的键盘归属）。
//! 3. **三态完备**：[`KeyOutcome`] 的 `Handled` / `NotMine` / `BubbleUp` 在 trait 里都
//!    有落点；[`KeyboardLayer`] 泛型化到状态类型，`TuiState` 的 7 个 layer 与测试里的
//!    假 layer 共用同一套 dispatch。
//! 4. **既有字段不破**：`TuiState::trace_keyboard: bool` 与 `TuiState::sidebar_keyboard:
//!    bool` 保留（既有 14 + 28 处断言），但它们的值只由 `take_*` / `release_*` 写，
//!    而「键盘在谁那里」的**判据**只有一份 —— 各 layer 的 `owns_keyboard`。
//!
//! ## seam 外的两件事
//!
//! - **`CtrlZ` 把整个进程挂起**（`.scratch/suspend-gesture/spec.md` §2）：在任何 layer
//!   之前生效 —— 它是终端层手势，覆盖层都拦不住它。dispatch 在 `key()` 里跑在 seam 外
//!   早返回之后。
//! - **`Ctrl-X` 关掉前台那一个会话**（票 09）：与挂起同一档的终端层手势 —— 任何界面都
//!   拦不住它。
//! - **拖选中的 `Esc` 撤选**（`.scratch/tui-feedback/spec.md` §5）：同样在 dispatch 之
//!   前；它是一次还没落地的指针手势，不与任何持有键盘的视图冲突。
//!
//! 这两条不进 dispatch 表；map 冻结项 6/7 把它们写在 seam 外。它们就是票面列的第八个
//! layer（`DragSuspend`）：它没有「被谁持有」的判据，因此没有 trait 实现。

use super::tui::Key;

/// 一个 layer 对一个事件的回答。
///
/// 三态覆盖 layer 与主循环、layer 与 layer 之间的全部关系：
///
/// - [`KeyOutcome::Handled`] —— 这一层吃掉这一下，dispatch 停在这里。
/// - [`KeyOutcome::NotMine`] —— 这一层不认这一下，落到下一层继续试。
/// - [`KeyOutcome::BubbleUp`] —— 这一下不属于任何「持键盘的层」，交给主循环做
///   seam 之外的事（如 `exit_key` 那一档）。dispatch 整条链都没有这一层的时候，
///   InputLayer 的内部仍然负责兜底。
///
/// 落到最底层的 `NotMine` 不必显式表达：dispatch 的尾迭代逻辑就处理了。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOutcome {
    Handled,
    NotMine,
    BubbleUp,
}

/// 一层键盘归属。
///
/// 七个具体实现（都在 `tui.rs` 的 `keyboard_layers` 模块里）：
/// `ReplayLayer` / `FileViewerLayer` / `PickerLayer` / `DetailLayer` /
/// `SidebarLayer` / `TraceLayer` / `InputLayer`。
///
/// 泛型参数是 dispatch 读写的状态类型：`TuiState` 是生产实现，测试可以用任意空类型。
/// 每层声明两件事：`owns_keyboard()` 现在持不持键盘；`handle_key` 持有时怎么分派。
/// 鼠标与滚轮是默认 `NotMine` 的 —— 一个 layer 不重写它们就是「这一下不归我」。
///
/// **顺序约定**：`layers` 从「最外层 / 最先试」到「输入区兜底」排列，dispatch 反向迭代。
/// 这是票面要的那个 `for layer in layers.iter().rev()`。
pub trait KeyboardLayer<S> {
    /// 这一层此刻持键盘吗？与 `handle_key` 同源；同源用一份才不会回到旧问题
    /// （票 04 §"第四套"那条注释自陈的那一处漏点）。
    fn owns_keyboard(&self, state: &S) -> bool;

    /// 处理一个键。
    fn handle_key(&mut self, state: &mut S, key: Key) -> KeyOutcome;

    /// 处理一次点击落点。默认是 `NotMine`（输入区不主动认领不在它身上的指针动作）。
    fn handle_mouse(&mut self, state: &mut S, column: u16, row: u16) -> KeyOutcome {
        let _ = (state, column, row);
        KeyOutcome::NotMine
    }

    /// 处理一格滚轮。默认是 `NotMine`。
    ///
    /// 列 / 行 / 方向都从签名里要 —— 滚轮与点击都按指针位置分派（`wheel_at`），这与
    /// `handle_key` 的「按归属分派」不是同一个分布，但 trait 上仍是同一层接口。
    fn handle_wheel(&mut self, state: &mut S, column: u16, row: u16, up: bool) -> KeyOutcome {
        let _ = (state, column, row, up);
        KeyOutcome::NotMine
    }

    /// 这一层是不是「输入区兜底」。
    ///
    /// `keyboard_in_the_input` 只问「有没有覆盖层在输入区之前持有键盘」；兜底层恒持有
    /// 键盘（所有键最终落到它），所以派生光标可见性时要把它排除。
    fn is_input_fallback(&self) -> bool {
        false
    }
}

/// 跑完整条键盘链：从最外层开始，第一层「持有且认这一键」的吃掉它。
///
/// 返回最后那个答案：没有任何层认领时是 [`KeyOutcome::NotMine`]；`BubbleUp` 与
/// `Handled` 都让链停下（前者表示「这一下交给主循环那一档」）。
pub fn key_dispatch<S>(
    state: &mut S,
    layers: &mut [Box<dyn KeyboardLayer<S>>],
    key: Key,
) -> KeyOutcome {
    let mut last = KeyOutcome::NotMine;
    for layer in layers.iter_mut().rev() {
        if !layer.owns_keyboard(state) {
            continue;
        }
        last = layer.handle_key(state, key);
        if matches!(last, KeyOutcome::Handled | KeyOutcome::BubbleUp) {
            return last;
        }
    }
    last
}

/// 同形的指针链：只按落点分派，不先看 `owns_keyboard`（持指针者按区域判，各层自查）。
///
/// 返回 `true` 表示这一下已经被某一层吃掉，调用方不再做命中动作分派。
pub fn mouse_dispatch<S>(
    state: &mut S,
    layers: &mut [Box<dyn KeyboardLayer<S>>],
    column: u16,
    row: u16,
) -> bool {
    for layer in layers.iter_mut().rev() {
        match layer.handle_mouse(state, column, row) {
            KeyOutcome::Handled | KeyOutcome::BubbleUp => return true,
            KeyOutcome::NotMine => continue,
        }
    }
    false
}

/// 同形但滚轮事件。
pub fn wheel_dispatch<S>(
    state: &mut S,
    layers: &mut [Box<dyn KeyboardLayer<S>>],
    column: u16,
    row: u16,
    up: bool,
) {
    for layer in layers.iter_mut().rev() {
        match layer.handle_wheel(state, column, row, up) {
            KeyOutcome::Handled | KeyOutcome::BubbleUp => return,
            KeyOutcome::NotMine => continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试状态：dispatch 只需要一个可变的占位。
    #[derive(Default)]
    struct Shim;

    /// `Handled`：一个 layer 持有、吃掉这一下，dispatch 停在这里。
    #[test]
    fn handled_layer_stops_the_dispatch_chain() {
        struct Eat;
        impl KeyboardLayer<Shim> for Eat {
            fn owns_keyboard(&self, _state: &Shim) -> bool {
                true
            }
            fn handle_key(&mut self, _state: &mut Shim, _key: Key) -> KeyOutcome {
                KeyOutcome::Handled
            }
        }
        let mut layers: Vec<Box<dyn KeyboardLayer<Shim>>> = vec![Box::new(Eat)];
        let mut shim = Shim;
        assert_eq!(
            key_dispatch(&mut shim, &mut layers, Key::Enter),
            KeyOutcome::Handled
        );
    }

    /// `NotMine`：上层认不出这一键，让下层试。
    #[test]
    fn not_mine_layer_falls_through() {
        struct Pass;
        impl KeyboardLayer<Shim> for Pass {
            fn owns_keyboard(&self, _state: &Shim) -> bool {
                true
            }
            fn handle_key(&mut self, _state: &mut Shim, _key: Key) -> KeyOutcome {
                KeyOutcome::NotMine
            }
        }
        struct Sink;
        impl KeyboardLayer<Shim> for Sink {
            fn owns_keyboard(&self, _state: &Shim) -> bool {
                true
            }
            fn handle_key(&mut self, _state: &mut Shim, _key: Key) -> KeyOutcome {
                KeyOutcome::Handled
            }
        }
        // 顺序：`Sink` 在 `Pass` 之后 = dispatch 先试 `Sink`；换成相反顺序才走到 `Sink`。
        // 这里让 `Pass` 排在后面先被试到，证明 NotMine 会落到 `Sink`。
        let mut layers: Vec<Box<dyn KeyboardLayer<Shim>>> = vec![Box::new(Sink), Box::new(Pass)];
        let mut shim = Shim;
        assert_eq!(
            key_dispatch(&mut shim, &mut layers, Key::Enter),
            KeyOutcome::Handled
        );
    }

    /// `BubbleUp`：这一下不属于任何持键盘的层，dispatch 整条都不吃，但层本身声明了这个
    /// 答案；落到主循环一档（seam 外的硬控、例如 `exit_key`）。
    #[test]
    fn bubble_up_layer_lets_the_dispatch_chain_stop() {
        struct Bail;
        impl KeyboardLayer<Shim> for Bail {
            fn owns_keyboard(&self, _state: &Shim) -> bool {
                true
            }
            fn handle_key(&mut self, _state: &mut Shim, _key: Key) -> KeyOutcome {
                KeyOutcome::BubbleUp
            }
        }
        let mut layers: Vec<Box<dyn KeyboardLayer<Shim>>> = vec![Box::new(Bail)];
        let mut shim = Shim;
        assert_eq!(
            key_dispatch(&mut shim, &mut layers, Key::Esc),
            KeyOutcome::BubbleUp
        );
    }

    /// 没层的 `NotMine` 是 dispatch 的收尾答案。
    #[test]
    fn all_not_mine_leaves_the_key_unclaimed() {
        struct Pass;
        impl KeyboardLayer<Shim> for Pass {
            fn owns_keyboard(&self, _state: &Shim) -> bool {
                true
            }
            fn handle_key(&mut self, _state: &mut Shim, _key: Key) -> KeyOutcome {
                KeyOutcome::NotMine
            }
        }
        let mut layers: Vec<Box<dyn KeyboardLayer<Shim>>> = vec![Box::new(Pass)];
        let mut shim = Shim;
        assert_eq!(
            key_dispatch(&mut shim, &mut layers, Key::Enter),
            KeyOutcome::NotMine
        );
    }
}
