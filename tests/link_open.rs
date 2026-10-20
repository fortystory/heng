//! 对话视图里点一条**工作区文件路径**：走**文件弹窗**那一扇门
//! （`.scratch/link-emphasis/spec.md` §3）。
//!
//! 接缝是「点一下」：一个状态进去，一块定尺的缓冲出来，指针落在对话视图里那段文字上，
//! 然后读回界面上有什么。断言说的都是**人看得见的东西** —— 弹窗里是文件的内容、标题是
//! 工作区相对路径、以及这一条**不**被交给宿主。

mod common;

use common::{buffer, click, find_cell, said, screen_lines};

use heng::render::TuiState;

/// 一个临时工作区：文件弹窗按 `TuiState.cwd` 读盘
/// （`.scratch/files-page/spec.md` §6）。
fn workspace(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("heng-link-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("临时工作区");
    dir
}

/// 以 `dir` 为工作目录的状态。
fn state_in(dir: &std::path::Path) -> TuiState {
    TuiState::new(common::facts(), dir.to_path_buf(), None)
}

/// 点对话视图里画着 `needle` 的那一格。
fn click_in_conversation(state: &mut TuiState, needle: &str) {
    let frame = buffer(120, 24, state);
    let (column, row) = find_cell(&frame, 120, 24, needle);
    click(state, column + 1, row);
}

#[test]
fn clicking_a_workspace_path_in_the_conversation_shows_the_file() {
    // 点一条工作区文件链接 → 界面上出现的是**文件的内容**（内置预览那一档），
    // 而不是把终端交出去一次（`.scratch/link-emphasis/spec.md` §3）。
    let dir = workspace("file");
    std::fs::write(dir.join("notes.md"), "第一行\n第二行\n").expect("写一个文件");
    let mut state = state_in(&dir);
    state.apply(said("看这个 notes.md"));

    click_in_conversation(&mut state, "notes.md");

    let text = screen_lines(120, 24, &mut state).join("\n");
    assert!(text.contains("第一行"), "文件内容画出来了：{text}");
    assert!(text.contains("notes.md"), "标题是工作区相对路径：{text}");
    assert!(
        !text.contains(&dir.to_string_lossy().into_owned()),
        "标题不该是一长串绝对路径：{text}"
    );
    assert_eq!(
        state.take_open_request(),
        None,
        "文件链接不交给宿主：弹窗本身就是回执"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn clicking_a_directory_in_the_conversation_still_goes_to_the_host() {
    // 目录仍旧交给宿主（`xdg-open`）：内置预览读不了目录。
    let dir = workspace("dir");
    std::fs::create_dir_all(dir.join("sub/inner")).expect("一个目录");
    let mut state = state_in(&dir);
    state.apply(said("看这个 sub/inner"));

    click_in_conversation(&mut state, "sub/inner");

    assert_eq!(
        state.take_open_request(),
        Some(dir.join("sub/inner").to_string_lossy().into_owned()),
        "目录交给宿主去开"
    );
    let text = screen_lines(120, 24, &mut state).join("\n");
    assert!(!text.contains("esc 关闭"), "不该开出文件弹窗：{text}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn clicking_a_url_in_the_conversation_still_goes_to_the_host() {
    // URL 仍旧交给宿主：它不是工作区里的文件。
    let dir = workspace("url");
    let mut state = state_in(&dir);
    state.apply(said("见 https://example.com/x 那一页"));

    click_in_conversation(&mut state, "https://example.com/x");

    assert_eq!(
        state.take_open_request(),
        Some("https://example.com/x".to_owned()),
        "URL 原样交给宿主"
    );
    let text = screen_lines(120, 24, &mut state).join("\n");
    assert!(!text.contains("esc 关闭"), "不该开出文件弹窗：{text}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn clicking_a_missing_path_in_the_conversation_does_nothing() {
    // 解析不出来（不存在、在区外）：**什么都不发生、也不留回执** —— 今天一样。
    let dir = workspace("missing");
    let mut state = state_in(&dir);
    state.apply(said("看这个 没有这个文件.html"));

    click_in_conversation(&mut state, "没有这个文件.html");

    assert_eq!(state.take_open_request(), None, "不该被打开");
    let text = screen_lines(120, 24, &mut state).join("\n");
    assert!(!text.contains("esc 关闭"), "不该开出文件弹窗：{text}");

    let _ = std::fs::remove_dir_all(&dir);
}
