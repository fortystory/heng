# 16: 会话标题：`SessionTitled`、前缀派生与规格涨版

Type: implement
Status: ready-for-agent
Blocked by: —
Part of: ../map.md

**要打通的行为**：一场会话有个**名字** —— 自动从第一句人话派生，也可以手动改；改了之后落进事件流，可重放、可被 `-c` 带上；CLI 也能改名。

**来源**：[spec.md](../spec.md) §4、§8、§9；决策票 [`01-session-identity-and-title.md`](01-session-identity-and-title.md)。

## 验收

- [ ] 新事件 `SessionTitled { title }`：log-only、按散文打码、**空串即撤回**；投影忽略它、不进模型上下文
- [ ] 标题权威链：最后一条非空 `SessionTitled` → 前 8 行内首条用户消息首行（截断）→ id 时间戳渲染 `MM-DD HH:mm`
- [ ] 派生只走**前 8 行前缀轻路径**（逐行 `serde_json::Value`，只要 `SessionStarted.cwd` 与首条 `role: User` 的 `text`），**不落流**、结果只放进程内缓存
- [ ] `SCHEMA_VERSION` 2 → 3（与票 05 的 `SessionStarted` 补字段同一次涨版）；旧流照常可读
- [ ] `heng sessions title <id> <text>`：空串即撤回
- [ ] 实测：前缀派生一场 3.7–4.8 ms（281 场那一批），不因为加标题而变慢
