import { describe, expect, it } from "vitest";
import { CHAT_WIDTH_MAX, CHAT_WIDTH_MIN, clampChatWidth } from "./chat-layout";

describe("clampChatWidth", () => {
  it("keeps the chat pane inside a draggable range", () => {
    expect(clampChatWidth(200)).toBe(CHAT_WIDTH_MIN);
    expect(clampChatWidth(900)).toBe(CHAT_WIDTH_MAX);
    expect(clampChatWidth(400)).toBe(400);
  });
});
