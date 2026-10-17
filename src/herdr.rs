//! 对 Herdr 的「自接入」上报（https://herdr.dev/docs/add-herdr-support/）。
//!
//! Herdr 靠 `src/detect/mod.rs` 的 `lookup_agent` 认 pane 里的 agent —— 那是一张精确等值表，
//! 没有 `heng`。第三方 agent 的正路是**自己上报**：状态（`idle` / `working` / `blocked`）、
//! resume 命令、退出时释放。于是 herdr 的侧栏能显示 heng 的名字与状态，server 重启后还能把
//! 会话原样叫回来。
//!
//! **只在 herdr 里才做**：`HERDR_ENV=1` 且 `HERDR_PANE_ID` / `HERDR_BIN_PATH` 都在。外面一律
//! `None`，一次调用都不发。上报失败一律忽略 —— 它是给人看的读数，不是同步手段。
//!
//! 上报走**后台线程 + 短命进程**：主循环只拼一次 argv，从不等待。`--seq` 用 UNIX 毫秒并在
//! 进程内保证严格递增，herdr 忽略比它旧的报告，所以并发与乱序都不会把新状态盖回去。

use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::EnvMap;
use crate::session::host::SessionState;

/// `--source`：我们这套集成的稳定标识。官方要求不以前缀 `herdr:` 开头。
const SOURCE: &str = "heng";
/// `--agent`：用户看到的名字。
const AGENT: &str = "heng";

/// herdr 侧栏上那个 agent 的三态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    /// 等人。
    Idle,
    /// 正在跑一个回合。
    Working,
    /// 停在等人答的那一步（问卷 / 权限门）。
    Blocked,
}

impl AgentState {
    /// 与宿主表的三态一一对应（`.scratch/session-panes/issues/11-*.md` 第 4 条）。
    pub fn from_session(state: SessionState) -> Self {
        match state {
            SessionState::Idle => AgentState::Idle,
            SessionState::RunningTurn => AgentState::Working,
            SessionState::AwaitingHuman => AgentState::Blocked,
        }
    }

    /// 协议里的字面量。
    pub fn as_str(self) -> &'static str {
        match self {
            AgentState::Idle => "idle",
            AgentState::Working => "working",
            AgentState::Blocked => "blocked",
        }
    }
}

/// 这个进程是不是跑在 herdr 的 pane 里（票 27 的判据）：**`HERDR_ENV == "1"` 精确等值**。
///
/// 官方承认这个变量但没有稳定性承诺，所以读不到 / 非 `1` / 不存在都当不在 herdr。
pub fn in_herdr(env: &EnvMap) -> bool {
    env.get("HERDR_ENV").map(String::as_str) == Some("1")
}

/// 上报器。`Reporter::from_env` 返回 `None` 就表示不在 herdr 里。
#[derive(Debug, Clone)]
pub struct Reporter {
    bin: String,
    pane: String,
}

impl Reporter {
    /// 从环境里认 herdr。判据是 `HERDR_ENV == "1"`（官方承认，但没有稳定性承诺），且两个
    /// 句柄都在；缺一个就当不在 herdr —— **检测失败绝不产生副作用**。
    pub fn from_env(env: &EnvMap) -> Option<Self> {
        if !in_herdr(env) {
            return None;
        }
        let bin = env.get("HERDR_BIN_PATH")?.clone();
        let pane = env.get("HERDR_PANE_ID")?.clone();
        if bin.is_empty() || pane.is_empty() {
            return None;
        }
        Some(Self { bin, pane })
    }

    /// 报一次状态，并带上 resume 命令（`heng -c <id>`）。resume 挂在每一条报告上，因为
    /// 「换了会话」与「状态变了」在这里走同一条路。
    pub fn report(&self, state: AgentState, session: &str) {
        let argv = report_argv(&self.pane, state, session, next_seq());
        self.run_in_background(argv);
    }

    /// 退出时释放这个 pane。异步线程可能来不及跑就被进程退出带走，所以这一条**同步**执行
    /// （herdr 的 CLI 调用是本地短命令，几十毫秒）。
    pub fn release(&self) {
        let argv = release_argv(&self.pane, next_seq());
        let _ = Command::new(&self.bin)
            .args(&argv)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    fn run_in_background(&self, argv: Vec<String>) {
        let bin = self.bin.clone();
        // 后台线程跑完 `status()`：不阻塞主循环，也不留僵尸。
        std::thread::spawn(move || {
            let _ = Command::new(&bin)
                .args(&argv)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        });
    }
}

/// `pane report-agent` 的 argv。纯函数，好测。
pub fn report_argv(pane: &str, state: AgentState, session: &str, seq: u64) -> Vec<String> {
    vec![
        "pane".into(),
        "report-agent".into(),
        pane.into(),
        "--source".into(),
        SOURCE.into(),
        "--agent".into(),
        AGENT.into(),
        "--state".into(),
        state.as_str().into(),
        "--seq".into(),
        seq.to_string(),
        "--agent-session-id".into(),
        session.into(),
        "--".into(),
        "heng".into(),
        "-c".into(),
        session.into(),
    ]
}

/// `pane release-agent` 的 argv。
pub fn release_argv(pane: &str, seq: u64) -> Vec<String> {
    vec![
        "pane".into(),
        "release-agent".into(),
        pane.into(),
        "--source".into(),
        SOURCE.into(),
        "--agent".into(),
        AGENT.into(),
        "--seq".into(),
        seq.to_string(),
    ]
}

/// 进程内严格递增的序号。起始值取 UNIX 毫秒，于是跨重启也比上一次大。
static LAST_SEQ: AtomicU64 = AtomicU64::new(0);

fn next_seq() -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0);
    let mut last = LAST_SEQ.load(Ordering::Relaxed);
    loop {
        let next = now.max(last.saturating_add(1));
        match LAST_SEQ.compare_exchange_weak(last, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return next,
            Err(current) => last = current,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> EnvMap {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    #[test]
    fn herdr_is_detected_only_by_the_exact_env_value() {
        // 票 27 的判据：`HERDR_ENV == "1"` **精确等值** —— 读不到、`0`、`true` 都当不在 herdr。
        let mut inside = EnvMap::new();
        inside.insert("HERDR_ENV".to_owned(), "1".to_owned());
        assert!(in_herdr(&inside), "1 就是在 herdr 里");

        for value in ["0", "true", "yes", ""] {
            let mut map = EnvMap::new();
            map.insert("HERDR_ENV".to_owned(), value.to_owned());
            assert!(!in_herdr(&map), "{value:?} 不算 herdr");
        }
        assert!(!in_herdr(&EnvMap::new()), "没有这个变量也不算");
    }

    #[test]
    fn detects_herdr_only_when_all_three_variables_agree() {
        let full = env(&[
            ("HERDR_ENV", "1"),
            ("HERDR_PANE_ID", "pane-7"),
            ("HERDR_BIN_PATH", "/usr/bin/herdr"),
        ]);
        assert_eq!(
            Reporter::from_env(&full).map(|r| (r.bin, r.pane)),
            Some(("/usr/bin/herdr".to_string(), "pane-7".to_string()))
        );

        for missing in ["HERDR_ENV", "HERDR_PANE_ID", "HERDR_BIN_PATH"] {
            let partial: EnvMap = full
                .iter()
                .filter(|(key, _)| key.as_str() != missing)
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            assert!(
                Reporter::from_env(&partial).is_none(),
                "缺 {missing} 时不该认成 herdr"
            );
        }

        let outside = env(&[("HERDR_ENV", "0")]);
        assert!(Reporter::from_env(&outside).is_none());

        let empty = env(&[
            ("HERDR_ENV", "1"),
            ("HERDR_PANE_ID", ""),
            ("HERDR_BIN_PATH", "/usr/bin/herdr"),
        ]);
        assert!(Reporter::from_env(&empty).is_none());
    }

    #[test]
    fn session_states_map_onto_agent_states() {
        assert_eq!(
            AgentState::from_session(SessionState::Idle),
            AgentState::Idle
        );
        assert_eq!(
            AgentState::from_session(SessionState::RunningTurn),
            AgentState::Working
        );
        assert_eq!(
            AgentState::from_session(SessionState::AwaitingHuman),
            AgentState::Blocked
        );
    }

    #[test]
    fn a_state_report_carries_its_resume_command() {
        let argv = report_argv("pane-7", AgentState::Working, "20261017T101500Z-abc123", 42);
        assert_eq!(
            argv,
            vec![
                "pane",
                "report-agent",
                "pane-7",
                "--source",
                "heng",
                "--agent",
                "heng",
                "--state",
                "working",
                "--seq",
                "42",
                "--agent-session-id",
                "20261017T101500Z-abc123",
                "--",
                "heng",
                "-c",
                "20261017T101500Z-abc123",
            ]
        );
    }

    #[test]
    fn a_release_names_the_same_source_and_agent() {
        let argv = release_argv("pane-7", 43);
        assert_eq!(
            argv,
            vec![
                "pane",
                "release-agent",
                "pane-7",
                "--source",
                "heng",
                "--agent",
                "heng",
                "--seq",
                "43",
            ]
        );
    }

    #[test]
    fn sequence_numbers_strictly_increase() {
        let mut last = next_seq();
        for _ in 0..1_000 {
            let next = next_seq();
            assert!(next > last, "{last} 之后是 {next}");
            last = next;
        }
        // 同一毫秒内连发两次也必须递增（herdr 忽略相等或更旧的报告）。
        let a = next_seq();
        let b = next_seq();
        assert!(b > a, "同一毫秒内也要递增：{a} 之后是 {b}");
    }
}
