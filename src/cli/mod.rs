//! CLI 边界：唯一读环境的地方。
//!
//! 二进制只是这个模块外面一层薄壳。库需要的一切都在这里从 `config.toml`、导出的环境与内建默认值组
//! 装出来，然后注入 [`crate::assemble`]。
//!
//! `probe` 是票 02 的手工验收工具：它对每个已配置的模型驱动一个真实回合，然后在同一个会话里再驱动
//! 第二个回合，这样第二个请求的用量行就能看出前缀缓存有没有命中。它无头运行，用一次性的会话日志。
//!
//! `prune` 与 `sessions` 读的都是票 12 建出来的那个 store：`prune` 手动删掉某个工作区的会话目录，
//! 而 `sessions` 从一个**已结束**会话自己的流里回答关于它的问题 —— `ls`、按轮分组的 `show`
//! （`--files` 是按工作区对象看的那个视图）、`replay`（投影的重算，spec §18）与 `stats`。它们住在
//! 这里，因为 store 的 root 来自环境，而库从不读环境；查询本身在
//! [`crate::session::observe`] 与 [`crate::agent::replay`]。交互式渲染器仍是唯一没有接线的那一块
//! （票 18）。
use self::discuss::discuss;
use self::interactive::interactive;
use self::probe::{load_config, probe, prune};
pub use self::sessions::run_sessions;
use crate::agent::{CancelSignal, UndoOutcome, replay};
use crate::config::{self, Config, Debater, DiscussionRoster, EnvMap, ReasoningEffort};
use crate::events::{
    ContextSource, Event, EventLog, EventPayload, SessionId, SpeakerId, StopReason, Usage,
    read_events, total_usage,
};
use crate::mcp::{self, McpService};
use crate::permissions::{Asker, Mode, Policy};
use crate::provider::capability::caps_for;
use crate::provider::openai::{BuildError, OpenAiProvider, stderr_warnings};
use crate::provider::{Message, ToolSpec};
use crate::questions::{UserQuestion, UserQuestions};
use crate::render::declaration::ToolDeclaration;
use crate::render::token::{self, Token};
use crate::render::{
    self, ConsoleAsker, ConsoleEvents, ConsoleHandle, ConsoleQuestions, FrontEndEvent, PickerKind,
    PickerOption, PlainOptions, RenderSinks, Renderer, SessionFacts, TuiOptions,
};
use crate::session::observe::{self, CostModel, Entry, Filter, Listing, Timeline};
use crate::session::{FrontFacts, SessionHost, SessionStore, StoredSession, titles};
use crate::tools::{self, Sandbox};
use crate::web::WebService;
use crate::web::fetch_http::HttpFetch;
use crate::web::search_deepseek::{DEEPSEEK_SEARCH_PROVIDER, DeepSeekSearch};
use crate::{
    AssemblyParts, DebaterParts, DiscussionHarness, DiscussionParts, Harness, Loaded,
    SessionScaffold, SynthesizerParts, assemble, assemble_discussion, load,
};
use ratatui::buffer::CellWidth;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

mod discuss;
mod goals;
mod interactive;
mod loading;
mod probe;
mod sessions;

#[cfg(test)]
mod tests;

/// 如果这个进程是 root，它为什么拒绝启动（spec §20）。
///
/// 刻意做成只由有效 uid 决定的纯函数：这次拒绝除了 uid 没有任何可读的东西，所以没有任何参数、环境
/// 变量或模式能把它关掉。这个项目的护栏假定最糟的情况留在工作区之内；而作为 root，一次误判就是全系
/// 统的，那是与正在管理的风险不同的一类风险。
pub fn root_refusal(euid: u32) -> Option<String> {
    (euid == 0).then(|| render::wording::root_refusal().to_owned())
}
/// 从环境里解析 `argv` 并运行。这是二进制的入口。
pub fn main() -> ExitCode {
    // 先于一切检查 —— 先于参数、先于 runtime —— 所以进不了以 root 身份运行程序的那条路径，也没有
    // 任何旗标能跳过这个检查（spec §20）。
    // SAFETY: `geteuid` 只读调用进程的 uid，不会失败。
    let euid = unsafe { libc::geteuid() };
    if let Some(message) = root_refusal(euid) {
        eprintln!("heng: {message}");
        return ExitCode::FAILURE;
    }
    let env: EnvMap = std::env::vars().collect();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!(
                "heng: {}",
                render::wording::startup_runtime(&error.to_string())
            );
            return ExitCode::FAILURE;
        }
    };
    runtime.block_on(run(&args, &env))
}
async fn run(args: &[String], env: &EnvMap) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("--version") | Some("-V") => {
            println!("heng {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("--help") | Some("-h") => {
            println!("{}", render::wording::help_main());
            ExitCode::SUCCESS
        }
        Some("probe") => probe(&args[1..], env).await,
        Some("discuss") => discuss(&args[1..], env).await,
        Some("prune") => prune(&args[1..], env),
        Some("sessions") => {
            let stdout = std::io::stdout();
            let stderr = std::io::stderr();
            let mut out = stdout.lock();
            let mut err = stderr.lock();
            run_sessions(&args[1..], env, &mut out, &mut err)
        }
        // 没有子命令（或者只有一个裸旗标）就是交互式会话：常见情形就是在某个工作区里直接跑
        // `heng`。`interactive` 自己解析自己那批参数，并拒掉任何它不认识的东西。
        _ => interactive(args, env).await,
    }
}

// ---------------------------------------------------------------------------
// 交互式前端（spec §19）
// ---------------------------------------------------------------------------
/// 交互式会话退出时的收尾：在终端交还之后往 `out` 打一行能直接粘的复盘命令，然后**原样**
/// 返回 `code`（`.scratch/exit-gesture/spec.md` §5）。
///
/// 收一个 writer 是为了让它可测 —— 直接在循环里 `eprintln!` 的话没人能断言它，而「打了几
/// 次、打给谁」正是这条需求要钉住的东西。写失败（管道断了）就吞掉：一行回执绝不该改退出码。
pub fn finish_session<W: Write>(code: ExitCode, session_id: &str, out: &mut W) -> ExitCode {
    let _ = writeln!(
        out,
        "heng: {}",
        render::wording::session_receipt(session_id)
    );
    code
}

// ---------------------------------------------------------------------------
// 讨论前端（spec §15）
// ---------------------------------------------------------------------------
