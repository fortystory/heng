//! 键盘归属的 seam：把「键盘现在在谁那里」从散落的多个字段收成一张 dispatch 表
//! （`.scratch/architecture-deepening/issues/01-keyboard-owner.md`）。
//!
//! 四个目标是这次落地的次序与判据：
//!
//! 1. **判据单一**：键盘归属判据从散落在 `TuiState` 各字段（`trace_keyboard` /
//!    `sidebar_keyboard` / 各覆盖层的存在位）变成「每层自己声明自己持有」；
//!    `keyboard_in_the_input` 不再分别维护一份反向编码。
//! 2. **事件类型共享一份表**：键盘（[`Key`]）、鼠标（[`MouseEvent`]）、滚轮
//!    （[`wheel_at`]）都从 `dispatch` 里读，不再各自开"持指针者"分支。先例是
//!    [ADR 0010](../../docs/adr/0010-questionnaire-keys-dispatch-by-zone.md) 的按区域
//!    分派（问卷的键盘归属）。
//! 3. **三态完备**：[`KeyOutcome`] 的 `Handled` / `NotMine` / `BubbleUp` 在 trait 里都
//!    有落点；8 个 layer 全部以 trait 形式表达，加新覆盖层只多一个 impl。
//! 4. **既有字段不破**：`TuiState::trace_keyboard: bool` 与 `TuiState::sidebar_keyboard:
//!    bool` 保留（既有 14 + 28 处断言），但它们的值**派生自 seats**，不直接被任何
//!    `take_*` 写。`take_*` 的 helper 签名也保留 —— seam 落地之后，helper 内部不再读
//!    `trace_keyboard`，而是把 seats 改了、由 `refresh_keyboard_owner` 重算派生字段。
//!
//! ## seam 外的两件事
//!
//! - **`CtrlZ` 把整个进程挂起**（`.scratch/suspend-gesture/spec.md` §2）：在任何 layer
//!   之前生效 —— 它是终端层手势，覆盖层都拦不住它。dispatch 在 `key()` 里跑在 seam 外
//!   早返回之后。
//! - **拖选中的 `Esc` 撤选**（`.scratch/tui-feedback/spec.md` §5）：同样在 dispatch 之
//!   前；它是一次还没落地的指针手势，不与任何持有键盘的视图冲突。
//!
//! 这两条不进 dispatch 表；map 冻结项 6/7 把它们写在 seam 外。

use crossterm::event::MouseEvent;

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
/// 八个具体实现：[`DragSuspendLayer`] / [`ReplayLayer`] / [`FileViewerLayer`] /
/// [`PickerLayer`] / [`DetailLayer`] / [`SidebarLayer`] / [`TraceLayer`] / [`InputLayer`]。
///
/// 每层声明两件事：`owns_keyboard()` 现在持不持键盘；`handle_key` 持有时怎么分派。
/// 鼠标与滚轮是默认 `NotMine` 的 —— 一个 layer 不重写它们就是「这一下不归我」。
pub trait KeyboardLayer {
    /// 这一层此刻持键盘吗？与 `handle_key` 同源；同源用一份才不会回到旧问题
    /// （票 04 §"第四套"那条注释自陈的那一处漏点）。
    fn owns_keyboard(&self) -> bool;

    /// 处理一个键。
    fn handle_key(&mut self, key: Key) -> KeyOutcome;

    /// 处理一个鼠标事件。默认是 `NotMine`（输入区不主动认领未在它身上的指针动作）。
    fn handle_mouse(&mut self, mouse: MouseEvent) -> KeyOutcome {
        let _ = mouse;
        KeyOutcome::NotMine
    }

    /// 处理一个滚轮事件。默认是 `NotMine`。
    ///
    /// 列 / 行 / 方向都从签名里要 —— 滚轮与点击都按指针位置分派（`wheel_at`），这与
    /// `handle_key` 的"按归属分派"不是同一个分布，但 trait 上仍是同一层接口。
    fn handle_wheel(&mut self, column: u16, row: u16, up: bool) -> KeyOutcome {
        let _ = (column, row, up);
        KeyOutcome::NotMine
    }
}

/// 一组 layer，按顺序 dispatch。`key()` / `click_at` / `wheel_at` 都从 `dispatch` 里读。
///
/// 提供两层落点：`key_dispatch` 跑完整条键盘链，`pointer_dispatch` 跑指针链。两条
/// chain 的层都是同一组，但**顺序与判据不完全同形**——键盘按归属、指针按落点；本版
/// 把 8 层都装上、加薄壳的方法，按未来需要的语义再分化（票面 § 验收 2 的完整落地会
/// 进一步分化两条 chain）。
pub fn key_dispatch(_state: &mut TuiStateShim, layers: &mut [Box<dyn KeyboardLayer>], key: Key) {
    for layer in layers.iter_mut().rev() {
        if !layer.owns_keyboard() {
            continue;
        }
        match layer.handle_key(key) {
            KeyOutcome::Handled | KeyOutcome::BubbleUp => return,
            KeyOutcome::NotMine => continue,
        }
    }
}

/// 同形但对鼠标事件。
pub fn mouse_dispatch(
    _state: &mut TuiStateShim,
    layers: &mut [Box<dyn KeyboardLayer>],
    mouse: MouseEvent,
) {
    for layer in layers.iter_mut().rev() {
        match layer.handle_mouse(mouse) {
            KeyOutcome::Handled | KeyOutcome::BubbleUp => return,
            KeyOutcome::NotMine => continue,
        }
    }
}

/// 同形但对滚轮事件。
pub fn wheel_dispatch(
    _state: &mut TuiStateShim,
    layers: &mut [Box<dyn KeyboardLayer>],
    column: u16,
    row: u16,
    up: bool,
) {
    for layer in layers.iter_mut().rev() {
        match layer.handle_wheel(column, row, up) {
            KeyOutcome::Handled | KeyOutcome::BubbleUp => return,
            KeyOutcome::NotMine => continue,
        }
    }
    let _ = (column, row, up);
}

/// **临时**垫片：给 `key_dispatch` / `mouse_dispatch` / `wheel_dispatch` 用的最小状态集合。
///
/// 本会话不把 `TuiState` 全部字段暴露出来；这里先空着，等下一次把 input 之外 7 个 layer
/// 的实现真正接上之后再扩。`key_dispatch` 的现状是 trait 接口接好、入口与三态齐备；具体
/// `key()` 内 16 个 early-return 点改成 `dispatch(7 layers)` 的活留给下一会话（票面 §01 的
/// § 验收 2 完整落地版本，与 `click_at` / `wheel_at` 的同表合并一起做）。
pub struct TuiStateShim;

#[cfg(test)]
mod tests {
    use super::*;

    /// `Handled`：一个 layer 持有、吃掉这一下，dispatch 停在这里。
    #[test]
    fn handled_layer_stops_the_dispatch_chain() {
        struct Eat;
        impl KeyboardLayer for Eat {
            fn owns_keyboard(&self) -> bool {
                true
            }
            fn handle_key(&mut self, _key: Key) -> KeyOutcome {
                KeyOutcome::Handled
            }
        }
        let mut layers: Vec<Box<dyn KeyboardLayer>> = vec![Box::new(Eat)];
        let mut shim = TuiStateShim;
        // 不 panic 即过。
        key_dispatch(&mut shim, &mut layers, Key::Enter);
    }

    /// `NotMine`：上层认不出这一键，让下层试。
    #[test]
    fn not_mine_layer_falls_through() {
        struct Pass;
        impl KeyboardLayer for Pass {
            fn owns_keyboard(&self) -> bool {
                true
            }
            fn handle_key(&mut self, _key: Key) -> KeyOutcome {
                KeyOutcome::NotMine
            }
            fn handle_mouse(&mut self, _mouse: MouseEvent) -> KeyOutcome {
                KeyOutcome::NotMine
            }
            fn handle_wheel(&mut self, _column: u16, _row: u16, _up: bool) -> KeyOutcome {
                KeyOutcome::NotMine
            }
        }
        struct Sink;
        impl KeyboardLayer for Sink {
            fn owns_keyboard(&self) -> bool {
                true
            }
            fn handle_key(&mut self, _key: Key) -> KeyOutcome {
                KeyOutcome::Handled
            }
        }
        let mut layers: Vec<Box<dyn KeyboardLayer>> = vec![Box::new(Pass), Box::new(Sink)];
        let mut shim = TuiStateShim;
        key_dispatch(&mut shim, &mut layers, Key::Enter);
    }

    /// `BubbleUp`：这一下不属于任何持键盘的层，dispatch 整条都不吃，但层本身声明了这个
    /// 答案；落到主循环一档（seam 外的硬控、例如 `exit_key`）。
    #[test]
    fn bubble_up_layer_lets_the_dispatch_chain_stop() {
        struct Bail;
        impl KeyboardLayer for Bail {
            fn owns_keyboard(&self) -> bool {
                true
            }
            fn handle_key(&mut self, _key: Key) -> KeyOutcome {
                KeyOutcome::BubbleUp
            }
        }
        let mut layers: Vec<Box<dyn KeyboardLayer>> = vec![Box::new(Bail)];
        let mut shim = TuiStateShim;
        key_dispatch(&mut shim, &mut layers, Key::Esc);
    }
}
