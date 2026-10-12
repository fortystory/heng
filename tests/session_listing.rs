//! 会话页的列表：前缀轻路径、mtime、检出分组与排序
//! （`.scratch/session-tab/spec.md` §3、§4）。
//!
//! 这一批用例刻意**不碰 git**：临时目录不是仓库，家族探测因此失败，而那正是「按 cwd 字符串
//! 各成一组」那一档要覆盖的分支。

use std::collections::HashMap;
use std::path::Path;

use heng::events::{EventLog, EventPayload, Role, SCHEMA_VERSION, SessionId, SpeakerId};
use heng::session::listing::{SessionMark, sessions};
use heng::session::{SessionStore, TitleCache};

/// 一个 store，外加它扎在哪。
struct Fixture {
    _dir: tempfile::TempDir,
    store: SessionStore,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionStore::new(dir.path().join("sessions"));
    Fixture { _dir: dir, store }
}

/// 在 `cwd` 那条桶里造一个会话，写一条 `SessionStarted` 与一句人话。
fn session(store: &SessionStore, cwd: &Path, first_words: Option<&str>) -> SessionId {
    let stored = store.create(cwd).unwrap();
    let mut log = EventLog::create(&stored.log_path).unwrap();
    log.append(
        SpeakerId::System,
        EventPayload::SessionStarted {
            session_id: stored.id.clone(),
            cwd: cwd.to_string_lossy().into_owned(),
            schema_version: SCHEMA_VERSION,
            model: None,
            provider: None,
            effort: None,
            permission_mode: None,
            sandbox_mode: None,
        },
    )
    .unwrap();
    if let Some(text) = first_words {
        log.append(
            SpeakerId::System,
            EventPayload::MessageCompleted {
                role: Role::User,
                text: text.to_owned(),
                reasoning: None,
                first_token_ms: None,
            },
        )
        .unwrap();
    }
    stored.id
}

fn list(store: &SessionStore, foreground: &SessionId) -> heng::session::SessionListing {
    sessions(store, &mut TitleCache::new(), foreground, &HashMap::new())
}

#[test]
fn an_empty_store_is_not_an_unreadable_one() {
    let fixture = fixture();
    let listing = list(&fixture.store, &SessionId::new("nobody"));
    assert!(listing.rows.is_empty());
    assert_eq!(listing.total, 0);
    assert!(!listing.unreadable, "空 store 与读不出是不同的两件事");
}

#[test]
fn a_row_carries_its_cwd_title_and_time() {
    let fixture = fixture();
    let dir = tempfile::tempdir().unwrap();
    let id = session(&fixture.store, dir.path(), Some("帮我改一下左栏"));

    let listing = list(&fixture.store, &id);
    assert_eq!(listing.rows.len(), 1);
    let row = &listing.rows[0];
    assert_eq!(row.id, id);
    assert_eq!(row.cwd.as_deref(), Some(dir.path().to_str().unwrap()));
    assert_eq!(row.title, "帮我改一下左栏");
    assert!(row.written.is_some(), "每一行都该有时间");
    assert!(!row.broken);
    assert!(!row.missing_cwd);
    assert!(row.foreground, "前台那一行自己带标记");
}

#[test]
fn the_newest_session_in_a_group_comes_first() {
    let fixture = fixture();
    let dir = tempfile::tempdir().unwrap();
    let older = session(&fixture.store, dir.path(), Some("先的那个会话"));
    // mtime 的先后是这一条用例的判据，而两次写入可能落在同一刻 —— 显式拉开一点。
    std::thread::sleep(std::time::Duration::from_millis(20));
    let newer = session(&fixture.store, dir.path(), Some("后的一个会话"));

    let listing = list(&fixture.store, &SessionId::new("nobody"));
    let ids: Vec<&SessionId> = listing.rows.iter().map(|row| &row.id).collect();
    assert_eq!(ids, vec![&newer, &older], "组内 mtime 降序");

    // 前台不在自己那一组时，组序仍按 mtime；它在时那一组钉顶。
    let pinned = list(&fixture.store, &older);
    assert_eq!(pinned.rows[0].id, older);
}

#[test]
fn the_foreground_group_is_pinned_to_the_top() {
    let fixture = fixture();
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    // 先写 first 那一组（更旧），再写 second（更新）—— 不钉顶时 second 在前。
    let older = session(&fixture.store, first.path(), Some("旧组"));
    std::thread::sleep(std::time::Duration::from_millis(20));
    let newer = session(&fixture.store, second.path(), Some("新组"));

    let plain = list(&fixture.store, &SessionId::new("nobody"));
    assert_eq!(plain.rows[0].id, newer);

    let pinned = list(&fixture.store, &older);
    assert_eq!(pinned.rows[0].id, older, "前台所在的检出组钉在组顶");
}

#[test]
fn a_group_header_is_the_directory_name_and_repeats_are_disambiguated() {
    let fixture = fixture();
    let root = tempfile::tempdir().unwrap();
    let left = root.path().join("a").join("heng");
    let right = root.path().join("b").join("heng");
    std::fs::create_dir_all(&left).unwrap();
    std::fs::create_dir_all(&right).unwrap();
    session(&fixture.store, &left, Some("左边那个会话"));
    session(&fixture.store, &right, Some("右边那个会话"));

    let listing = list(&fixture.store, &SessionId::new("nobody"));
    let groups: Vec<&str> = listing.rows.iter().map(|row| row.group.as_str()).collect();
    assert!(
        groups.iter().all(|group| group.ends_with("heng")),
        "重名的组头写尾两段路径：{groups:?}"
    );
    assert!(
        groups
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == 2,
        "两棵不同的检出不能合成一个组：{groups:?}"
    );
}

#[test]
fn a_broken_stream_marks_only_its_own_row() {
    let fixture = fixture();
    let good_dir = tempfile::tempdir().unwrap();
    let good = session(&fixture.store, good_dir.path(), Some("好流"));
    let bad_dir = tempfile::tempdir().unwrap();
    let bad = session(&fixture.store, bad_dir.path(), Some("坏流"));
    // 把坏流那一份日志换成垃圾：头 8 行一行 JSON 都解析不出来，那一行因此标「流损坏」。
    let stored = fixture
        .store
        .list_all()
        .unwrap()
        .into_iter()
        .find(|stored| stored.id == bad)
        .unwrap();
    std::fs::write(&stored.log_path, "这不是一行 JSON\n也不是\n").unwrap();

    let listing = list(&fixture.store, &SessionId::new("nobody"));
    let broken: Vec<&SessionId> = listing
        .rows
        .iter()
        .filter(|row| row.broken)
        .map(|row| &row.id)
        .collect();
    assert_eq!(broken, vec![&bad], "坏的只是那一行");
    assert!(
        listing.rows.iter().any(|row| row.id == good && !row.broken),
        "好流照常"
    );
}

#[test]
fn a_moved_away_workspace_shows_up_as_missing() {
    let fixture = fixture();
    let dir = tempfile::tempdir().unwrap();
    let id = session(&fixture.store, dir.path(), Some("搬走的那个会话"));
    let gone = dir.path().to_path_buf();
    drop(dir);
    assert!(!gone.is_dir(), "临时目录真的没了");

    let listing = list(&fixture.store, &id);
    assert!(listing.rows[0].missing_cwd, "cwd 不在的那一行自己说");
}

#[test]
fn the_host_mark_rides_along_on_the_row() {
    let fixture = fixture();
    let dir = tempfile::tempdir().unwrap();
    let id = session(&fixture.store, dir.path(), Some("待答的那个会话"));

    let mut marks = HashMap::new();
    marks.insert(id.clone(), SessionMark::Awaiting);
    let listing = sessions(
        &fixture.store,
        &mut TitleCache::new(),
        &SessionId::new("nobody"),
        &marks,
    );
    assert_eq!(listing.rows[0].mark, SessionMark::Awaiting);
}

#[test]
fn listing_never_writes_to_the_stream() {
    // 列表与派生都是只读动作：`EventLog::open` 会补换行、截断坏尾巴，也就是改文件。
    let fixture = fixture();
    let dir = tempfile::tempdir().unwrap();
    let id = session(&fixture.store, dir.path(), Some("别动我"));
    let stored = fixture
        .store
        .list_all()
        .unwrap()
        .into_iter()
        .find(|stored| stored.id == id)
        .unwrap();
    let before = std::fs::read(&stored.log_path).unwrap();
    let len_before = std::fs::metadata(&stored.log_path).unwrap().len();

    let _ = list(&fixture.store, &id);

    let after = std::fs::read(&stored.log_path).unwrap();
    assert_eq!(before, after, "一个字节都不该变");
    assert_eq!(
        len_before,
        std::fs::metadata(&stored.log_path).unwrap().len()
    );
}
