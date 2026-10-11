# research：历史会话聚合与列表数据源的事实与成本

Type: research
Status: resolved
Blocked by: —
Part of: ../map.md

## 问题

查清「把本项目及其 git worktree 里跑过的会话聚合成一张列表」要付的**事实与代价**，好让票 02（条目）与票 04（宿主）不必猜。

**范围级决定已在 charting 冻死**（见 `../map.md` 的冻结项 3、4）：只列本项目 + 它的 git worktree 的会话；聚合靠**现场探测**。本票不是"要不要这么做"，是"这么做要花什么、边界在哪"。

## 需要查清

1. **git 探测怎么认 worktree 家族**：对候选 cwd 跑什么命令最稳（`git -C <cwd> rev-parse --git-common-dir`？`--show-toplevel`？`--abbrev-ref HEAD`？）；worktree 里 `.git` 是**文件**不是目录这件事有没有坑；同一个仓库的主检出与各 worktree 返回什么、怎么归并成一组；裸仓库 / 子模块 / `--separate-git-dir` 之类边界要不要管（可以明确"不管"，但要写下来）。
2. **探测的代价与失败**：候选 cwd 不存在（worktree 已删、目录改名、跨机器搬来的会话目录）时 `git -C` 报什么、耗时多少；无 `git`、超时、hang 的处理；一个 store 里几十上百个桶时，串行探测要不要池化 / 缓存（进程内一次、按键缓存）。
3. **读流的代价（实测）**：`SessionStore::list_all` 加每条流 `read_events` **全量解析**在真实 store 上的耗时与规模（条数、字节数）；**只读前缀**是否可行 —— `SessionStarted` 是不是第一行、首条用户消息一般落在第几行（这决定标题能不能便宜地派生，票 01 §5 要用）；给出一个"只读前 N 行"的实现代价估计。
4. **已有可复用的路**：`observe::summarize` / `Listing` 字段够不够列表用（`cwd` / 开始 / 用量 / 轮次 / 消息 / 结束）；还差哪些字段（标题、worktree 名、分支名）；要不要一个专门的"列表摘要"读取路径（只读前缀）而不是复用全量。
5. **失效会话的分类**：cwd 不存在、流损坏（末行不完整）、目录在但 `log.jsonl` 不在 —— 各自在列表里能不能显示、显示成什么（票 02 要用这张表）。
6. **缓冲与刷新**：列表在切回页签时要不要重新探测（成本）；mtime 变化能不能当失效信号；`SessionStore` 有没有现成的变更通知（大概没有，写清代价）。
7. **跨机器**：会话目录被拷到另一台机器后 `SessionStarted.cwd` 在本地不存在 —— 归到"工作区已不在"是不是唯一诚实的答案（对应 `../map.md` 的「尚未明确」）。

## 产物与落点

- 事实与实测数字写成 `.scratch/session-tab/research/03-aggregation-facts-and-cost.md`；实测脚本 / 基准放在 `research/` 下（照 `.scratch/tui-history-replay/research/` 的既有惯例：那里有 `02-replay-seam-and-cost.md` 与 `bench/`）。
- 答案追加进本票的 `## 作答`，`Status:` 改 `resolved`，并把一条 context pointer 追加到 `../map.md` 的 `已定的决定`。

## 规模判断（要不要用 bench）

第 3 条（读流成本）与第 2 条（探测成本）如果光靠读代码说不清，就照 tui-history-replay 的做法写一个**一次性的最小 bench**（`research/bench/`）实测，而不是估。结论里必须给数字与命令。

## 先读

- `src/session/store.rs`（布局、`bucket` / `cwd_slug` / `list_all` / `started_cwd`）
- `src/session/observe.rs`（`Listing` 与批量摘要）
- `src/events.rs` 的 `read_events`（全量解析的现场）与 `SCHEMA_VERSION`
- `src/tools/git.rs`（既有 git 用法的边界：**没有** `rev-parse`，也没 `worktree`）
- `.scratch/git-worktree/seed.md`（相邻意向：本图只读 worktree，不建）
- `.scratch/tui-history-replay/research/`（实测与 bench 的既有写法）

## 作答

全文、逐条证据、实测命令与数字在
[`research/03-aggregation-facts-and-cost.md`](../research/03-aggregation-facts-and-cost.md)；
一次性 bench 在 `research/bench/`（编译产物进 `.cache/`）。被测 HEAD：`f532d1b`，本机 `git 2.56.0`。

本机真实 store = **281 场会话 / 88,469,279 B / 74,803 事件 / 4 个桶 / 4 个不同 cwd**（release、5 轮）：

1. **认 worktree 家族**：家族键是 `git -C <cwd> rev-parse --path-format=absolute --git-common-dir`
   —— 主检出、`src/` 子目录、两个临时 worktree 四者返回**同一个**绝对路径。不带 `--path-format=absolute`
   不可用（主检出答 `.git`、子目录答 `../.git`、worktree 答绝对路径）。显示名取 `--show-toplevel`
   （worktree 答 worktree 自己），分支名取 `--abbrev-ref HEAD`（detached 答字面量 `HEAD`，空仓库 exit 128）。
   worktree 里 `.git` 是 60 字节的文件（`gitdir: <主 .git>/worktrees/<name>`），`git -C` 全部正常。
   家族成员可直接 `worktree list --porcelain`。**不管**：裸仓库（`--show-toplevel` exit 128、
   `--is-inside-work-tree=false`）、子模块（家族键更深一层，自成一家）、`--separate-git-dir`（未实测）。
2. **探测的代价与失败**：单次 `rev-parse` **0.98–2.3 ms**（进程启动为主）；一次调用可问三项
   （`--git-common-dir --show-toplevel --abbrev-ref HEAD` 共 1.9–3.2 ms）。cwd 不存在 / 是文件 /
   不是仓库全部 exit **128**、**1.5–2.8 ms**，没有 hang；`git` 不在 PATH 是 exit 127。
   1000 个候选串行 **983.2 ms**，400 个候选 8 线程 **40.3 ms**；但列表范围本身只有 4 个桶的 cwd，
   池化不是必需品。
3. **读流的代价（实测）**：`list_all` 4.6–5.1 ms；`list_all` + 每条 `read_events` **全量解析
   155.6–168.8 ms**（最慢单条 32.2 ms / 17.3 MB / 14,544 事件）；`observe::summarize` 同价
   （154.7–165.8 ms，无重复读）；**只读前 8 行前缀 3.7–4.8 ms**（约 42×便宜）。`SessionStarted`
   **是第一条事件**（281/281 与合成 372/372），前缀足以拿 cwd。**首条用户消息不在前缀里稳定出现**：
   134 条有它的流里 130 条落在第 4–7 行（p50 = 6），另 4 条在第 685 / 2549 / 2737 / 13195 行；
   **147/281（52.3%）的流根本没有 `role:User` 消息**，扫到「没有」= 读完整条流（全库 54.5–58.5 ms）。
   放大样本（合成 372 场 / 186 MB / 390,452 事件）复现同一比例。
4. **已有可复用的路**：`observe::summarize` 就是现成的「一条流的列表行」，字段 `id` / `dir` /
   `cwd` / `started` / `written`(mtime) / `ended` / `tokens` / `messages` / `rounds` 实测全部有值
   （281/281 有 `cwd` 与 `started`）。**缺**标题、分支名、工作区显示名、失效态。前缀路径值得单开
   （`read_events` 一次读全，对「只要前 8 行」没有捷径；`BufReader::lines()` + `serde_json::Value`
   只取一个字段约 30 行代码）。`EventLog::open` 会 `repair_before_append` 改文件，**列表不能走它**。
5. **失效会话的分类**（源码 + 最小 store 实测，共 9 类，表在报告「结论 5」）：目录在但 `log.jsonl`
   不在 ⇒ `list_all` **不认**；0 字节 ⇒ `Ok([])` + 全 `None` 的 `Listing`；中间坏行 ⇒ `read_events`
   / `summarize` 双双 `Err`（一条坏流能毁掉整页）；末行半行 ⇒ 静默丢弃、其余照读；末行无换行 ⇒
   照读；无 `SessionStarted` ⇒ `cwd=None`；EACCES ⇒ `Err`；桶根的散文件不认；名字不规范的目录
   **也当会话**（`scan_bucket` 只查 `is_dir` + `log.jsonl`）。**cwd 失效有两种形态**：本机
   `/home/forty/code/fortystory/fs-agent` 存在但**不是仓库**（167 场会话），另一种是目录根本不在。
6. **缓冲与刷新**：`SessionStore` **没有**任何变更通知（grep `notify` / `watch::` / `Notify` 零命中），
   刷新只能重扫：前缀 3.9 ms / 全量 164.6 ms / git 每候选约 1 ms。mtime 当信号够用
   （1.123 µs 一条，281 条 0.32 ms），`sort_newest_first` 本来就用它。想判「这条流还在写」可以只读
   尾部 4 KiB 拿末条事件类型，**2.5–3.2 ms/全库**。真实 store 里 **0/281** 条流以 `SessionEnded`
   结尾（整库几乎没有），合成样本 61/372 条被 tail 正确识别。
7. **跨机器**：cwd 只在 `SessionStarted.cwd` 里、桶名是不可逆的 FNV 哈希，所以「属于哪个工作区」
   只能按字符串在本地判，且判定完全落在列表一侧（`summarize` 对 372 条不存在的 cwd 照常返回）。
   诚实答案不是一句「工作区已不在」，而是三态：目录不在 / 目录在但不是仓库 / 探测本身失败。

