// 对话窗宽度：左缘拖动，不靠 CSS resize（右下角手柄会往窗外拉）。

export const CHAT_WIDTH_DEFAULT = 340;
export const CHAT_WIDTH_MIN = 280;
export const CHAT_WIDTH_MAX = 720;
const CHAT_WIDTH_KEY = "codelattice.chatWidth";

export function clampChatWidth(width: number): number {
  if (!Number.isFinite(width)) return CHAT_WIDTH_DEFAULT;
  return Math.min(CHAT_WIDTH_MAX, Math.max(CHAT_WIDTH_MIN, Math.round(width)));
}

export function readChatWidth(): number {
  try {
    const raw = localStorage.getItem(CHAT_WIDTH_KEY);
    if (raw) return clampChatWidth(Number(raw));
  } catch {
    /* 隐私模式 */
  }
  return CHAT_WIDTH_DEFAULT;
}

export function writeChatWidth(width: number): void {
  try {
    localStorage.setItem(CHAT_WIDTH_KEY, String(clampChatWidth(width)));
  } catch {
    /* 配额不足时只改本次会话 */
  }
}
