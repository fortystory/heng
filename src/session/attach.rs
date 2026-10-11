//! 装载一场磁盘上的会话：能**立即**判定的几条检查，以及流坏了到什么程度
//! （`.scratch/session-tab/spec.md` §2、决策票 04 §9 与票 05）。
//!
//! 为什么要有这一层：`assemble` 里的沙箱探测与技能发现不是一次点击能同步承担的，而
//! 「已经是前台」「工作区不在了」「流读不出来」三件事**现在**就知道 —— 它们不该排在一次
//! 装载后面才说。

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::events::SessionId;
use crate::session::host::SessionHost;

/// 一条流坏到了什么程度。三档对应三种落点：可写 / 只读预览 / 拒装。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamHealth {
    /// 全部可读。**尾行坏也算这一档**：`EventLog::open` 会把最后那条坏尾巴截掉，于是它写得了。
    Writable,
    /// 半读：坏行**之后还有好行**。装起来只能看 —— 写下去会把洞留在中间。
    ReadOnly {
        /// 坏了几行。
        broken_lines: usize,
    },
    /// 一行都读不出，或者文件根本打不开。**拒装**。
    Unreadable(String),
}

/// 装载前那几条能立即判定的检查。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachCheck {
    /// 可以装：交给装载 task（票 18 §2）。
    Ready,
    /// 它已经在宿主表里了 —— 零延迟取回，**绝不重走装载**。
    ///
    /// 重走一次会让 `resuming` 闸门把这条流上**正在进行**的工具调用读成崩溃遗留，合成一条
    /// 「结果未知」——那是在改一场活着的会话的账。
    AlreadyHosted(SessionId),
    /// 工作区不在了。
    MissingCwd(PathBuf),
    /// 流读不出。
    Unreadable(String),
}

/// 这条流坏到了哪一档。
///
/// 判据是**坏行的位置**而不是坏行的数量：坏行只在末尾时那条尾巴本来就会被 `EventLog::open`
/// 截掉（于是可写），坏行后面还有好行才是真正的洞。
pub fn health(log_path: &Path) -> StreamHealth {
    let file = match File::open(log_path) {
        Ok(file) => file,
        Err(error) => return StreamHealth::Unreadable(error.to_string()),
    };
    let reader = BufReader::new(file);
    let mut total = 0usize;
    let mut broken = 0usize;
    let mut broken_before_good = false;
    let mut seen_broken = false;
    for line in reader.lines() {
        let Ok(line) = line else {
            return StreamHealth::Unreadable("读到一半出错".to_owned());
        };
        if line.trim().is_empty() {
            continue;
        }
        total += 1;
        if serde_json::from_str::<serde_json::Value>(&line).is_ok() {
            if seen_broken {
                broken_before_good = true;
            }
        } else {
            broken += 1;
            seen_broken = true;
        }
    }
    if total == 0 {
        return StreamHealth::Unreadable("这条流一行都没有".to_owned());
    }
    if broken == 0 {
        return StreamHealth::Writable;
    }
    if broken_before_good {
        return StreamHealth::ReadOnly {
            broken_lines: broken,
        };
    }
    // 坏行只在尾巴上：那一条会被 `EventLog::open` 截掉。
    StreamHealth::Writable
}

/// 装载前那几条立即判定。
///
/// `cwd` 是流上记着的那个工作目录（读不出就是 `None`）—— 「工作区还在不在」问的是它，
/// 不是当前进程的 cwd。
pub fn check(
    host: &SessionHost,
    id: &SessionId,
    cwd: Option<&Path>,
    log_path: &Path,
) -> AttachCheck {
    if host.contains(id) {
        return AttachCheck::AlreadyHosted(id.clone());
    }
    if let Some(cwd) = cwd
        && !cwd.is_dir()
    {
        return AttachCheck::MissingCwd(cwd.to_path_buf());
    }
    match health(log_path) {
        StreamHealth::Unreadable(reason) => AttachCheck::Unreadable(reason),
        // 可写与半读都装得起来：半读那一档由调用方标成**只读预览**。
        StreamHealth::Writable | StreamHealth::ReadOnly { .. } => AttachCheck::Ready,
    }
}
