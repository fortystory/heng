#!/usr/bin/env bash
# 票 03（历史会话聚合与列表数据源的事实与成本）的一次性测量运行器。
#
# 用法（仓库根目录）：
#   bash .scratch/session-tab/research/bench/run-bench.sh
#
# 编译产物按仓库规矩进 `.cache/`；合成 store 与失效场景 store 也建在 `.cache/` 下，跑完自己
# 决定删不删（删掉不影响仓库状态，重跑这个脚本能再造出来）。
# 真实 store 只读，本脚本不删改它。
set -euo pipefail
cd "$(dirname "$0")"

REPO=$(cd ../../../.. && pwd)
export CARGO_TARGET_DIR="$REPO/.cache/session-tab-bench-target"
BIN="$CARGO_TARGET_DIR/release/agg-bench"
REAL="${HENG_STORE:-$HOME/.local/share/heng/sessions}"
SYNTH="$REPO/.cache/session-tab-synth-store"
FAIL="$REPO/.cache/session-tab-failure-store"
OUT="$REPO/.cache/session-tab-bench-release.txt"

echo "== build =="
cargo build --release --offline

echo "== 合成 store =="
python3 synth_store.py "$SYNTH"

{
  echo "########## 真实 store: $REAL ##########"
  "$BIN" scan "$REAL" 5
  echo
  echo "########## 真实 store 的尾部 ##########"
  "$BIN" tail "$REAL"
  echo
  echo "########## 合成 store: $SYNTH ##########"
  "$BIN" scan "$SYNTH" 2
  echo
  echo "########## 合成 store 的尾部 ##########"
  "$BIN" tail "$SYNTH"
  echo
  echo "########## git 探测：串行与池化 ##########"
  "$BIN" probe-serial 1000 /nonexistent-xyz
  "$BIN" probe-serial 200 "$REPO"
  "$BIN" probe-parallel 400 "$REPO"
  echo
  echo "########## 失效会话 ##########"
  bash failure_cases.sh "$FAIL"
  "$BIN" scan "$FAIL" 1
} > "$OUT" 2>&1

echo "输出写到 $OUT"
