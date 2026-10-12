use super::*;

/// `/goal-new <名字> <来源>… [--force]`：从一批票生成一份目标清单
/// （`.scratch/goal-loop/spec.md` §2）。来源可以是**一个或多个**（空格分开）。
///
/// 返回要说给人听的那句话。**手势，不是工具调用**：它不走工具表、不走权限门、也不写事件 ——
/// 清单文件存在即是「目标已创建」的证据。
pub(crate) fn run_goal_command(harness: &Harness, goals_dir: Option<&Path>, args: &str) -> String {
    let Some(dir) = goals_dir else {
        return render::wording::no_goal_dir().to_owned();
    };
    let Ok(line) = parse_goal_new_line(args) else {
        return render::wording::goal_usage().to_owned();
    };
    // 每个来源相对于**会话**的 cwd 解析，而不是进程的：`--cwd` 可以指向别处。
    let sources: Vec<PathBuf> = line
        .sources
        .iter()
        .map(|source| {
            if Path::new(source).is_absolute() {
                PathBuf::from(source)
            } else {
                harness.cwd().join(source)
            }
        })
        .collect();
    match crate::goals::create(&line.name, &sources, dir, line.force) {
        Ok((manifest, path)) => render::wording::goal_created(
            &manifest.name,
            manifest.entries.len(),
            &path.display().to_string(),
        ),
        Err(error) => error.to_string(),
    }
}

/// `/goal-new` 那一行参数：`<名字> <来源>… [--force]`。
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct GoalNewLine {
    pub(crate) name: String,
    /// 一个或多个来源，按写下的顺序。每个来源是一条路径（**不能带空格**：空格是来源之间的分隔）。
    pub(crate) sources: Vec<String>,
    pub(crate) force: bool,
}

/// 解析 `/goal-new` 的参数。形如 `--force` 的旗标可以出现在任何位置；第一个词是目标名，剩下的
/// 每一个词是一个来源 —— 于是「一个目标装几个 feature」是一条命令。
pub(crate) fn parse_goal_new_line(args: &str) -> Result<GoalNewLine, ()> {
    let mut force = false;
    let mut words: Vec<&str> = Vec::new();
    for word in args.split_whitespace() {
        if word == "--force" {
            force = true;
        } else {
            words.push(word);
        }
    }
    let [name, sources @ ..] = words.as_slice() else {
        return Err(());
    };
    if sources.is_empty() {
        return Err(());
    }
    Ok(GoalNewLine {
        name: (*name).to_owned(),
        sources: sources.iter().map(|source| (*source).to_owned()).collect(),
        force,
    })
}

/// 目标那一族命令要的环境事实：清单目录、会话桶、cwd。
#[derive(Clone)]
pub(crate) struct GoalSetup {
    /// 目标清单目录；没有数据根时是 `None`（`/goal-new` 与 `/loop` 都照说）。
    pub(crate) dir: Option<PathBuf>,
    pub(crate) store: SessionStore,
    pub(crate) cwd: PathBuf,
    /// 两个阈值（§6）。
    pub(crate) settings: crate::config::GoalSettings,
}

/// 一个目标当下的进度：扫本桶，按归属筛出属于它的会话，再把它们的 `todo` 调用合并起来
/// （§3、§4）。
///
/// 派生的，不新增任何状态文件 —— 与日账本扫会话文件是同一条路。
pub(crate) fn goal_progress(
    goals: &GoalSetup,
    name: &str,
    manifest: &crate::goals::Manifest,
) -> crate::goals::Progress {
    let calls: Vec<crate::goals::TodoCall> = goal_streams(goals, name)
        .iter()
        .flat_map(|events| crate::goals::todo_calls(events))
        .collect();
    crate::goals::progress(&manifest.entries, &calls)
}

/// 这个目标在**别处**已经花掉的 token（§8）：该目标下**除当前会话之外**那些会话的和。
///
/// 派生的：扫本桶、按归属筛出属于它的会话，把每条流的 `UsageRecorded` 加起来。不新增状态文件
/// —— 与日账本扫会话文件按日聚合是同一条路。
///
/// 当前会话被**排除**掉了，而那正是这个数的定义：闸门读的是「整条流的求和 + 别处已经花掉的」，
/// 当前会话自己的花费已经在它自己那条流里了。把它算进来，一道闸门就会把同一笔钱数两遍 ——
/// `--continue` 回到一条已经认领过目标的流时、以及同一个会话第二次 `/loop` 同一个目标时，
/// 都会走到那条路上。
pub(crate) fn goal_usage(goals: &GoalSetup, name: &str, current: &str) -> u64 {
    let Ok(sessions) = goals.store.list(&goals.cwd) else {
        return 0;
    };
    let mut claimed: Vec<(String, Vec<Event>)> = Vec::new();
    for session in sessions {
        let Ok(events) = read_events(&session.log_path) else {
            continue;
        };
        if crate::goals::has_goal(&events, name) {
            claimed.push((session.id.as_str().to_owned(), events));
        }
    }
    crate::goals::usage_apart_from(&claimed, current)
}

/// 属于这个目标的那些会话的流（§4 的归属筛）：扫本桶、按 `GoalSelected` 过滤。
///
/// 扫的是磁盘上的会话文件，所以进行中的这个会话也在里面 —— 它的每一行都已经刷下去了。
pub(crate) fn goal_streams(goals: &GoalSetup, name: &str) -> Vec<Vec<Event>> {
    let Ok(sessions) = goals.store.list(&goals.cwd) else {
        return Vec::new();
    };
    let mut streams = Vec::new();
    for session in sessions {
        let Ok(events) = read_events(&session.log_path) else {
            continue;
        };
        if crate::goals::has_goal(&events, name) {
            streams.push(events);
        }
    }
    streams
}

/// 输入区禁言的那只闩（`.scratch/goal-loop/spec.md` §5）。
///
/// 它一造出来就禁言，一离开作用域就解除 —— 于是「循环的每一条返回路径都恢复得回来」是构造上
/// 的性质，而不是一句要人记住的规矩。
pub(crate) struct Muted<'a>(&'a ConsoleHandle);

impl<'a> Muted<'a> {
    pub(crate) fn new(console: &'a ConsoleHandle) -> Self {
        console.set_muted(true);
        Self(console)
    }
}

impl Drop for Muted<'_> {
    fn drop(&mut self) {
        self.0.set_muted(false);
    }
}

/// provider 失败之后等多久再驱动一次那个回合（§9）。
///
/// 定死在这里，不做退避：无人值守的重试是为了盖过一次抖动，不是为了把一次真正的故障熬过去
/// —— 后者该做的是停下报告。两秒足够让限流窗口挪一格。
pub(crate) const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(2);

/// 越界 id 的报账（`.scratch/goal-loop/spec.md` §3）：只在集合**变了**的时候报。
///
/// 沉默会让模型以为它记下了；而每回合都刷同一行会把转录淹掉。启动时报一次，此后循环里每回合
/// 重算发现新的再报 —— 于是「运行中新冒出来的越界 id」不会一直没人说。
pub(crate) fn report_unknown_goal_ids(
    harness: &Harness,
    name: &str,
    unknown: &[String],
    reported: &mut Vec<String>,
) {
    if unknown.is_empty() || unknown == reported.as_slice() {
        return;
    }
    *reported = unknown.to_vec();
    harness.notice(&render::wording::goal_unknown_ids(name, unknown));
}

/// 停下并报告（`.scratch/goal-loop/spec.md` §9）：落一条收尾事件，把话说在转录里。
///
/// 报告里带 `count`（连续几次翻页零完成 / 失败了几次 / 试了几次）与卡住的条目 id —— 只说
/// 「停了」没有用，回头要核得出它是怎么卡住的。
///
/// 那份进度**由这里自己算**（这一刻的派生值），不由调用方递进来：调用点有四条（撞顶 / 无进展 /
/// provider 失败 / 人主动停），而「每一处都记得传刚重算的那份」是一条会失守的约定 —— 第一次真机
/// 跑撞顶时就失守了：报告把已经做完的 01 也列成「还卡着」。
pub(crate) fn stop_goal_loop(
    harness: &mut Harness,
    name: &str,
    reason: crate::events::GoalStopReason,
    count: u32,
    manifest: &crate::goals::Manifest,
    goals: &GoalSetup,
) -> Result<(), String> {
    let progress = goal_progress(goals, name, manifest);
    let stuck = crate::goals::unfinished(&manifest.entries, &progress);
    let detail = render::wording::goal_stopped(name, reason, count, &stuck);
    if let Err(error) = harness.stop_goal(name, reason, &detail, stuck, count) {
        return Err(render::wording::error_report(&error));
    }
    harness.notice(&detail);
    Ok(())
}

/// 解析 `/loop` 的参数：一个目标名字，一个不断开的词。
pub(crate) fn parse_loop_line(args: &str) -> Result<String, String> {
    let name = args.trim();
    if name.is_empty() || name.split_whitespace().count() != 1 {
        return Err(render::wording::loop_usage().to_owned());
    }
    Ok(name.to_owned())
}
