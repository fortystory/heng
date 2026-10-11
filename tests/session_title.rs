//! 会话标题：权威链、前缀轻路径与撤回
//! （`.scratch/session-tab/spec.md` §4、决策票 01）。

use std::path::{Path, PathBuf};

use heng::events::{EventLog, EventPayload, Role, SCHEMA_VERSION, SessionId, SpeakerId};
use heng::session::titles::read_prefix;
use heng::session::{TitleCache, derive, fallback_title, normalize, title_of};

/// 一条流，事件按给的顺序落进去。
fn log_with(dir: &Path, events: Vec<EventPayload>) -> PathBuf {
    let path = dir.join("log.jsonl");
    let mut log = EventLog::create(&path).unwrap();
    for payload in events {
        log.append(SpeakerId::System, payload).unwrap();
    }
    path
}

fn started(id: &str, cwd: &str) -> EventPayload {
    EventPayload::SessionStarted {
        session_id: SessionId::new(id),
        cwd: cwd.to_owned(),
        schema_version: SCHEMA_VERSION,
        model: None,
        provider: None,
        effort: None,
        permission_mode: None,
        sandbox_mode: None,
    }
}

fn user_says(text: &str) -> EventPayload {
    EventPayload::MessageCompleted {
        role: Role::User,
        text: text.to_owned(),
        reasoning: None,
        first_token_ms: None,
    }
}

fn assistant_says(text: &str) -> EventPayload {
    EventPayload::MessageCompleted {
        role: Role::Assistant,
        text: text.to_owned(),
        reasoning: None,
        first_token_ms: None,
    }
}

fn titled(title: &str) -> EventPayload {
    EventPayload::SessionTitled {
        title: title.to_owned(),
    }
}

const ID: &str = "20261012T061010Z-abcd1234";

#[test]
fn the_prefix_carries_the_cwd_and_the_first_user_message() {
    let dir = tempfile::tempdir().unwrap();
    let path = log_with(
        dir.path(),
        vec![started(ID, "/x/heng"), user_says("帮我改一下左栏")],
    );

    let prefix = read_prefix(&path).unwrap();
    assert_eq!(prefix.cwd.as_deref(), Some("/x/heng"));
    assert_eq!(prefix.first_user_text.as_deref(), Some("帮我改一下左栏"));
}

#[test]
fn a_user_message_beyond_the_prefix_window_does_not_count() {
    // 前 8 行是派生的窗口；第 9 行的人话够不着（决策票 01 §2 的实测：p90 = 7 行）。
    let dir = tempfile::tempdir().unwrap();
    let mut events = vec![started(ID, "/x/heng")];
    for index in 0..7 {
        events.push(assistant_says(&format!("第 {index} 段")));
    }
    events.push(user_says("太远了"));
    let path = log_with(dir.path(), events);

    let prefix = read_prefix(&path).unwrap();
    assert_eq!(prefix.first_user_text, None);
    assert_eq!(
        derive(&SessionId::new(ID), &path),
        fallback_title(&SessionId::new(ID))
    );
}

#[test]
fn a_manual_title_beats_the_derived_one() {
    let dir = tempfile::tempdir().unwrap();
    let path = log_with(
        dir.path(),
        vec![
            started(ID, "/x/heng"),
            user_says("第一句话"),
            titled("左栏会话页"),
        ],
    );

    assert_eq!(derive(&SessionId::new(ID), &path), "左栏会话页");
}

#[test]
fn an_empty_title_withdraws_the_manual_one() {
    // 空串即撤回：这一条之后不再有手动标题，回落派生。撤回也必须写下这一条。
    let dir = tempfile::tempdir().unwrap();
    let path = log_with(
        dir.path(),
        vec![
            started(ID, "/x/heng"),
            user_says("第一句话"),
            titled("左栏会话页"),
            titled(""),
        ],
    );

    assert_eq!(derive(&SessionId::new(ID), &path), "第一句话");
}

#[test]
fn the_last_manual_title_wins() {
    let dir = tempfile::tempdir().unwrap();
    let path = log_with(
        dir.path(),
        vec![
            started(ID, "/x/heng"),
            user_says("第一句话"),
            titled("先叫这个"),
            titled("后来改的"),
        ],
    );

    assert_eq!(derive(&SessionId::new(ID), &path), "后来改的");
}

#[test]
fn without_any_human_words_the_fallback_is_the_id_stamp() {
    let dir = tempfile::tempdir().unwrap();
    let path = log_with(dir.path(), vec![started(ID, "/x/heng")]);

    let title = derive(&SessionId::new(ID), &path);
    assert_eq!(title, fallback_title(&SessionId::new(ID)));
    assert!(
        title.len() == "MM-DD HH:mm".len() && title.contains(':'),
        "兜底是 `MM-DD HH:mm`：{title}"
    );
}

#[test]
fn a_long_stream_still_finds_the_title_at_its_tail() {
    // 改名落点在流尾附近，而尾扫只读最后 8 KiB —— 一条上万行的流也要找得到它。
    let dir = tempfile::tempdir().unwrap();
    let mut events = vec![started(ID, "/x/heng"), user_says("第一句话")];
    for index in 0..3_000 {
        events.push(assistant_says(&format!("很长的一段 {index}")));
    }
    events.push(titled("尾改的名"));
    let path = log_with(dir.path(), events);

    assert_eq!(derive(&SessionId::new(ID), &path), "尾改的名");
}

#[test]
fn normalize_takes_the_first_line_and_squeezes_whitespace() {
    assert_eq!(
        normalize("  帮我改 一下 \n第二行"),
        Some("帮我改 一下".to_owned())
    );
    assert_eq!(normalize("带\u{7}控制字符"), Some("带控制字符".to_owned()));
    assert_eq!(normalize("   "), None);
    assert_eq!(normalize(""), None);
    assert_eq!(normalize("\n\n"), None);

    // 上限按字符数，不按列。
    let long = "字".repeat(500);
    assert_eq!(normalize(&long).unwrap().chars().count(), 200);
}

#[test]
fn the_cache_answers_second_time_without_touching_the_disk() {
    let dir = tempfile::tempdir().unwrap();
    let path = log_with(
        dir.path(),
        vec![started(ID, "/x/heng"), user_says("第一次")],
    );
    let id = SessionId::new(ID);
    let mut cache = TitleCache::new();

    assert_eq!(title_of(&id, &path, &mut cache), "第一次");

    // 不做失效判定：一次运行里流没变，派生结果就没变。改名那条路径自己覆盖缓存。
    let mut log = EventLog::open(&path).unwrap();
    log.append(SpeakerId::User, titled("后来改的")).unwrap();
    assert_eq!(title_of(&id, &path, &mut cache), "第一次");

    cache.forget(&id);
    assert_eq!(title_of(&id, &path, &mut cache), "后来改的");
}

/// 实测：本机真实 store 上，前缀派生一整轮多少毫秒。
///
/// 决策票 01 §4 记的是 281 场 3.7–4.8 ms（对照全量 `summarize` 155.6–172.2 ms）。它读的是
/// 本机真实的 store，所以默认 `#[ignore]`；复现命令：
/// `cargo test --test session_title -- --ignored --nocapture`。
#[test]
#[ignore]
fn measure_prefix_derivation_over_the_real_store() {
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let root = PathBuf::from(home).join(".local/share/heng/sessions");
    let store = heng::session::SessionStore::new(&root);
    let Ok(sessions) = store.list_all() else {
        return;
    };
    if sessions.is_empty() {
        println!("本机 store 里没有会话");
        return;
    }
    let head = std::time::Instant::now();
    let prefixes: Vec<_> = sessions
        .iter()
        .map(|session| read_prefix(&session.log_path).unwrap_or_default())
        .collect();
    let head_elapsed = head.elapsed();
    let tail = std::time::Instant::now();
    for session in &sessions {
        let _ = heng::session::titles::last_title(&session.log_path);
    }
    let tail_elapsed = tail.elapsed();
    println!(
        "分段：前缀读 {head_elapsed:?}，尾扫 {tail_elapsed:?}（{} 条前缀命中用户消息）",
        prefixes
            .iter()
            .filter(|prefix| prefix.first_user_text.is_some())
            .count()
    );
    let started = std::time::Instant::now();
    let titles: Vec<String> = sessions
        .iter()
        .map(|session| derive(&session.id, &session.log_path))
        .collect();
    let elapsed = started.elapsed();
    println!(
        "{} 场，前缀派生一轮 {:?}（平均 {:?}，其中 {} 条落了兜底）",
        sessions.len(),
        elapsed,
        elapsed / sessions.len() as u32,
        titles
            .iter()
            .filter(|title| title.len() == "MM-DD HH:mm".len() && title.contains(':'))
            .count()
    );
}
