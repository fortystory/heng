//! 这个会话的**工具声明**：provider 收到的那份三件套，抄给渲染层的那一份。
//!
//! 三件套是「名字 + 用法说明 + 参数 schema」（`src/provider/mod.rs` 的 `ToolSpec`）—— 这一份
//! 三样都在，**顺序**是那张表的顺序（`Registry::specs()` 给的那一份，与 provider 收到的
//! `tools` 数组同序）。
//!
//! 为什么用**渲染层自己的**小类型，而不是 provider 那个公开类型：渲染层要的是「用法说明 +
//! 参数 schema」两样，依赖 provider 的类型会把两层绑在一起
//! （`.scratch/tool-declaration-visibility/issues/06-grilling-boundaries.md` 的决定）。
//!
//! 抄的时候**用法说明也在内** —— 今天那份账只抄了参数 schema，于是 provider 读到的那段用法说明
//! 在界面上一个字都读不到
//! （`.scratch/tool-declaration-visibility/issues/07-account-carries-the-whole-declaration.md`）。

use serde_json::Value;

use super::wording;

/// 一个工具的**声明**：名字 + 用法说明 + 参数 schema（provider 收到的那三件套）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolDeclaration {
    /// 工具的名字（`ToolSpec::name`）—— provider 请求里的 `tools` 数组按它认人。
    pub name: String,
    /// provider 读到的那段**用法说明**（`ToolSpec::description`）—— 模型靠它知道这个工具怎么用。
    pub description: String,
    /// provider 读到的那份**参数 schema**（`ToolSpec::parameters`）。
    pub parameters: Value,
}

/// 这个会话那张工具表的**全表**：逐条摊开，每条是「名字 + 用法说明」。
///
/// **参数 schema 不在这里** —— 它在 per-call 那一档（详情覆盖层的「声明」面）里：那张面答的是
/// 「这次调用的那个工具声明是什么」，而这张全表答的是「这个会话有哪些工具、每个工具的用法
/// 说明是什么」。两件事各答各的。
///
/// 全表本来就不是一页纸：内建声明的用法说明合计五千多字符（研究 01 第 2 节）—— 于是它是
/// 「一行 + 点开一块可滚的详情」，不是「摊在转录上」。
///
/// **顺序是那张表的顺序** —— 与 provider 收到的 `tools` 数组同序（`Registry::specs()` 给的那
/// 一份）。
pub fn whole_table(declarations: &[ToolDeclaration]) -> String {
    let entries: Vec<String> = declarations
        .iter()
        .map(|declaration| {
            // 一条声明是「名字 + 用法说明」两段。
            format!(
                "{}\n{}",
                wording::declaration_table_entry(&declaration.name),
                declaration.description
            )
        })
        .collect();
    // 条与条之间空一行：不空开就分不清哪一段用法说明属于哪个工具。
    entries.join("\n\n")
}
