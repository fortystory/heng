# herdr 集成

herdr 靠进程名认 pane 里的 agent：`src/detect/mod.rs` 的 `lookup_agent` 是一张精确等值表，
没有 `heng`。于是 heng **自己上报**，协议见
[Add Herdr support to your agent](https://herdr.dev/docs/add-herdr-support/)。

- **只在 herdr 里做**：`HERDR_ENV=1` 且 `HERDR_PANE_ID` / `HERDR_BIN_PATH` 都在；外面一次调用
  都不发。检测失败绝不产生副作用。
- **报什么**：前台会话的三态 → `idle` / `working` / `blocked`（`RunningTurn` → `working`、
  `AwaitingHuman` → `blocked`）；每一条都带 resume 命令 `heng -c <id>`；退出时
  `pane release-agent`。
- **怎么报**：后台线程 + 短命进程，主循环从不等待；`--seq` 用 UNIX 毫秒并在进程内严格递增，
  herdr 会忽略旧报告。
- **落在哪**：`src/herdr.rs`；接入点是 `src/cli/interactive.rs` 的 80ms 状态轮询与退出收尾。
- **验证**：在 herdr 的 pane 里跑 `heng`，然后 `herdr agent list` 与
  `herdr pane get "$HERDR_PANE_ID"`。
