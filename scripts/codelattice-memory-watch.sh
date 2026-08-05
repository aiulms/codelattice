#!/usr/bin/env bash
# codelattice-memory-watch.sh — 采样 codelattice mcp 进程的内存曲线
#
# 用途：验证 MCP sidecar 在重度分析（workspace 索引 / impact preview /
# detect-changes）期间的内存峰值，以及分析结束后是否正常回落。
#
# 用法：
#   scripts/codelattice-memory-watch.sh [--interval 秒] [--duration 秒] [--log 路径]
#
# 默认：interval=2s，duration=无限（Ctrl-C 结束并输出汇总），
#       log=docs/perf/memory-watch-YYYYMMDD-HHMMSS.log
#
# 建议流程：
#   1. 终端 A 启动本脚本
#   2. 终端 B（或 Kimi 会话）触发一次重度分析
#   3. 分析完成后等 1-2 分钟，Ctrl-C 结束采样，查看汇总里的峰值与回落判定

set -u

INTERVAL=2
DURATION=0   # 0 = 无限
LOG=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --interval) INTERVAL="$2"; shift 2 ;;
    --duration) DURATION="$2"; shift 2 ;;
    --log)      LOG="$2";      shift 2 ;;
    -h|--help)
      sed -n '2,20p' "$0"; exit 0 ;;
    *) echo "未知参数: $1" >&2; exit 2 ;;
  esac
done

# 日志默认落在 docs/perf/，便于后续归档进 closure 笔记
if [[ -z "$LOG" ]]; then
  ROOT="$(cd "$(dirname "$0")/.." && pwd)"
  mkdir -p "$ROOT/docs/perf"
  LOG="$ROOT/docs/perf/memory-watch-$(date +%Y%m%d-%H%M%S).log"
fi

sample() {
  local ts="$1"
  # ps 的 rss 单位是 KB；匹配完整的 mcp sidecar 命令，排除 watchdog / grep 自身
  ps -Ao pid=,rss=,command= | while read -r pid rss cmd; do
    case "$cmd" in
      *codelattice\ mcp*) echo "$ts $pid $rss" >> "$LOG" ;;
    esac
  done
}

# 用 awk 在结束时从日志直接算汇总，避免 subshell 里关联数组丢失的问题
summarize() {
  echo "" | tee -a "$LOG"
  echo "===== 采样汇总 $(date '+%Y-%m-%d %H:%M:%S') =====" | tee -a "$LOG"
  awk '
    # 只处理采样数据行（HH:MM:SS pid rss），跳过表头和历史汇总段
    /^[0-9][0-9]:[0-9][0-9]:[0-9][0-9] [0-9]+ [0-9]+$/ {
      pid=$2; rss=$3;
      if (!(pid in peak) || rss > peak[pid]) { peak[pid]=rss; peak_at[pid]=$1 }
      last[pid]=rss; count[pid]++
    }
    END {
      for (p in peak) {
        drop = peak[p] - last[p]
        # 回落判定：末值较峰值下降超过 200MB 且末值低于峰值的一半
        verdict = (drop > 204800 && last[p] < peak[p] * 0.5) ? "已回落" : \
                  (peak[p] < 102400 ? "全程低水位" : "未明显回落（需关注）")
        printf "PID %-8s 样本 %-5d 峰值 %8.1f MB（%s） 末值 %8.1f MB  → %s\n", \
               p, count[p], peak[p]/1024, peak_at[p], last[p]/1024, verdict
      }
    }
  ' "$LOG" | tee -a "$LOG"
  echo "日志: $LOG"
}

trap summarize EXIT

echo "采样间隔 ${INTERVAL}s，日志: $LOG"
echo "开始采样 $(date '+%Y-%m-%d %H:%M:%S')（Ctrl-C 结束并输出汇总）"
echo "# time pid rss_kb" > "$LOG"

ELAPSED=0
while true; do
  TS="$(date '+%H:%M:%S')"
  sample "$TS"
  if [[ "$DURATION" -gt 0 && "$ELAPSED" -ge "$DURATION" ]]; then
    break
  fi
  sleep "$INTERVAL"
  ELAPSED=$((ELAPSED + INTERVAL))
done
