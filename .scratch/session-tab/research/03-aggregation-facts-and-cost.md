# research：历史会话聚合与列表数据源的事实与成本

Part of: ../map.md
Ticket: ../issues/03-aggregation-facts-and-cost.md
Status: resolved（本文件只报事实与实测数字，不推荐方案、不选赢家）

被测仓库 HEAD：`f532d1b`（`feat(tui): / 菜单去掉左右两条竖线`）。源码行号以本文件写就时的文件为准；引用处都带符号名，行号漂移时按名字重新定位。本文件里的每个数字都是本机实测，命令与样本数写在每一节；没有实测支撑的话全部标了「未实测」。

**一句话结论**：本机真实 store（281 场会话 / 88,469,279 B / 74,803 事件）上，`list_all` + 全量 `read_events` 解析一遍是 **155.6–168.8 ms**（release），只读前 8 行前缀是 **3.7–4.8 ms**（约 42×），逐行扫到首条用户消息是 **54.5–58.5 ms**；worktree 家族的家族键是 `git -C <cwd> rev-parse --path-format=absolute --git-common-dir`，单次约 **0.98–1.14 ms**，1000 个候选串行约 **1.0 s**，8 线程池化 400 个候选只要 **40.3 ms**。

---

## 结论

### 1. git 探测怎么认 worktree 家族

- **唯一的家族键**：`git -C <cwd> rev-parse --path-format=absolute --git-common-dir`。主检出与它的每一个 worktree 返回**同一个路径**（本仓库的实测值都是 `/home/forty/code/fortystory/heng/.git`）。
- 不加 `--path-format=absolute` **不能**用：主检出返回 `.git`、子目录返回 `../.git`、worktree 返回绝对路径（同一台机器上三种形状），字符串比较会漏掉主检出与子目录。
- `--show-toplevel` 答的是**这个检出自己**的顶层目录，worktree 里返回 worktree 路径、主检出里返回主路径 —— 它正是「列表里那一行该写哪个工作区」的显示名来源，不是家族键。
- worktree 里 `.git` 是一个 **60 字节的普通文件**，内容为 `gitdir: <主仓库>/.git/worktrees/<名字>`。`git -C` 一族全部正常（实测），不需要自己解析这个文件。
- 分支名：`git -C <cwd> rev-parse --abbrev-ref HEAD`。worktree 返回它 checkout 的分支名（实测 `tmp-wt1`），detached worktree 返回字面量 **`HEAD`**，**空仓库（无 commit）返回 exit 128**（`有歧义的参数 'HEAD'`）。
- 家族内的全部成员可以直接列出：`git -C <cwd> worktree list --porcelain`（主仓库与每个 worktree 都给 `worktree <路径>` + `HEAD <sha>` + `branch <ref>` / `detached`）。
- **明确不管**的边界（见「边界与失败」第 1 节）：裸仓库、子模块、`--separate-git-dir`。

### 2. 探测的代价与失败

- 单次 `rev-parse` 约 **0.98–2.3 ms**（进程启动占绝对大头；大多数样本落在 1.0–2.0 ms）。一次调用可以问多项：`git -C <d> rev-parse --git-common-dir --show-toplevel --abbrev-ref HEAD` 实测 1.9–3.2 ms，**≈ 一次调用的价钱拿三个答案**。
- cwd 不存在：exit **128**，stderr `cannot change to '<dir>': 没有那个文件或目录`，耗时 **1.5–2.8 ms**（不慢于成功路径，没有 hang）。
- cwd 是文件、或不是仓库：exit **128**，同样 2–3 ms。
- 没有 `git`（`PATH=/nonexistent`）：exit **127** + `git: 未找到命令`，CLI 侧看到的是 spawn 失败。
- 1000 个候选串行：**983.2 ms**（`agg-bench probe-serial 1000 /nonexistent-xyz`，中位）。400 个候选池化：1 线程 455.6 ms → 2 线程 192.0 ms → 4 线程 75.5 ms → 8 线程 **40.3 ms** → 16 线程 29.2 ms。
- 但**列表范围本身把候选数压得很小**：真实 store 281 场会话去重后只有 **4 个不同 cwd**，其中 3 个属于本次要列的范围（见「事实与实测」第 1 节）。也就是说如果只对「每个桶的 cwd」探测一次，一次探测就是一次进程调用，成本在**毫秒级**，池化不是必需品。

### 3. 读流的代价（实测）

真实 store（`~/.local/share/heng/sessions`，release、5 轮，命令见下）：

| 路径 | 281 条流的耗时 | 每条平均 |
|---|---|---|
| `SessionStore::list_all`（只扫目录） | 4.6–5.1 ms | 16 µs |
| `list_all` + 每条 `read_events` 全量解析 | **155.6–168.8 ms**（中位 161.8） | 576 µs |
| `list_all` + 每条 `observe::summarize`（= 全量读 + 聚合） | **154.7–165.8 ms** | 582 µs |
| `list_all` + 每条只读前 8 行前缀 | **3.7–4.8 ms** | 14 µs |
| `list_all` + 逐行扫到首条用户消息（不解析） | **54.5–58.5 ms** | 198 µs |
| 每条流统计 mtime（`metadata().modified()`） | 0.32 ms（1.123 µs/次 × 281） | 1.1 µs |

- **只读前缀可行**：`SessionStarted` **是**第一条事件（281/281 条真实流的首行就是它；前 8 行内拿到 cwd 的也是 281/281）。只读前 8 行比全量解析便宜 **约 42×**。
- **首条用户消息（`MessageCompleted{role:User}`）不在前缀里稳定出现**：134 条有它的流里，130 条落在前 8 行内（行号 4–7，众数 6），另外 4 条落在第 **685 / 2549 / 2737 / 13195** 行。
- **一半的流根本没有 `role:User` 的消息**：147/281（52.3%）。它们要一直读到结尾才知道没有，扫到「没有」的成本就是整条流 —— 这就是上面 54.5–58.5 ms 的构成。
- 真实 store 规模：281 场会话、**88,469,279 B**、**74,803 条事件**（平均 266.2）。单文件 p50 7,301 B / p90 824,415 B / max 17,289,895 B；事件数 p50 9 / p90 626 / max 14,544。
- 放大样本（`.cache/` 下现造的合成 store，30 桶 / 372 场 / 186,137,134 B / 390,452 事件）上同一条路径是 **300.8–342.0 ms** 全量、**3.3–4.6 ms** 前缀，比例不变。

### 4. 已有可复用的路

- `observe::summarize(&StoredSession) -> io::Result<Listing>`（`src/session/observe.rs:77`）已经就是「一条流的列表行」：全量 `read_events` 一次，然后 `started_cwd` + 遍历一遍累加。**实测它与裸 `read_events` 同价**（同一次运行里 164.6 vs 163.9 ms，差在噪声里），没有额外的重复读。
- `Listing` 现有字段全部有值：`id` / `dir` / `cwd` / `started` / `written` / `ended` / `tokens` / `messages` / `rounds`（281/281 有 `cwd` 与 `started`）。`started` 取第一条事件的 `at`，`written` 取文件 mtime，`tokens` 走 `events::total_usage`，`messages` 数 `MessageCompleted`、`rounds` 数 `RoundStarted`。
- **列表要用、但 `Listing` 里没有的**：标题（首条用户消息派生，见第 3 条的行号事实）、分支名（`--abbrev-ref HEAD`）、worktree 的显示名（`--show-toplevel`）、以及「流损坏 / cwd 已不在」这两个失效态。
- **专门的前缀读取路径不是必须，但有明确收益**：`read_events` 是「一次读全、全部解析」（`src/events.rs:1086`），它对「只要前 8 行」没有捷径 —— 调用方拿到 `Vec<Event>` 时文件已经读完。前缀路径的实现代价很小：`BufReader::lines()` 加 `serde_json::from_str::<serde_json::Value>` 只取 `payload.SessionStarted.cwd`（本 bench 的 `prefix_scan` 就是 30 行代码）。

### 5. 失效会话的分类

| 场景 | `list_all` 认它吗 | `read_events` 的答 | `summarize` 的答 | 列表里能不能显示 |
|---|---|---|---|---|
| 目录在、`log.jsonl` 在、内容合法 | 是 | `Ok(events)` | 完整 `Listing` | 能 |
| 目录在、`log.jsonl` **不在** | **否**（`scan_bucket` 要求 `log_path.is_file()`） | — | — | 不能，条目根本不存在 |
| `log.jsonl` 是 **0 字节** | 是 | `Ok(vec![])` | 全 `None` / 0 的 `Listing` | 能，但每个字段都要兜底 |
| 只有 `SessionStarted` 一行 | 是 | `Ok([一条])` | `cwd` 有、`started` 有、其余 0 | 能 |
| **中间行损坏**、后面还有合法行 | 是 | `Err(InvalidData)` | `Err(InvalidData)` | 不能直接显示，必须按「流损坏」兜底 |
| **末行写坏**（半行） | 是 | `Ok(...)`，坏的末行被**静默丢弃** | 同上（读到坏行之前的全部） | 能，字段齐 |
| **末行完整但没有换行** | 是 | `Ok(...)`（含最后一条） | 同上 | 能 |
| 流里没有 `SessionStarted` | 是 | `Ok(...)` | `cwd = None` | 能，归到「工作区未知」 |
| 目录不可读（EACCES） | 是（`read_dir` 成功、`is_file` 走 `metadata`） | `Err(PermissionDenied)` | `Err(PermissionDenied)` | 不能，按「读不出」兜底 |
| 桶根下散着普通文件 | **否**（`!path.is_dir()`） | — | — | 不能 |
| 名字不是会话 id 的目录、但有 `log.jsonl` | **是**（只查 `is_dir` + `log.jsonl`，不校验名字） | `Ok(...)` | 完整 `Listing` | 能 |

- 失效的 cwd 本身不会让 `list_all` / `summarize` 出错：cwd 只从 `SessionStarted` 读出来，**从来不 stat**。路径是否存在要列表自己判。
- 真实 store 里有一条现成的样本：`/home/forty/code/fortystory/fs-agent` **存在**（空目录）但**不是 git 仓库**，167 场会话的 cwd 都指着它（仓库改名前的路径）。所以「cwd 失效」至少有**两种**：「目录在、不是仓库」与「目录不在」。

### 6. 缓冲与刷新

- `SessionStore` **没有**任何变更通知/订阅：全仓 grep `notify` / `watch::` / `Notify` 在 `src/session/`、`src/events.rs`、`src/lib.rs` 里零命中。刷新的唯一依据是**重新扫节点上的目录 + 文件的 mtime**（`store::modified`，`src/session/store.rs:284`）。
- 刷新一次列表的代价 = 第 3 条的那张表：只做前缀 **4.6 ms**；做全量摘要 **164.6 ms**；再加一轮 git 探测（4 个桶的 cwd）约 **+4 ms**。
- mtime 能当失效信号，粒度足够：`metadata().modified()` 每次 1.123 µs，281 条 0.32 ms；`sort_newest_first` 本来就用它排序（`store.rs:217-223`，平手用 id 兜底）。
- 「最近活动」的另一条便宜路：**只读每条流的尾部 4 KiB** 就能拿到末条事件的类型。281 条全做一遍 **2.5–3.2 ms**（`agg-bench tail`）。
- 真实 store 里 **0/281** 条流的末条完整事件是 `SessionEnded`（整库就没有 `SessionEnded`）；合成样本里 61/372 条是，tail 路径 2.9–3.9 ms 全部识别。

### 7. 跨机器

- 会话目录里唯一记 cwd 的地方是 `SessionStarted.cwd`；桶名是 FNV-1a 哈希（`store.rs:258-281`），不可逆。所以搬到另一台机器后，「那场会话属于哪个工作区」**只能**按那条字符串在本地判。
- 本地判法的边界要写清：`Path::is_dir()` 为假 ⇒ 「目录不在」；`is_dir()` 为真但 `rev-parse` 失败 ⇒ 「目录在、不是仓库」（本机 167 场会话就是这一态）；两者都要显示，不能合并成一句「工作区已不在」。
- 合成 store 上 372/372 条 cwd 不存在时，`summarize` 全部照常返回 `Listing`（不报错），判定完全落在列表这一侧；判定成本含在 `summarize` 里（317–339 ms，也就是 164 ms + 372 次 `is_dir()`）。

---

## 事实与实测

### 1. store 的规模与形状（真实数据）

命令与原始数字：

```
$ find ~/.local/share/heng/sessions -name log.jsonl | wc -l
281
$ find ~/.local/share/heng/sessions -name log.jsonl -printf '%s\n' | awk '{s+=$1} END {print s}'
88469279
$ for b in ~/.local/share/heng/sessions/*/; do ...; done   # 每桶会话数
167  -home-forty-code-fortystory-fs-agent-986aee694bc228c4
110  -home-forty-code-fortystory-heng-be4950a197ebe1f7
  3  -home-forty-tmp-empty-dir-adfeaeae465ea786
  2  -home-forty-code-ai-job-92f9a5ccdfd90fe3
```

- 4 个桶、281 场会话；`XDG_DATA_HOME` 未设，根在 `$HOME/.local/share/heng/sessions`（`src/config.rs:1455-1457`）。
- 事件分布：`ToolCallStarted` 13,090 / `PermissionDecided` 13,084 / `ToolCallCompleted` 13,084 / `MessageCompleted` 11,344 / `TurnStarted` 11,093 / `UsageRecorded` 11,060 / `ContextInjected` 818 / `TurnEnded` 415 / `SessionStarted` 281 / `SandboxStatus` 235。
- 行字节：p50 299 B / p90 2,850 B / p99 14,139 B / mean 1,183 B / 74,803 行。
- 每个桶的 `cwd`：去重后只有 **4 个**，`/home/forty/code/fortystory/fs-agent`（167 条，存在但不是仓库）、`/home/forty/code/fortystory/heng`（109 条）、`/home/forty/tmp/empty_dir`（3 条）、`/home/forty/code/ai-job`（2 条）。**没有一条 cwd 落在 worktree 里** —— 本机的 worktree 一侧只能靠临时建的那个看。
- 会话数不是桶数：`list_all` 遍历的是桶，`list(cwd)` 只扫一个桶。

### 2. 实测命令（可复现）

bench 在 `.scratch/session-tab/research/bench/`（自成 workspace，path 依赖产品 crate），编译产物按仓库规矩进 `.cache/session-tab-bench-target/`。

一键重跑全部测量（从仓库根目录，`$HENG_STORE` 缺省就是本机真实 store）：

```
$ bash .scratch/session-tab/research/bench/run-bench.sh
$ # 输出写到 .cache/session-tab-bench-release.txt
```

它内部做的就是下面这一串（想单跑某一步时照抄）：

```
$ cd .scratch/session-tab/research/bench
$ CARGO_TARGET_DIR=<repo>/.cache/session-tab-bench-target cargo build --release --offline
$ B=<repo>/.cache/session-tab-bench-target/release/agg-bench
$ $B scan ~/.local/share/heng/sessions 5        # 真实 store，5 轮
$ python3 synth_store.py <repo>/.cache/session-tab-synth-store
$ $B scan <repo>/.cache/session-tab-synth-store 2
$ $B tail ~/.local/share/heng/sessions           # 尾部前缀键
$ $B probe-serial 1000 /nonexistent-xyz
$ $B probe-parallel 400 <worktree 路径>          # 本票实测时是临时 worktree，已删
$ bash failure_cases.sh <repo>/.cache/session-tab-failure-store
$ $B scan <repo>/.cache/session-tab-failure-store 1
```

真实 store **只读**：脚本不删改任何会话目录。合成 store、失效场景 store 与 worktree 都建在 `.cache/` 下，删掉不影响仓库状态、重跑可再造。

完整输出在 `.cache/session-tab-bench-release.txt`（真实 + 合成 + git + 失效，随测量一起写，可再生）。下面各表是**两次独立运行**的结果范围。

### 3. 全量解析的分布（真实 store，release）

| 指标 | 值 |
|---|---|
| 首轮解析出的事件总数 | 74,803（平均 266.2/流） |
| 解析错误 | 0 |
| 全库总耗时（5 轮） | best 155.6 / median 161.8 / worst 172.2 ms |
| 单条流中位耗时 | p50 0.02 ms / p90 1.51 ms / max 32.16 ms |
| 最慢的流 | 17,289,895 B / 14,544 事件 → **32.2 ms**（占全库 20%） |

最慢的 5 条：17.3 MB / 30.4–32.2 ms；4.47 MB / 6.9 ms；3.50 MB / 5.9 ms；3.01 MB / 5.6 ms；3.29 MB / 5.5 ms。**一条巨流吃掉全库五分之一的时间**，读流的成本分布是极长尾的。

### 4. 前缀路径的命中率与成本（真实 store）

`agg-bench scan` 的前缀实现就是 `BufReader::lines()` + 每行 `serde_json::from_str::<Value>()`，只取 `payload.SessionStarted.cwd` 与 `payload.MessageCompleted.role == "User"`：

- 前 8 行内拿到 `cwd`：**281/281**（行号全部是 1）。
- 前 8 行内见到首条用户消息：**130/134**（占有用户消息的那部分）。
- 只读 8 行的耗时：best 3.7 / median 3.9 / worst 4.3 ms（5 轮）。
- 逐行扫到首条用户消息（不解析，仅字符串判 `"role": "User"`）：第一次 best 54.5 / median 55.7 / worst 55.8 ms，第二次 best 57.9 / median 57.9 / worst 58.5 ms（各 3 轮）——**比全量解析便宜不到 3×**，因为 147 条流没有用户消息、必须读到底。

`first_user_line` 直方图（134 条）：`4→30`、`5→26`、`6→47`、`7→27`、`685→1`、`2549→1`、`2737→1`、`13195→1`。p50 = 6、p90 = 7。

### 5. git 探测的实测（临时 worktree）

本机 `git worktree list` 原本只有主检出，所以临时建了两个（都在 `.cache/` 下，用完已 `git worktree remove` + `prune` + 删分支 `tmp-wt1`）：

```
$ git worktree add .cache/session-tab-wt/wt1 -b tmp-wt1
$ git worktree add --detach .cache/session-tab-wt/wt2 HEAD
$ cat .cache/session-tab-wt/wt1/.git
gitdir: /home/forty/code/fortystory/heng/.git/worktrees/wt1
```

| 候选 cwd | `rev-parse --git-common-dir`（不带 path-format） | `--path-format=absolute --git-common-dir` | `--show-toplevel` | `--abbrev-ref HEAD` | `--is-inside-work-tree` |
|---|---|---|---|---|---|
| 主检出 | `.git` | `<repo>/.git` | `<repo>` | `master` | true |
| 主检出的 `src/` | `../.git` | `<repo>/.git` | `<repo>` | `master` | true |
| `wt1` | `<repo>/.git` | `<repo>/.git` | `<repo>/.cache/session-tab-wt/wt1` | `tmp-wt1` | true |
| `wt2`（detached） | `<repo>/.git` | `<repo>/.git` | `…/wt2` | **`HEAD`** | true |
| `wt1/src/` | `<repo>/.git` | `<repo>/.git` | `…/wt1` | `tmp-wt1` | true |
| `/tmp`、`$HOME` | exit 128 `不是 Git 仓库` | 同 | 同 | 同 | 同 |
| `/nonexistent-dir-xyz` | exit 128 `cannot change to …` | 同 | 同 | 同 | 同 |
| `Cargo.toml`（文件） | exit 128 `不是目录` | 同 | 同 | 同 | 同 |
| 空仓库（`git init` 无 commit） | `.git` | `<dir>/.git` | `<dir>` | **exit 128** | true |
| 裸仓库 `bare.git` | **`.`** | `<dir>/bare.git` | exit 128 `必须在工作区中运行` | — | **false**（exit 0） |
| 子模块 `wt1/submod/` | `<repo>/.git/worktrees/wt1/modules/submod` | 同（更深一层） | `…/wt1/submod` | — | true |

家族归并演示（同一个键）：

```
$ for d in <主检出> <主检出>/src <wt1> <wt1>/src; do
    git -C "$d" rev-parse --path-format=absolute --git-common-dir; done
/home/forty/code/fortystory/heng/.git          # 四行完全相同
/home/forty/code/fortystory/heng/.git
/home/forty/code/fortystory/heng/.git
/home/forty/code/fortystory/heng/.git
```

耗时（`date +%s%N` 前后差，3 次，均含进程启动）：

```
rev-parse --git-common-dir                        1.7 / 1.7 / 1.6 ms
rev-parse --path-format=absolute --git-common-dir 2.8 / 1.8 / 1.9 ms
rev-parse --show-toplevel                         1.7 / 1.6 / 2.3 ms
rev-parse --abbrev-ref HEAD                       2.1 / 1.7 / 1.9 ms
rev-parse --is-inside-work-tree                   1.8 / 2.1 / 1.7 ms
worktree list --porcelain                         2.8 / 2.9 / 1.6 ms
rev-parse --git-common-dir --show-toplevel --abbrev-ref HEAD  3.2 / 2.2 / 1.9 ms
```

1000 次串行与池化（`agg-bench`，release，5 轮取中位；下面是第二次运行的原始行）：

```
串行 git -C /nonexistent-xyz rev-parse --git-common-dir：n=1000 ok=0 spawn_fail=0
  总耗时: rounds=5 best=973.7ms median=983.2ms worst=1130.3ms
  每次 0.98ms → 100 个候选 98.3ms，1000 个候选 983.2ms
串行 git -C /home/forty/code/fortystory/heng 同上（成功）：n=200 ok=200 spawn_fail=0
  总耗时: rounds=5 best=196.3ms median=215.8ms worst=223.4ms
  每次 1.11ms → 100 个候选 111.2ms，1000 个候选 1112.1ms
并发（<worktree>，n=400）：
  1 线程 455.6ms（1.14ms/次）/ 2 线程 192.0ms / 4 线程 75.5ms / 8 线程 40.3ms / 16 线程 29.2ms
```

`worktree list --porcelain` 的实测输出（3 个成员）：

```
worktree /home/forty/code/fortystory/heng
HEAD f532d1bf87674c7d87604a82a08434a4fff587a5
branch refs/heads/master

worktree /home/forty/code/fortystory/heng/.cache/session-tab-wt/wt1
HEAD f532d1bf87674c7d87604a82a08434a4fff587a5
branch refs/heads/tmp-wt1

worktree /home/forty/code/fortystory/heng/.cache/session-tab-wt/wt2
HEAD f532d1bf87674c7d87604a82a08434a4fff587a5
detached
```

worktree 目录**整个删掉**之后：`git worktree list` 仍然列它，只是带 `prunable` 标记；`git worktree prune -v` 才把它从登记里去掉（`删除 worktrees/wt2：gitdir 文件指向一个不存在的位置`）。也就是说「git 里的 worktree 家族」与「磁盘上真实存在的目录」会短暂不一致。

### 6. 尾部读取（便宜的第二条前缀路径）

`agg-bench tail`：每条流打开、seek 到 `size-4096`、读到底、从末行往前找第一条能解析成 JSON 的事件：

- 真实 store：281 条，**2.5–3.2 ms**；末条事件类型 `ContextInjected` 134 / `TurnEnded` 122 / `CommandRun` 9 / `PermissionAsked` 6 / `RoundEnded` 4 / `SandboxStatus` 2 / `GoalCompleted` 1 / `HistorySuperseded` 1；**末条是 `SessionEnded` 的有 0 条**。
- 合成 store：372 条，**2.9–3.9 ms**；`TurnEnded` 291 / `SessionEnded` 61 / `MessageCompleted` 20 —— 61 条被正确识别。

### 7. `Listing`/`summarize` 的行为细节

- `summarize` 的字段来源（`src/session/observe.rs:77-104`）：`cwd` ← `store::started_cwd`（第一条 `SessionStarted`，**不 stat**）；`started` ← 第一条事件的 `at`；`written` ← 文件 mtime；`ended` ← `SessionEnded{reason}`；`messages` ← `MessageCompleted` 计数；`rounds` ← `RoundStarted` 计数；`tokens` ← `events::total_usage(...).total_tokens()`（一次额外的全量遍历，但在内存里）。
- 它把**整个 `Vec<Event>` 读进内存**（`read_events` 返回 `Vec<Event>`），281 条流一次全量摘要是 164.6 ms，内存峰值没测（见「未实测」）。
- `list_all` 的失败语义：根目录不存在 ⇒ `Ok(vec![])`（`store.rs:150`）；桶读不动 ⇒ `Err`。**一个坏桶会让整次 `list_all` 失败**，不会跳过。
- `scan_bucket` 只认「`is_dir()` + `log.jsonl` 是文件」两件事（`store.rs:185-213`），不校验目录名是否为会话 id、不看 `outputs/`。

### 8. 失效场景的实测（最小 store）

`failure_cases.sh` 造了 10 个目录（9 个有 `log.jsonl` + 1 个没有 + 1 个散文件），`agg-bench scan ... 1` 的输出：

```
list_all=9 sessions total_bytes=2498        # 缺 log.jsonl 的目录与散文件都不算（两次运行 2498–2750 B）
read_events(all): 首轮 events=12 errors=1
  error×1: …20261001T090300Z-00000004/log.jsonl：第 2 行：key must be a string at line 1 column 2
summarize: ok=8 errors=1 cwd_some=6 started_some=7
文件事实: empty=1 末字节不是换行=2 尾部 4KiB 里有 SessionEnded=1
```

逐条对照（Python 复核行号）：

```
20261001T090000Z-00000001: lines=3 first_user=2 cwd=yes   # 健康
20261001T090100Z-00000002: lines=3 first_user=2 cwd=yes   # 末行半行：被静默跳过
20261001T090200Z-00000003: lines=2 first_user=2 cwd=yes   # 末行无换行：照读
20261001T090300Z-00000004: lines=3 first_user=3 cwd=yes   # 中间坏行：read_events 报错
20261001T090500Z-00000006: lines=0 first_user=None cwd=no # 0 字节：Ok([])，Listing 全 None
20261001T090600Z-00000007: lines=1 first_user=None cwd=yes# 只有 SessionStarted
20261001T090700Z-00000008: lines=1 first_user=1 cwd=no    # 无 SessionStarted
20261001T090800Z-00000009: lines=2 first_user=None cwd=yes# 有 SessionEnded
not-a-session-dir/log.jsonl: lines=1 cwd=yes              # 名字不规范也当会话
```

`read_events` 的坏行规则（`src/events.rs:1086-1108`）：解析不了的**末**行 `break`（跳过），**非末**行返回 `InvalidData`（带文件、行号、serde 的错）。注意判定「是不是末行」用的是 `peekable`，即「后面还有没有**行**」，不是「后面还有没有**内容**」。

---

## 边界与失败

1. **明确不管的 git 边界**（都已实测，写下来是为了让票 02 不必猜）：
   - **裸仓库**：`--git-common-dir` 返回 `.`（相对 cwd），`--show-toplevel` exit 128，`--is-inside-work-tree` = `false`（exit 0）。它不是一个工作区，不能当候选 cwd。
   - **子模块**：子模块目录的 `--git-common-dir` 落在 `<主 .git>/worktrees/<wt>/modules/<name>`（比父家族的键**更深一层**），所以「同键即同家族」会把子模块分出去 —— 子模块是独立仓库，分出去是自洽的，但它不会是 `--show-toplevel` 意义上的「本项目的工作区」。本图不做决定，只记录。
   - **`--separate-git-dir`**：未实测。机制上与 worktree 同路（`--git-common-dir` 会答那个分离出去的目录），所以「同键同家族」仍然成立；但没有实测支撑，标为未证实。
   - **`GIT_DIR` / `GIT_WORK_TREE` 环境变量**：本机未设（实测 `GIT_DIR=<unset>`）。这两个变量会覆盖 `-C` 的发现结果；列表要探测的是**磁盘上的目录**，所以子进程要么不继承它们，要么显式清掉 —— 本票没测「带 `GIT_DIR` 时探测会答什么」。
2. **探测不会 hang，但会慢**：本机 1000 次串行 1.0 s，单次 1 ms 量级；**没测到过 hang 或超时**。本 bench 自己实现了 5 s 超时并在 `probe` 模式里用（`try_wait` 轮询 + `kill`），用于防「网络文件系统挂载点」这类没在本机复现的情形；这个超时是 bench 的，不是产品的。
3. **cwd 失效的两种形态都要处理**：`/home/forty/code/fortystory/fs-agent`（空目录、不是仓库、167 场会话）与「目录根本不在」。前者 `is_dir()` 为真但探测 exit 128，后者 `is_dir()` 为假。混成一句「工作区已不在」会丢掉这个区别。
4. **读流失败会污染整张列表**：中间坏行、EACCES 都让 `summarize` 返回 `Err`；`list`/`list_all` 层面的失败则是整次调用 `Err`。列表要能一站一条地兜底（票 02 的「流损坏」态），否则一条坏流能毁掉整页。
5. **`EventLog::open` 会改文件**：它在 `read_events` 之后调 `repair_before_append`（`src/events.rs:888-891` → `1114-1134`），末行完整就补一个换行、末行解析不了就 `set_len` 截掉。**列表绝不能走 `EventLog::open`**；只读路径要用 `read_events`。这条是源码事实，本票**没有**实机跑过一次「打开并截断」（那会改用户的会话目录，超出只读约束）。
6. **`read_events` 对「末尾空行」宽容**：空行被 `continue` 跳过（`events.rs:1093`），所以空的 `log.jsonl` 是 `Ok(vec![])` 而不是错误。
7. **前缀路径的语义边界**：`SessionStarted` 一定在首行是**格式约定**（`record_skeleton` 的第一句就是 `record_session_started`，`src/lib.rs:384-385`，且它写第一条事件），不是 `read_events` 保证的不变量；`started_cwd` 用的是 `find_map`（找**第一个**，不要求是第一行）。281/281 与 372/372 两个样本都成立。
8. **147/281 条流没有 `role:User` 消息**：它们不是坏流（是 harness 合成的 / rollover 出来的 / 讨论或执行者的流），但「标题从首条用户消息派生」在它们身上**没有可派生的内容**，必须给一条兜底文案。这是票 01 §5 的直接输入。
9. **真实数据里没有 ends 样本**：0/281 条流以 `SessionEnded` 结尾，整库也很可能有极少 `SessionEnded` —— 「会话已结束」这一态在本机是**不可观测分支**。
10. **未测内存**：本票只测时间。`read_events` 把整条流读成 `Vec<Event>`（17 MB 的那条会放大成多少堆内存）没有量；`summarize` 是逐条流的，峰值应该是单条流，但没有实测。
11. **未测真实终端/真实 TUI**：bench 是 release 二进制直调库函数，不含配置加载、provider、渲染。列表进渲染循环后的成本（换页重绘、左栏宽度重排）不在本票范围。
12. **`agg-bench` 自己的前缀实现是 `serde_json::Value`**：产品若实现前缀路径，用 `Event` 全量反序列化会贵得多（那是 155.6 ms 那一档），用 `Value` 只取一个字段是本票 4.6 ms 那一档。这两个数不是同一条路径的微优化，是两种实现的量级差。

---

## 对票 02 / 04 的输入

### 给票 02（条目、排序与分组）

- **家族键**用 `--path-format=absolute --git-common-dir`；显示名用 `--show-toplevel`；分支名用 `--abbrev-ref HEAD`（detached 时是字面量 `HEAD`，空仓库时 exit 128 ⇒ 没有分支名可显示）。
- 本机 281 场会话只有 **4 个不同 cwd**、全部属于 3 个「工作区」（其中一个是已失效的空目录）。分组的上限是**桶数 + worktree 数**，不是会话数。
- 一行可用的字段：`id` / `cwd` / `started` / `written`(mtime) / `ended` / `tokens` / `messages` / `rounds` 全部现成（`Listing`）；**缺**标题、分支名、工作区显示名、失效态。
- 排序口径已有：`sort_newest_first` 按 `log.jsonl` 的 mtime 降序、平手比 id（`store.rs:217-223`）。
- 失效态至少有五种，别合并：流损坏（`read_events` `Err`）、读不出（EACCES）、cwd 目录不在、cwd 在但不是仓库、没有 `SessionStarted`（cwd 未知）。表在「结论」第 5 节。
- 空态不罕见：本机 147/281 条流没有首条用户消息，0/281 条流有 `SessionEnded`。
- 成本账（真实 store，release）：刷新一次列表，只读前缀 **4.6 ms**，全量摘要 **164.6 ms**，尾部判定 **2.5–3.2 ms**，git 探测每个候选 **1 ms** 量级。

### 给票 04（宿主与并发）

- 列表刷新**没有**变更通知可用；要新鲜就得重扫。重扫的价钱同上表；mtime 是唯一现成的变更信号（1.1 µs/条）。
- 「这条流还在被写」可以用尾部 4 KiB 的末条事件类型判，成本 2.5–3.2 ms/全库，比全量解析便宜 60×。
- 同进程多 Harness 下，每场会话的工作区探测是一次进程调用；**4 个会话 ≈ 4 ms**，不是瓶颈（1000 个候选才 1 s）。
- 退出/收尾语义要注意：真实 store 里会话几乎从不写 `SessionEnded`，所以「进程退出时后台会话写不写收尾」在现有数据上没有先例可依。

---

## 参考

源码（repo-relative，行号以 HEAD `f532d1b` 为准）：

- `src/session/store.rs` —— `StoredSession` / `bucket`（cwd canonicalize + FNV slug）/ `list` / `list_all` / `scan_bucket` / `sort_newest_first` / `started_cwd` / `modified`
- `src/session/observe.rs:43-111` —— `Listing` 的字段与来源、`list` / `summarize`
- `src/events.rs:1086-1108` —— `read_events` 的坏行规则；`src/events.rs:1114-1134` —— `repair_before_append`（会改文件，列表不能走）
- `src/lib.rs:384-386` —— `record_skeleton` 的第一句就是 `agent::record_session_started`（`src/agent.rs:302`），所以 `SessionStarted` 是流的第一条；`src/lib.rs:262-272` —— `EventLog::open` / `create` 与 `resuming` 判据
- `src/config.rs:1455-1457` —— `sessions_dir`（`XDG_DATA_HOME/heng/sessions`，否则 `$HOME/.local/share/heng/sessions`）
- `src/tools/git.rs:33-56` —— git 工具只认 `status/diff/log/show/add/commit/stash`，`rev-parse` 不在表里

本票新增的一次性产物（都在 `.scratch/session-tab/research/bench/`，不是产品代码）：

- `Cargo.toml` —— 自己的 `[workspace]`，path 依赖 `../../../..`
- `src/main.rs` —— `scan` / `prefix` / `tail` / `probe` / `probe-serial` / `probe-parallel` 六个模式
- `synth_store.py` —— 合成 store 生成器（确定性 seed）
- `failure_cases.sh` —— 失效会话场景生成器
- `run-bench.sh` —— 一键重跑全部测量（真机 store 只读）
- 编译产物进 `.cache/session-tab-bench-target/`；测量输出存 `.cache/session-tab-bench-release.txt`

外部引用：无（本票没有引用任何 URL）。git 行为全部来自本机 `git version 2.56.0` 的实测。
