//! 进程级的路径锁表：同 cwd 共享一把、异 cwd 各一把
//! （`.scratch/session-tab/spec.md` §1、票 13）。
//!
//! 这两条合起来才是那把锁的意思：同一个工作区里两个执行者不能交错写同一个文件，而两个
//! **不同**工作区的会话不该因为那把工作区级的 `exclusive` 互相阻塞。

use std::path::Path;
use std::time::Duration;

use heng::tools::PathLockTable;

fn lock_path(root: &Path) -> std::path::PathBuf {
    root.join("f.txt")
}

#[tokio::test]
async fn the_same_cwd_shares_one_lock_table() {
    let dir = tempfile::tempdir().unwrap();
    let table = PathLockTable::new();

    // 同一个目录的两种写法（带 `.`）必须落到同一行 —— 键取的是规范化之后的 cwd。
    let first = table.locks(dir.path());
    let second = table.locks(&dir.path().join("."));

    let held = first.lock(&lock_path(dir.path())).await;
    let waiting = tokio::time::timeout(
        Duration::from_millis(80),
        second.lock(&lock_path(dir.path())),
    )
    .await;
    assert!(waiting.is_err(), "同 cwd 的第二次请求要排在同一把锁后面");
    drop(held);

    // 放开之后它立刻拿得到：那一把确实是同一把，而不是永远拿不到。
    tokio::time::timeout(
        Duration::from_millis(500),
        second.lock(&lock_path(dir.path())),
    )
    .await
    .expect("前一个写入者放开之后就该拿到");
}

#[tokio::test]
async fn different_cwds_never_block_each_other() {
    let first_dir = tempfile::tempdir().unwrap();
    let second_dir = tempfile::tempdir().unwrap();
    let table = PathLockTable::new();

    let first = table.locks(first_dir.path());
    let second = table.locks(second_dir.path());

    // 工作区级那把 `exclusive` 是**按检出**算的：另一个 cwd 的会话不该排在它后面。
    let _held = first.lock_exclusive().await;
    tokio::time::timeout(Duration::from_millis(500), second.lock_exclusive())
        .await
        .expect("异 cwd 不因为工作区级那把锁互相阻塞");

    // 同一个 cwd 反过来是排队的 —— 这才是那把锁的本意。
    let same = table.locks(first_dir.path());
    let blocked = tokio::time::timeout(Duration::from_millis(80), same.lock_exclusive()).await;
    assert!(blocked.is_err(), "同 cwd 的 exclusive 要排队");
}
