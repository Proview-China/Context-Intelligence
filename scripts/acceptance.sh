#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "用法: $0 <pretrackler_path> <small_dir> <large_dir> [version]"
}

BIN=${1:-}
SMALL=${2:-}
LARGE=${3:-}
VER=${4:-v1}

if [[ -z "$BIN" || -z "$SMALL" || -z "$LARGE" ]]; then
  usage; exit 1
fi

log_run() {
  local name=$1; shift
  local cmd=("$@")
  echo "==== 运行: $name ===="
  local t0=$(date +%s)
  ("${cmd[@]}" | tee "/tmp/pretackler_${name}.log") || true
  local t1=$(date +%s)
  local dur=$((t1-t0))
  local succ=$(rg -c " 完成 " "/tmp/pretackler_${name}.log" || true)
  local fail=$(rg -c " 失败 " "/tmp/pretackler_${name}.log" || true)
  echo "结果: 用时=${dur}s 完成=${succ} 失败=${fail} 日志=/tmp/pretackler_${name}.log"
}

echo "=== 小目录 ==="
log_run small "$BIN" "$SMALL" --version "$VER" --prompt ./prompt_template.md

echo "=== 大目录 ==="
log_run large "$BIN" "$LARGE" --version "$VER" --prompt ./prompt_template.md --concurrency-ceil 32

echo "=== 故障注入 429 ==="
log_run inject429 "$BIN" "$SMALL" --version "$VER" --prompt ./prompt_template.md --inject-fault 429 --verbose

echo "=== 故障注入 5xx ==="
log_run inject5xx "$BIN" "$SMALL" --version "$VER" --prompt ./prompt_template.md --inject-fault 5xx --verbose

echo "=== 故障注入 idle ==="
log_run injectidle "$BIN" "$SMALL" --version "$VER" --prompt ./prompt_template.md --inject-fault idle --stream-idle-timeout 1 --verbose

echo "完成。汇总日志位于 /tmp/pretackler_*.log"

