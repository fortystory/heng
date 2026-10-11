#!/usr/bin/env bash
# 票 03 的失效会话场景生成器（一次性产物）。
#
# 造一个最小 store，每种「失效会话」一个会话目录，然后交给 agg-bench 观察
# `SessionStore::list_all` / `events::read_events` / `observe::summarize` 各自怎么应对。
#
# 用法（仓库根目录）：
#   bash .scratch/session-tab/research/bench/failure_cases.sh .cache/session-tab-failure-store
#   .cache/session-tab-bench-target/release/agg-bench scan .cache/session-tab-failure-store 1
set -euo pipefail

ROOT="${1:-.cache/session-tab-failure-store}"
BUCKET="$ROOT/-home-forty-code-fortystory-heng-be4950a197ebe1f7"
rm -rf "$ROOT"
mkdir -p "$BUCKET"

line1='{"seq":1,"at":"2026-10-01T08:45:02.037860883Z","speaker_id":"System","payload":{"SessionStarted":{"session_id":"20261001T084502Z-00000001","cwd":"'"$PWD"'","schema_version":2}}}'
line2='{"seq":2,"at":"2026-10-01T08:45:03.037860883Z","speaker_id":"User","payload":{"MessageCompleted":{"role":"User","text":"第一句人话","reasoning":null}}}'
line3='{"seq":3,"at":"2026-10-01T08:45:04.037860883Z","speaker_id":"System","payload":{"TurnStarted":{"agent":{"Debater":"main"},"iteration":1}}}'
line4='{"seq":4,"at":"2026-10-01T08:45:05.037860883Z","speaker_id":"System","payload":{"SessionEnded":{"reason":"Completed"}}}'

mk() { mkdir -p "$BUCKET/$1/outputs"; }

# 1) 健康
mk 20261001T090000Z-00000001
printf '%s\n%s\n%s\n' "$line1" "$line2" "$line3" > "$BUCKET/20261001T090000Z-00000001/log.jsonl"

# 2) 末行不完整（写到一半被杀）
mk 20261001T090100Z-00000002
printf '%s\n%s\n{"seq":3,"at":"2026-10-01T08:45:0' "$line1" "$line2" > "$BUCKET/20261001T090100Z-00000002/log.jsonl"

# 3) 末行完整但没有换行
mk 20261001T090200Z-00000003
printf '%s\n%s' "$line1" "$line2" > "$BUCKET/20261001T090200Z-00000003/log.jsonl"

# 4) 中间行损坏、后面还有完整的行
mk 20261001T090300Z-00000004
printf '%s\n{坏行\n%s\n' "$line1" "$line2" > "$BUCKET/20261001T090300Z-00000004/log.jsonl"

# 5) 目录在、log.jsonl 不在
mk 20261001T090400Z-00000005

# 6) log.jsonl 是空文件
mk 20261001T090500Z-00000006
: > "$BUCKET/20261001T090500Z-00000006/log.jsonl"

# 7) 只有 SessionStarted 一行
mk 20261001T090600Z-00000007
printf '%s\n' "$line1" > "$BUCKET/20261001T090600Z-00000007/log.jsonl"

# 8) 流里没有 SessionStarted（只有用户消息）
mk 20261001T090700Z-00000008
printf '%s\n' "$line2" > "$BUCKET/20261001T090700Z-00000008/log.jsonl"

# 9) 正常收尾
mk 20261001T090800Z-00000009
printf '%s\n%s\n' "$line1" "$line4" > "$BUCKET/20261001T090800Z-00000009/log.jsonl"

# 10) 桶根下散着一个文件（不是目录）与一个名字不是会话 id 的目录
echo x > "$BUCKET/stray.txt"
mk not-a-session-dir
printf '%s\n' "$line1" > "$BUCKET/not-a-session-dir/log.jsonl"

echo "已生成 $ROOT"
find "$ROOT" -name log.jsonl -printf '%s %p\n' | sort
