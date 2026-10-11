//! 会话页要的那份列表：前缀轻路径 + mtime + 检出分组
//! （`.scratch/session-tab/spec.md` §3、§4）。
//!
//! 三条纪律在这里落地：
//!
//! * **绝不走 `observe::summarize`** —— 那个是全量解析（281 场实测 155.6–172.2 ms）。这里要的
//!   只有 `SessionStarted.cwd` 与前 8 行内首条用户消息，走 [`crate::session::titles::read_prefix`]
//!   （281 场 3.7–4.8 ms 那一档）。
//! * **绝不走 `EventLog::open`** —— 它会补换行、截断坏尾巴，也就是**改文件**。列表与派生都是
//!   只读动作。
//! * **一条坏流不毁整页**：`list_all` 只扫目录，坏流只体现在那一行的「流损坏」标记上。

use std::collections::HashMap;
use std::path::Path;
use std::time::SystemTime;

use crate::events::SessionId;
use crate::session::store::{SessionStore, StoredSession};
use crate::session::titles::{TitleCache, title_of};

/// 会话页上一行的状态标记。四态互斥，前两者只有宿主表说得出来
/// （`.scratch/session-tab/spec.md` §3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SessionMark {
    /// 空闲：标记列留空。
    #[default]
    Idle,
    /// 后台正在跑回合：`●`。
    Running,
    /// 等在一个人答：`?`。
    Awaiting,
}

/// 会话页的一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRow {
    pub id: SessionId,
    /// `SessionStarted.cwd`。读不出（前缀坏）就是 `None`。
    pub cwd: Option<String>,
    /// 权威链给的标题。
    pub title: String,
    /// 分组的键：检出家族（`--git-common-dir` 的绝对路径）或 cwd 字符串本身。
    pub group_key: String,
    /// 组头显示的那个名字。重名由 [`SessionListing::sessions`] 统一改成尾两段路径。
    pub group: String,
    /// 流文件最后一次被写入的时刻。
    pub written: Option<SystemTime>,
    /// 这一条流损坏到读不出前缀。
    pub broken: bool,
    /// `cwd` 那个目录现在不在了。
    pub missing_cwd: bool,
    /// 这一场就是当前前台。
    pub foreground: bool,
    /// 宿主表说的状态（票 22）。票 17 这一层只把它搬过来。
    pub mark: SessionMark,
}

/// store 里跑过的全部会话，按页面的顺序排好。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionListing {
    pub rows: Vec<SessionRow>,
    /// 一条都没露过面的会话数不算在这里 —— 它是 `rows.len()` 与 store 总数之差，由调用方
    /// 自己说（`＋N 场` 那一行）。
    pub total: usize,
    /// store 整个读不出（不是「没有会话」）。
    pub unreadable: bool,
}

/// 扫 store，给会话页排出一份列表。
///
/// `foreground` 是当前前台那一场：它所在的**检出组**被钉在组顶，它自己带 `▍`。
/// `marks` 是宿主表给的状态（票 22 填），票 17 这一层只负责把它贴到行上。
pub fn sessions(
    store: &SessionStore,
    cache: &mut TitleCache,
    foreground: &SessionId,
    marks: &HashMap<SessionId, SessionMark>,
) -> SessionListing {
    let stored = match store.list_all() {
        Ok(stored) => stored,
        Err(_) => {
            return SessionListing {
                rows: Vec::new(),
                total: 0,
                unreadable: true,
            };
        }
    };
    let total = stored.len();

    // 家族探测每 cwd 只起一次 `git` 进程（spec §3 的已知代价）。
    let mut families: HashMap<String, Option<String>> = HashMap::new();
    let mut rows: Vec<SessionRow> = Vec::with_capacity(stored.len());
    for session in stored {
        rows.push(row_of(session, cache, foreground, marks, &mut families));
    }

    label_groups(&mut rows);
    sort_rows(&mut rows, foreground);
    SessionListing {
        rows,
        total,
        unreadable: false,
    }
}

fn row_of(
    session: StoredSession,
    cache: &mut TitleCache,
    foreground: &SessionId,
    marks: &HashMap<SessionId, SessionMark>,
    families: &mut HashMap<String, Option<String>>,
) -> SessionRow {
    let written = std::fs::metadata(&session.log_path)
        .and_then(|meta| meta.modified())
        .ok();
    let prefix = crate::session::titles::read_prefix(&session.log_path);
    let (cwd, broken) = match prefix {
        Ok(prefix) => (
            prefix.cwd,
            // 头 8 行里一行 JSON 都解析不出来 —— 这条流坏了。打不开也是。
            prefix.lines > 0 && prefix.parsed == 0,
        ),
        Err(_) => (None, true),
    };
    let missing_cwd = cwd.as_deref().is_some_and(|cwd| !Path::new(cwd).is_dir());
    let family = cwd.as_deref().and_then(|cwd| {
        families
            .entry(cwd.to_owned())
            .or_insert_with(|| git_family(cwd))
            .clone()
    });
    let group_key = family.clone().unwrap_or_else(|| {
        cwd.as_deref()
            .map(|cwd| cwd.to_owned())
            .unwrap_or_else(|| "（读不出工作目录）".to_owned())
    });
    SessionRow {
        title: title_of(&session.id, &session.log_path, cache),
        foreground: &session.id == foreground,
        mark: marks.get(&session.id).copied().unwrap_or_default(),
        cwd,
        group_key,
        group: String::new(),
        written,
        broken,
        missing_cwd,
        id: session.id,
    }
}

/// 组头：家族探测成功就用 `--show-toplevel` 的目录名，失败就按 cwd 字符串各成一组、取目录名。
fn label_groups(rows: &mut [SessionRow]) {
    let mut labels: HashMap<String, String> = HashMap::new();
    for row in rows.iter() {
        labels.entry(row.group_key.clone()).or_insert_with(|| {
            if let Some(cwd) = &row.cwd {
                if let Some(top) = git_toplevel(cwd) {
                    return top;
                }
                return Path::new(cwd)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| cwd.clone());
            }
            "（读不出工作目录）".to_owned()
        });
    }
    // 重名的组头写尾两段路径：两个都叫 `heng` 的检出得能分开。
    let mut counts: HashMap<String, usize> = HashMap::new();
    for label in labels.values() {
        *counts.entry(label.clone()).or_default() += 1;
    }
    for row in rows.iter_mut() {
        let label = labels.get(&row.group_key).cloned().unwrap_or_default();
        if counts.get(&label).copied().unwrap_or(0) > 1
            && let Some(cwd) = &row.cwd
        {
            row.group = tail_two(cwd);
            continue;
        }
        row.group = label;
    }
}

/// 尾两段路径：`/a/b/c` → `b/c`。画不出来的（只有一段）就原样。
fn tail_two(path: &str) -> String {
    let parts: Vec<String> = Path::new(path)
        .components()
        .filter_map(|part| match part {
            std::path::Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    match parts.len() {
        0 => path.to_owned(),
        1 => parts[0].clone(),
        _ => format!("{}/{}", parts[parts.len() - 2], parts[parts.len() - 1]),
    }
}

/// 排序（spec §3）：前台所在的检出组钉组顶 → 组间 / 组内 mtime 降序 → id 降序兜底 →
/// 读不出 mtime 的排最后。
fn sort_rows(rows: &mut [SessionRow], foreground: &SessionId) {
    let pinned = rows
        .iter()
        .find(|row| &row.id == foreground)
        .map(|row| row.group_key.clone());
    rows.sort_by(|a, b| {
        let pin = |row: &SessionRow| u8::from(Some(&row.group_key) != pinned.as_ref());
        // 前台那一行也在组内置顶 —— spec §3 的「前台永远可见（它在钉顶那组的第一行）」要
        // 求的正是这个，光钉组不够。
        let front = |row: &SessionRow| u8::from(!row.foreground);
        pin(a)
            .cmp(&pin(b))
            .then_with(|| front(a).cmp(&front(b)))
            .then_with(|| compare_by_time(a, b))
    });
}

/// 只按时间排：mtime 降序 → id 降序兜底 → 读不出 mtime 的最后。
///
/// 过滤态下用的就是它 —— 决策票 10 §3 定了「过滤态下取消钉顶」，排序回到这一档。
pub fn sort_by_time(rows: &mut [SessionRow]) {
    rows.sort_by(compare_by_time);
}

fn compare_by_time(a: &SessionRow, b: &SessionRow) -> std::cmp::Ordering {
    match (a.written, b.written) {
        (Some(left), Some(right)) => right.cmp(&left).then_with(|| b.id.cmp(&a.id)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => b.id.cmp(&a.id),
    }
}

/// 一个 cwd 落在哪个检出家族里：`--git-common-dir` 的绝对路径就是家族键（同一棵检出的所有
/// worktree 共享它）。不是仓库、没有 `git`、命令失败一律答 `None`。
fn git_family(cwd: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let common = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!common.is_empty()).then_some(common)
}

/// 家族那个检出的顶层目录名，用作组头。
fn git_toplevel(cwd: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let top = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    Path::new(&top)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}
