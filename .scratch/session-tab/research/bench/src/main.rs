//! 票 03 的一次性基准：列表数据源的读取成本与 git 探测成本。
//!
//! 全部走产品现成代码（`SessionStore::list_all` / `events::read_events` /
//! `observe::summarize`），不 fork、不改产品。三个模式：
//!
//! * `scan <root>` —— 全量解析 vs 只读前缀 vs `summarize` 的耗时、规模、首行/首条用户消息
//!   的行号分布，外加一次「逐行扫到首条用户消息」「metadata 10 万次」的 micro。
//! * `prefix <root>` —— 只做前缀测量（扫一遍，供前缀的单独复核）。
//! * `probe <dir>...` —— 对若干候选 cwd 跑 `git -C <dir> rev-parse ...`，给退出码、耗时、
//!   是否超时。
//!
//! 用法（编译产物进 .cache/，见 run-bench.sh）：
//! `agg-bench scan /home/forty/.local/share/heng/sessions`

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use heng::session::observe;
use heng::session::store::SessionStore;

/// 前缀扫描的行数（票面问「只读前 N 行」的实现代价）。
const PREFIX_LINES: usize = 8;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("scan") => {
            let rounds: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(5);
            scan(Path::new(&args[1]), rounds)
        }
        Some("prefix") => prefix_only(Path::new(&args[1])),
        Some("probe") => probe(&args[1..]),
        Some("tail") => tail(Path::new(&args[1])),
        Some("probe-serial") => {
            let n: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1000);
            let base = args.get(2).cloned().unwrap_or_else(|| "/nonexistent-xyz".to_owned());
            probe_serial(n, &base)
        }
        Some("probe-parallel") => {
            let n: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1000);
            let base = args.get(2).cloned().unwrap_or_else(|| "/nonexistent-xyz".to_owned());
            probe_parallel(n, &base)
        }
        _ => {
            eprintln!("usage: agg-bench scan <root> | prefix <root> | tail <root> | probe <dir>... | probe-serial <n> <base> | probe-parallel <n> <base>");
            std::process::exit(2);
        }
    }
}

/// 一次测量的统计量（毫秒）。
struct Timing {
    rounds: usize,
    best_ms: f64,
    median_ms: f64,
    worst_ms: f64,
}

fn timing(mut values: Vec<f64>) -> Timing {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = values.len();
    Timing {
        rounds: n,
        best_ms: values[0],
        median_ms: values[n / 2],
        worst_ms: values[n - 1],
    }
}

impl Timing {
    fn line(&self, label: &str) {
        println!(
            "{label}: rounds={} best={:.1}ms median={:.1}ms worst={:.1}ms",
            self.rounds, self.best_ms, self.median_ms, self.worst_ms
        );
    }
}

/// 一次 `list_all` 的结果 + 每条流的磁盘事实。
struct Corpus {
    store: SessionStore,
    root: PathBuf,
    sessions: Vec<PathBuf>,
    bytes: Vec<u64>,
    total_bytes: u64,
}

fn corpus(root: &Path) -> Corpus {
    let store = SessionStore::new(root);
    let list = store.list_all().expect("list_all");
    let mut sessions = Vec::new();
    let mut bytes = Vec::new();
    let mut total_bytes = 0u64;
    for session in &list {
        sessions.push(session.log_path.clone());
        let size = std::fs::metadata(&session.log_path).map(|m| m.len()).unwrap_or(0);
        bytes.push(size);
        total_bytes += size;
    }
    Corpus {
        store,
        root: root.to_path_buf(),
        sessions,
        bytes,
        total_bytes,
    }
}

fn scan(root: &Path, rounds: usize) {
    let c = corpus(root);
    println!("root={}", c.root.display());
    println!("list_all={} sessions total_bytes={}", c.sessions.len(), c.total_bytes);

    // ---- 1) `list_all` 本身 ----
    let mut list_all_ms = Vec::new();
    for _ in 0..10.min(rounds * 2) {
        let start = Instant::now();
        let n = c.store.list_all().expect("list_all").len();
        std::hint::black_box(n);
        list_all_ms.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    timing(list_all_ms).line("list_all");

    // ---- 2) 全量 `read_events`（票面第 3 条）----
    let mut full_total = Vec::new();
    let mut full_per_file: Vec<Vec<f64>> = vec![Vec::new(); c.sessions.len()];
    let mut events_total = 0usize;
    let mut errors = 0usize;
    let mut error_kinds: BTreeMap<String, usize> = BTreeMap::new();
    for round in 0..rounds {
        let start = Instant::now();
        for (i, path) in c.sessions.iter().enumerate() {
            let t = Instant::now();
            match heng::events::read_events(path) {
                Ok(events) => {
                    if round == 0 {
                        events_total += events.len();
                    }
                    std::hint::black_box(&events);
                }
                Err(error) => {
                    if round == 0 {
                        errors += 1;
                        *error_kinds.entry(error.to_string()).or_default() += 1;
                    }
                }
            }
            full_per_file[i].push(t.elapsed().as_secs_f64() * 1000.0);
        }
        full_total.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    println!(
        "read_events(all): 首轮 events={events_total}（平均 {:.1} 条/流）errors={errors}",
        events_total as f64 / c.sessions.len() as f64
    );
    for (kind, n) in error_kinds.iter().take(3) {
        println!("  error×{n}: {}", kind.chars().take(200).collect::<String>());
    }
    println!("  （错误种类共 {} 种）", error_kinds.len());
    timing(full_total).line("read_events all files");

    // 每条流的中位耗时（取 5 轮的中位），按耗时排序给出最慢的十条。
    let mut per_file: Vec<(f64, u64, PathBuf)> = c
        .sessions
        .iter()
        .enumerate()
        .map(|(i, path)| {
            let mut v = full_per_file[i].clone();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            (v[v.len() / 2], c.bytes[i], path.clone())
        })
        .collect();
    per_file.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    println!("--- 全量读最慢的 10 条流 ---");
    for (ms, bytes, path) in per_file.iter().take(10) {
        println!("  {:>8.1}ms {:>9}B  {}", ms, bytes, rel(&c.root, path));
    }
    let mut ascending: Vec<f64> = per_file.iter().map(|r| r.0).collect();
    ascending.reverse();
    println!(
        "per-file 中位耗时：p50={:.2}ms p90={:.2}ms max={:.2}ms",
        ascending[ascending.len() / 2],
        ascending[(ascending.len() as f64 * 0.9) as usize],
        ascending[ascending.len() - 1]
    );

    // ---- 3) `observe::summarize`（票面第 4 条）----
    let list = c.store.list_all().expect("list_all");
    let mut summarize_total = Vec::new();
    let mut fields_ok = 0usize;
    let mut cwd_some = 0usize;
    let mut started_some = 0usize;
    let mut summarize_errors = 0usize;
    for round in 0..rounds.min(3) {
        let start = Instant::now();
        for session in &list {
            let Ok(listing) = observe::summarize(session) else {
                if round == 0 {
                    summarize_errors += 1;
                }
                continue;
            };
            if round == 0 {
                if listing.cwd.is_some() {
                    cwd_some += 1;
                }
                if listing.started.is_some() {
                    started_some += 1;
                }
                fields_ok += 1;
            }
            std::hint::black_box(&listing);
        }
        summarize_total.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    timing(summarize_total).line("observe::summarize all files");
    println!(
        "summarize: ok={fields_ok} errors={summarize_errors} cwd_some={cwd_some} started_some={started_some}"
    );

    // ---- 4) 只读前缀 N 行 ----
    let mut prefix_total = Vec::new();
    let mut prefix_hits = 0usize;
    let mut first_line_ok = 0usize;
    let mut line_counts: BTreeMap<u64, usize> = BTreeMap::new();
    let mut first_user_lines: Vec<usize> = Vec::new();
    for round in 0..rounds {
        let start = Instant::now();
        for path in &c.sessions {
            let (cwd, first_user) = prefix_scan(path, PREFIX_LINES);
            if round == 0 {
                if cwd.is_some() {
                    first_line_ok += 1;
                }
                if first_user.is_some() {
                    prefix_hits += 1;
                }
            }
        }
        prefix_total.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    timing(prefix_total).line(&format!("read prefix {PREFIX_LINES} lines"));

    // 全文件逐行扫到首条用户消息（最坏情况的真成本）；行号只在第一轮收集。
    let mut to_user_total = Vec::new();
    for round in 0..rounds.min(3) {
        let start = Instant::now();
        for path in &c.sessions {
            if let Some(line) = first_user_line(path) {
                if round == 0 {
                    first_user_lines.push(line);
                    *line_counts.entry(line as u64).or_default() += 1;
                }
            }
        }
        to_user_total.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    timing(to_user_total).line("scan lines until first user message (all files)");
    first_user_lines.sort_unstable();
    if !first_user_lines.is_empty() {
        let n = first_user_lines.len();
        println!(
            "first_user_line: n={n} min={} p50={} p90={} max={}",
            first_user_lines[0],
            first_user_lines[n / 2],
            first_user_lines[(n as f64 * 0.9) as usize],
            first_user_lines[n - 1]
        );
    }
    println!("prefix({PREFIX_LINES} lines) 内见到首条用户消息的会话数 = {prefix_hits}");
    println!("首行可解析出 cwd 的会话数 = {first_line_ok}");
    println!("first_user_line 直方图（前 12 个取值）= {:?}", line_counts.iter().take(12).collect::<Vec<_>>());

    // ---- 5) 尾部：末行是不是坏行 / 流里有没有 SessionEnded ----
    let mut bad_tail = 0usize;
    let mut ended = 0usize;
    let mut empty = 0usize;
    for path in &c.sessions {
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        if size == 0 {
            empty += 1;
            continue;
        }
        if !file_ends_with_newline(path) {
            bad_tail += 1;
        }
        if tail_has_session_ended(path) {
            ended += 1;
        }
    }
    println!(
        "文件事实: empty={empty} 末字节不是换行={bad_tail} 尾部 4KiB 里有 SessionEnded={ended}"
    );

    // ---- 5b) `SessionStarted.cwd` 在本地是否存在 ----
    let list = c.store.list_all().expect("list_all");
    let mut exists = 0usize;
    let mut missing = 0usize;
    let start = Instant::now();
    for session in &list {
        if let Some(listing) = observe::summarize(session).ok() {
            if let Some(cwd) = listing.cwd {
                if Path::new(&cwd).is_dir() {
                    exists += 1;
                } else {
                    missing += 1;
                }
            }
        }
    }
    println!(
        "cwd 存在性：exists={exists} missing={missing}（含 summarize 的 {:.1}ms）",
        start.elapsed().as_secs_f64() * 1000.0
    );

    // ---- 6) metadata 的成本 ----
    let mut meta_ms = Vec::new();
    for _ in 0..rounds.min(3) {
        let start = Instant::now();
        for _ in 0..100_000 {
            for path in c.sessions.iter().take(4) {
                std::hint::black_box(std::fs::metadata(path).map(|m| m.modified()).ok());
            }
        }
        meta_ms.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    let m = timing(meta_ms);
    println!(
        "metadata()+modified(): 400k 次/轮 rounds={} best={:.1}ms median={:.1}ms → 每次 {:.3}µs",
        m.rounds,
        m.best_ms,
        m.median_ms,
        m.median_ms * 1000.0 / 400_000.0
    );
}

fn prefix_only(root: &Path) {
    let c = corpus(root);
    let mut lines_ms = Vec::new();
    for _ in 0..5 {
        let start = Instant::now();
        for path in &c.sessions {
            std::hint::black_box(prefix_scan(path, PREFIX_LINES));
        }
        lines_ms.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    timing(lines_ms).line("prefix scan");
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// 只读前 `n` 行，答 `(cwd, 首条用户消息的行号)`。
fn prefix_scan(path: &Path, n: usize) -> (Option<String>, Option<usize>) {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return (None, None),
    };
    let mut cwd = None;
    let mut first_user = None;
    for (index, line) in BufReader::new(file).lines().enumerate() {
        if index >= n {
            break;
        }
        let Ok(line) = line else { break };
        // 只反序列化成 `serde_json::Value`（不是 `Event`）：前缀路径的实现代价就在这一步。
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let Some(payload) = value.get("payload").and_then(|p| p.as_object()) else {
            continue;
        };
        if cwd.is_none() {
            if let Some(found) = payload.get("SessionStarted").and_then(|s| s.get("cwd")).and_then(|c| c.as_str()) {
                cwd = Some(found.to_owned());
            }
        }
        if first_user.is_none() {
            if let Some(message) = payload.get("MessageCompleted") {
                if message.get("role").and_then(|r| r.as_str()) == Some("User") {
                    first_user = Some(index + 1);
                }
            }
        }
    }
    // 首行不是 SessionStarted 时 cwd 就是 None：上面只在 `{"seq":` 前缀上偷看。
    (cwd, first_user)
}

/// 逐行（不解析）找到首条 `MessageCompleted{role:User}` 的行号。
fn first_user_line(path: &Path) -> Option<usize> {
    let file = std::fs::File::open(path).ok()?;
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line = line.ok()?;
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let Some(message) = value.get("payload").and_then(|p| p.get("MessageCompleted")) else {
            continue;
        };
        if message.get("role").and_then(|r| r.as_str()) == Some("User") {
            return Some(index + 1);
        }
    }
    None
}

fn file_ends_with_newline(path: &Path) -> bool {
    use std::io::Seek;
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
    if size == 0 {
        return false;
    }
    if file.seek(std::io::SeekFrom::Start(size - 1)).is_err() {
        return false;
    }
    let mut last = [0u8; 1];
    if file.read_exact(&mut last).is_err() {
        return false;
    }
    last[0] == b'\n'
}

/// 尾部 4KiB 里有没有 `"SessionEnded"`（尾部读的成本与可行性）。
fn tail_has_session_ended(path: &Path) -> bool {
    use std::io::Seek;
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
    let start = size.saturating_sub(4096);
    if file.seek(std::io::SeekFrom::Start(start)).is_err() {
        return false;
    }
    let mut buf = Vec::new();
    if file.read_to_end(&mut buf).is_err() {
        return false;
    }
    buf.windows(13).any(|w| w == b"SessionEnded\"")
}

// ---------------------------------------------------------------------------
// git 探测
// ---------------------------------------------------------------------------

/// 一条 `git -C <dir> <args>` 的观测。
struct Probe {
    exit: Option<i32>,
    stdout: String,
    stderr: String,
    ms: f64,
    timed_out: bool,
}

const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

fn run_git(dir: &str, args: &[&str]) -> Probe {
    let start = Instant::now();
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return Probe {
                exit: None,
                stdout: String::new(),
                stderr: format!("spawn 失败: {error}"),
                ms: start.elapsed().as_secs_f64() * 1000.0,
                timed_out: false,
            }
        }
    };
    // 用两个线程收管道，主线程按超时等：避免读管道与等退出互相死锁。
    // 消息带上是哪条流，收的两段才不会被认错。
    enum Stream {
        Out,
        Err,
    }
    let (tx, rx) = mpsc::channel();
    let mut out = child.stdout.take();
    let mut err = child.stderr.take();
    let tx_out = tx.clone();
    std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(handle) = out.as_mut() {
            let _ = handle.read_to_string(&mut buf);
        }
        let _ = tx_out.send((Stream::Out, buf));
    });
    std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(handle) = err.as_mut() {
            let _ = handle.read_to_string(&mut buf);
        }
        let _ = tx.send((Stream::Err, buf));
    });

    let deadline = Instant::now() + PROBE_TIMEOUT;
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    timed_out = true;
                    break None;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(_) => break None,
        }
    };
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    let mut stdout = String::new();
    let mut stderr = String::new();
    while let Ok((stream, buf)) = rx.recv_timeout(Duration::from_millis(200)) {
        match stream {
            Stream::Out => stdout.push_str(&buf),
            Stream::Err => stderr.push_str(&buf),
        }
    }
    Probe {
        exit: status.and_then(|s| s.code()),
        stdout,
        stderr,
        ms,
        timed_out,
    }
}

fn probe(dirs: &[String]) {
    let queries: [(&str, Vec<&str>); 5] = [
        ("rev-parse --git-common-dir", vec!["rev-parse", "--git-common-dir"]),
        ("rev-parse --show-toplevel", vec!["rev-parse", "--show-toplevel"]),
        ("rev-parse --is-inside-work-tree", vec!["rev-parse", "--is-inside-work-tree"]),
        ("rev-parse --abbrev-ref HEAD", vec!["rev-parse", "--abbrev-ref", "HEAD"]),
        ("rev-parse --show-prefix", vec!["rev-parse", "--show-prefix"]),
    ];
    for dir in dirs {
        println!("== {dir} ==");
        for (label, args) in &queries {
            let probe = run_git(dir, args);
            println!(
                "  {label}: exit={:?} {:.1}ms timed_out={} stdout={:?} stderr={:?}",
                probe.exit,
                probe.ms,
                probe.timed_out,
                probe.stdout.trim(),
                probe.stderr.trim().chars().take(160).collect::<String>()
            );
        }
    }
}

/// 尾部前缀键：只读每条流末尾 4 KiB，解析最后一条完整事件的 payload 键。
fn tail(root: &Path) {
    use std::io::Seek;
    let c = corpus(root);
    let mut hit = 0usize;
    let mut miss = 0usize;
    let mut unreadable = 0usize;
    let mut keys: BTreeMap<String, usize> = BTreeMap::new();
    let mut elapsed = Vec::new();
    for _ in 0..5 {
        let start = Instant::now();
        hit = 0;
        miss = 0;
        unreadable = 0;
        keys.clear();
        for path in &c.sessions {
            let Ok(mut file) = std::fs::File::open(path) else {
                unreadable += 1;
                continue;
            };
            let size = file.metadata().map(|m| m.len()).unwrap_or(0);
            let window = 4096u64.min(size);
            if file.seek(std::io::SeekFrom::Start(size - window)).is_err() {
                unreadable += 1;
                continue;
            }
            let mut buf = Vec::new();
            if file.read_to_end(&mut buf).is_err() {
                unreadable += 1;
                continue;
            }
            let text = String::from_utf8_lossy(&buf);
            let mut found = false;
            for line in text.lines().rev() {
                let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
                    continue;
                };
                let Some(payload) = value.get("payload").and_then(|p| p.as_object()) else {
                    continue;
                };
                for key in payload.keys() {
                    *keys.entry(key.clone()).or_default() += 1;
                }
                if payload.contains_key("SessionEnded") {
                    found = true;
                }
                break;
            }
            if found {
                hit += 1;
            } else {
                miss += 1;
            }
        }
        elapsed.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    timing(elapsed).line("tail 4KiB × all files");
    println!("末条完整事件是 SessionEnded 的会话数 = {hit}，不是 = {miss}，读不到 = {unreadable}");
    let mut sorted: Vec<_> = keys.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));
    println!("末条事件的类型分布（前 10）= {:?}", sorted.iter().take(10).collect::<Vec<_>>());
}

/// 只取退出码，不量 stdout：串行 n 次 `git -C <base> rev-parse --git-common-dir`。
fn probe_serial(n: usize, base: &str) {
    let mut elapsed = Vec::new();
    let mut ok = 0usize;
    let mut spawn_fail = 0usize;
    for _ in 0..5 {
        let start = Instant::now();
        ok = 0;
        spawn_fail = 0;
        for _ in 0..n {
            match Command::new("git")
                .arg("-C")
                .arg(base)
                .arg("rev-parse")
                .arg("--git-common-dir")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
            {
                Ok(status) if status.success() => ok += 1,
                Ok(_) => {}
                Err(_) => spawn_fail += 1,
            }
        }
        elapsed.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    let t = timing(elapsed);
    println!(
        "串行 git -C {base} rev-parse --git-common-dir：n={n} ok={ok} spawn_fail={spawn_fail}"
    );
    t.line("  总耗时");
    println!(
        "  每次 {:.2}ms → 100 个候选 {:.1}ms，1000 个候选 {:.1}ms",
        t.median_ms / n as f64,
        t.median_ms / n as f64 * 100.0,
        t.median_ms / n as f64 * 1000.0
    );
}

/// 用 N 个线程跑同样的探测，看池化的收益。
fn probe_parallel(n: usize, base: &str) {
    for jobs in [1usize, 2, 4, 8, 16] {
        let start = Instant::now();
        let next = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let done = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut handles = Vec::new();
        for _ in 0..jobs {
            let next = next.clone();
            let done = done.clone();
            let base = base.to_owned();
            handles.push(std::thread::spawn(move || {
                loop {
                    let index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if index >= n {
                        break;
                    }
                    let status = Command::new("git")
                        .arg("-C")
                        .arg(&base)
                        .args(["rev-parse", "--git-common-dir"])
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status();
                    if status.map(|s| s.success()).unwrap_or(false) {
                        done.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            }));
        }
        for handle in handles {
            let _ = handle.join();
        }
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        println!(
            "  并发 {jobs:>2} 线程：{ms:.1}ms（{:.2}ms/次，ok={}）",
            ms / n as f64,
            done.load(std::sync::atomic::Ordering::Relaxed)
        );
    }
}
