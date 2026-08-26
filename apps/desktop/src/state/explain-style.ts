// 回答口吻：跟主题一样存在本地，不进 models.json。
// 默认适中；用户这句里写「讲人话 / 讲专业点」只覆盖本轮。

export type ExplainStyle = "plain" | "balanced" | "pro";

export const DEFAULT_EXPLAIN_STYLE: ExplainStyle = "balanced";
const STYLE_KEY = "codelattice.explainStyle";

const STYLE_PROMPT: Record<ExplainStyle, string> = {
  plain: [
    "[回答口吻] 人话",
    "先说这是干什么的，最多四句。不要用 exec、system、静态分析、命令注入、置信度、启发式这些词；必须提时改成「程序里写了要跑外面的命令」「分析器看不出具体跑谁」。不要分点上课，不要主动讲风险。",
  ].join("\n"),
  balanced: [
    "[回答口吻] 适中",
    "先用人话说清楚干什么，再用一句带上必要名字（如 calls、(unknown)）。可以提「外部命令」，不要展开成注入或跨平台风险清单，除非用户问风险。一段或两点即可。",
  ].join("\n"),
  pro: [
    "[回答口吻] 专业",
    "可用术语（calls、confidence、external-command-invocation、静态分析边界）。结构：含义 → 证据 → 限制。不要把分析器的未知升级成已确认的安全结论；命令注入只能当假设并标明。",
  ].join("\n"),
};

export function readExplainStyle(): ExplainStyle {
  try {
    const raw = localStorage.getItem(STYLE_KEY);
    if (raw === "plain" || raw === "balanced" || raw === "pro") return raw;
  } catch {
    /* 隐私模式 */
  }
  return DEFAULT_EXPLAIN_STYLE;
}

export function writeExplainStyle(style: ExplainStyle): void {
  try {
    localStorage.setItem(STYLE_KEY, style);
  } catch {
    /* 配额不足时只改本次会话 */
  }
}

export function detectExplainStyleOverride(text: string): ExplainStyle | null {
  if (/讲专业|专业点|专业风格|用术语/.test(text)) return "pro";
  if (/适中|兼顾|中间态/.test(text)) return "balanced";
  if (/讲人话|大白话|通俗|小白|说人话|不要术语/.test(text)) return "plain";
  return null;
}

export function resolveExplainStyle(saved: ExplainStyle, userText: string): ExplainStyle {
  return detectExplainStyleOverride(userText) ?? saved;
}

export function formatExplainStylePrompt(style: ExplainStyle): string {
  return STYLE_PROMPT[style];
}
