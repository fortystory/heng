//! 会话标题：权威链、前缀轻路径与进程内缓存（`.scratch/session-tab/spec.md` §4）。
//!
//! 两件事钉在一起：
//!
//! * **自动派生不落流** —— 它每次进程启动重算一遍，只在进程内缓存；落盘就是给标题造第二处
//!   真相（决策票 01 §4）。
//! * **读一眼不算改** —— 这里绝不走 [`crate::events::EventLog::open`]：那个入口会补换行、
//!   截断坏尾巴，也就是**改文件**。列表与派生都是只读动作。
//!
//! 权威链三级（决策票 01 §2）：
//!
//! 1. 流上**最后一条** `SessionTitled`，且它的 `title` 归一化后非空（空串是撤回，见 §3）；
//! 2. 否则该流**前 8 行**内首条非空 `MessageCompleted { role: User }` 的正文首行；
//! 3. 否则从 `SessionId` 的 UTC 段渲染**本地** `MM-DD HH:mm`；解析不出就取 id 前 8 字符。

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use crate::events::SessionId;

/// 派生只看一条流的**前 8 行**。
///
/// 实测（决策票 01 §2）：134 条有用户消息的流里 130 条落在第 4–7 行（p50 = 6、p90 = 7）。
/// 没命中的那几条不往下扫 —— 扫到文件尾是 54.5–58.5 ms，而前缀读是 3.7–4.8 ms。
pub const PREFIX_LINES: usize = 8;

/// 找「最后一条 `SessionTitled`」时尾扫多少字节。
///
/// 改名是人手敲的一两次，落点在流尾附近；一次 8 KiB 足够，而整条流可能有上万行。
const TAIL_BYTES: u64 = 8 * 1024;

/// 标题的字符上限（决策票 01 §5）。存储层不按**列**预截断 —— 那是渲染层的事。
pub const TITLE_CHARS: usize = 200;

/// 一条流的前缀里那两样东西。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Prefix {
    /// `SessionStarted.cwd`。桶名是不可逆的 FNV 哈希，工作目录只能从这里读。
    pub cwd: Option<String>,
    /// 前 8 行里**首条**归一化后非空的用户消息正文（原文，未截断）。
    pub first_user_text: Option<String>,
}

/// 只读一条流的头 8 行，逐行 `serde_json::Value`。
///
/// 刻意不做 `Event` 全量反序列化：一条流的 payload 形状会随版本漂，而这里只要两个字段。
/// 读不出的行跳过（一条坏流不毁整页）。
pub fn read_prefix(path: &Path) -> std::io::Result<Prefix> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut prefix = Prefix::default();
    for line in reader.lines().take(PREFIX_LINES) {
        // 两样都拿到了就不必再读 —— 头几行里常常有一次很大的上下文注入，读它是纯浪费。
        if prefix.cwd.is_some() && prefix.first_user_text.is_some() {
            break;
        }
        let Ok(line) = line else { break };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let Some(payload) = value.get("payload") else {
            continue;
        };
        if prefix.cwd.is_none() {
            prefix.cwd = payload
                .get("SessionStarted")
                .and_then(|started| started.get("cwd"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned);
        }
        if prefix.first_user_text.is_none()
            && let Some(message) = payload.get("MessageCompleted")
            && message.get("role").and_then(serde_json::Value::as_str) == Some("User")
            && let Some(text) = message.get("text").and_then(serde_json::Value::as_str)
            && normalize(text).is_some()
        {
            prefix.first_user_text = Some(text.to_owned());
        }
    }
    Ok(prefix)
}

/// 流上最后一条 `SessionTitled` 的 `title`，原样（空串即撤回，由调用方判）。
///
/// 尾扫而不是全量读：改名落点在流尾附近，而一条长流可能有上万行。文件开头被读到的那一截
/// 可能切在一个字中间，所以第一行丢掉 —— 短流（没有可丢的）完整读。
pub fn last_title(path: &Path) -> std::io::Result<Option<String>> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let start = len.saturating_sub(TAIL_BYTES);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes);
    let body = if start == 0 {
        text.as_ref()
    } else {
        match text.find('\n') {
            Some(at) => &text[at + 1..],
            None => "",
        }
    };
    for line in body.lines().rev() {
        if !line.contains("SessionTitled") {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(title) = value
            .get("payload")
            .and_then(|payload| payload.get("SessionTitled"))
            .and_then(|titled| titled.get("title"))
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        return Ok(Some(title.to_owned()));
    }
    Ok(None)
}

/// 标题字符串的归一化（决策票 01 §5）：取首行 → 去控制字符 → 内部连续空白压成一个空格 →
/// 首尾不留白 → 截到 [`TITLE_CHARS`] 个字符。
///
/// 空（或归一化之后为空）答 `None` —— 那正是「撤回」与「这一级没命中」共用的信号。刻意不解析
/// markdown，也不剥行首的 `#` / `-` / `>`（会显示成 `# 帮我改 x`，是记下来的取舍）。
pub fn normalize(text: &str) -> Option<String> {
    let first = text.lines().next().unwrap_or("");
    let mut out = String::new();
    let mut pending_space = false;
    for ch in first.chars() {
        if ch.is_control() {
            continue;
        }
        if ch.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push(ch);
    }
    let trimmed = out.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(TITLE_CHARS).collect())
}

/// 兜底标题：从 `SessionId` 的 UTC 段渲染**本地** `MM-DD HH:mm`；解析不出就取 id 前 8 字符。
///
/// 它**不进终端标题**（决策票 01 §7）：那里每一段都该是人选中的信息。左栏照显示 —— 行与行
/// 之间要能区分。
pub fn fallback_title(id: &SessionId) -> String {
    let raw = id.as_str();
    let stamp = raw.split('-').next().unwrap_or(raw).trim_end_matches('Z');
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(stamp, "%Y%m%dT%H%M%S") {
        let local = naive.and_utc().with_timezone(&chrono::Local);
        return local.format("%m-%d %H:%M").to_string();
    }
    raw.chars().take(8).collect()
}

/// 进程内的标题缓存，键是会话 id。
///
/// 懒算：问哪一场算哪一场。**不做失效判定** —— 一次运行里流文件没变，派生结果就没变；而改名
/// 那条路径自己会把新值写回来。它不落盘。
#[derive(Debug, Default)]
pub struct TitleCache {
    entries: HashMap<SessionId, String>,
}

impl TitleCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, id: &SessionId) -> Option<&str> {
        self.entries.get(id).map(String::as_str)
    }

    /// 记下一场会话的标题。改名之后调用方用它覆盖旧值。
    pub fn remember(&mut self, id: SessionId, title: String) {
        self.entries.insert(id, title);
    }

    /// 忘掉一场会话的标题（改完名之后让它重算）。
    pub fn forget(&mut self, id: &SessionId) {
        self.entries.remove(id);
    }
}

/// 这一场会话现在叫什么（权威链 + 进程内缓存）。
///
/// 缓存命中就直接答；没命中就走三级权威链，再把它记下来。
pub fn title_of(id: &SessionId, log_path: &Path, cache: &mut TitleCache) -> String {
    if let Some(title) = cache.get(id) {
        return title.to_owned();
    }
    let title = derive(id, log_path);
    cache.remember(id.clone(), title.clone());
    title
}

/// 权威链本身（不走缓存）。逐级往下，第一级命中就用它。
pub fn derive(id: &SessionId, log_path: &Path) -> String {
    // 第 1 级：最后一条 `SessionTitled`。空串是撤回 —— 它也意味着「这一级没命中」。
    if let Ok(Some(raw)) = last_title(log_path)
        && let Some(title) = normalize(&raw)
    {
        return title;
    }
    // 第 2 级：前 8 行内首条非空的用户消息。
    if let Ok(prefix) = read_prefix(log_path)
        && let Some(text) = prefix.first_user_text.as_deref()
        && let Some(title) = normalize(text)
    {
        return title;
    }
    // 第 3 级：兜底。
    fallback_title(id)
}
