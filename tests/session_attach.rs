//! 会话级事实的派生，以及装载前的立即检查与流的三档健康
//! （`.scratch/session-tab/spec.md` §2、票 18）。

use std::path::Path;

use heng::events::{Event, EventPayload, SCHEMA_VERSION, SessionId, SpeakerId};
use heng::session::attach::{check, health};
use heng::session::host::{SessionCommand, SessionHost};
use heng::session::{AttachCheck, SessionFacts, StreamHealth};

fn event(seq: u64, payload: EventPayload) -> Event {
    Event::new(seq, SpeakerId::System, payload)
}

fn started(model: Option<&str>) -> EventPayload {
    EventPayload::SessionStarted {
        session_id: SessionId::new("s-1"),
        cwd: "/workspace".to_owned(),
        schema_version: SCHEMA_VERSION,
        model: model.map(str::to_owned),
        provider: Some("deepseek".to_owned()),
        effort: Some("high".to_owned()),
        permission_mode: Some("ask".to_owned()),
        sandbox_mode: Some("bwrap".to_owned()),
    }
}

/// 一条**旧格式**的 `SessionStarted`：五格全缺。
fn started_bare() -> EventPayload {
    EventPayload::SessionStarted {
        session_id: SessionId::new("s-1"),
        cwd: "/workspace".to_owned(),
        schema_version: SCHEMA_VERSION,
        model: None,
        provider: None,
        effort: None,
        permission_mode: None,
        sandbox_mode: None,
    }
}

fn snapshot(model: &str) -> EventPayload {
    EventPayload::SessionFactsChanged {
        model: Some(model.to_owned()),
        provider: Some("kimi".to_owned()),
        effort: None,
        permission_mode: Some("auto".to_owned()),
        sandbox_mode: Some("off".to_owned()),
    }
}

#[test]
fn the_last_snapshot_beats_the_opening_record() {
    let events = vec![
        event(1, started(Some("old-model"))),
        event(2, snapshot("new-model")),
    ];
    let facts = SessionFacts::current(&events);
    assert_eq!(
        facts.model.as_deref(),
        Some("new-model"),
        "当前事实取最后一条"
    );
    assert_eq!(facts.provider.as_deref(), Some("kimi"));
    assert_eq!(facts.permission_mode.as_deref(), Some("auto"));
    assert_eq!(facts.effort, None, "快照里那一格就是空的");
}

#[test]
fn without_a_snapshot_it_falls_back_to_the_opening_record() {
    let events = vec![event(1, started(Some("only-model")))];
    let facts = SessionFacts::current(&events);
    assert_eq!(facts.model.as_deref(), Some("only-model"));
    assert_eq!(facts.sandbox_mode.as_deref(), Some("bwrap"));
}

#[test]
fn a_stream_with_neither_answers_nothing() {
    let facts = SessionFacts::current(&[]);
    assert_eq!(facts, SessionFacts::default());
    assert_eq!(facts.missing().len(), 5);
}

#[test]
fn an_old_stream_reports_which_facts_were_filled_in() {
    // 旧流：五个字段全缺。
    let old = SessionFacts::current(&[event(1, started_bare())]);
    let (filled, names) = old.with_fallback(&SessionFacts {
        model: Some("fallback-model".to_owned()),
        provider: Some("fallback-provider".to_owned()),
        effort: Some("low".to_owned()),
        permission_mode: Some("ask".to_owned()),
        sandbox_mode: Some("off".to_owned()),
    });
    assert_eq!(filled.model.as_deref(), Some("fallback-model"));
    assert_eq!(names.len(), 5, "如实报出补了哪几样：{names:?}");

    // 半缺：只有 model 缺，别的照用流上那份。
    let partial = SessionFacts {
        model: None,
        provider: Some("from-stream".to_owned()),
        effort: Some("high".to_owned()),
        permission_mode: Some("ask".to_owned()),
        sandbox_mode: Some("bwrap".to_owned()),
    };
    let (filled, names) = partial.with_fallback(&SessionFacts {
        model: Some("fallback-model".to_owned()),
        provider: Some("ignored".to_owned()),
        ..SessionFacts::default()
    });
    assert_eq!(
        filled.provider.as_deref(),
        Some("from-stream"),
        "补是逐格的"
    );
    assert_eq!(names, vec!["模型"]);
}

fn write_log(dir: &Path, lines: &[&str]) -> std::path::PathBuf {
    let path = dir.join("log.jsonl");
    std::fs::write(&path, lines.join("\n")).unwrap();
    path
}

fn good_line(seq: u64) -> String {
    serde_json::to_string(&event(seq, started(Some("m")))).unwrap()
}

#[test]
fn a_sound_stream_is_writable() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_log(dir.path(), &[&good_line(1), &good_line(2)]);
    assert_eq!(health(&path), StreamHealth::Writable);
}

#[test]
fn a_broken_tail_is_still_writable() {
    // 尾行坏：`EventLog::open` 会把那一条截掉，于是这一场写得了。
    let dir = tempfile::tempdir().unwrap();
    let path = write_log(dir.path(), &[&good_line(1), "这不是一行 JSON"]);
    assert_eq!(health(&path), StreamHealth::Writable);
}

#[test]
fn a_hole_in_the_middle_is_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_log(dir.path(), &[&good_line(1), "坏的", &good_line(3)]);
    assert_eq!(
        health(&path),
        StreamHealth::ReadOnly { broken_lines: 1 },
        "坏行后面还有好行 = 真正的洞"
    );
}

#[test]
fn a_stream_that_reads_nothing_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let empty = write_log(dir.path(), &[]);
    assert!(matches!(health(&empty), StreamHealth::Unreadable(_)));
    let missing = dir.path().join("nope.jsonl");
    assert!(matches!(health(&missing), StreamHealth::Unreadable(_)));
}

#[test]
fn the_checks_answer_before_anything_is_loaded() {
    let dir = tempfile::tempdir().unwrap();
    let host = SessionHost::<SessionCommand>::new(SessionId::new("s-other"));
    let id = SessionId::new("s-1");
    let workspace = dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let log = write_log(dir.path(), &[&good_line(1)]);

    assert_eq!(
        check(&host, &id, Some(&workspace), &log),
        AttachCheck::Ready
    );
    // 工作区不在了：立即说得出，不必等一次组装。
    assert_eq!(
        check(&host, &id, Some(&dir.path().join("gone")), &log),
        AttachCheck::MissingCwd(dir.path().join("gone"))
    );
    // 流读不出：拒装。
    let broken = write_log(dir.path(), &[]);
    assert!(matches!(
        check(&host, &id, Some(&workspace), &broken),
        AttachCheck::Unreadable(_)
    ));
}
