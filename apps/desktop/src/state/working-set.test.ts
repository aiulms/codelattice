import { describe, expect, it } from "vitest";
import {
  WORKING_SET_CAP,
  attachConversationMemory,
  conversationMemoryShouldReset,
  formatRecentTurns,
  formatWorkingSetPrompt,
  removeWorkingSetItem,
  selectionWorkingId,
  upsertWorkingSet,
  visibleUserChatText,
  type WorkingSetItem,
} from "./working-set";

function item(id: string, label: string): WorkingSetItem {
  return { id, kind: "edge", label, block: `${label}\n底层边数：1` };
}

describe("upsertWorkingSet", () => {
  it("appends a new item and moves a repeat to the end", () => {
    const a = item("rel:a", "A → X");
    const b = item("rel:b", "B → Y");
    const once = upsertWorkingSet([], a);
    const two = upsertWorkingSet(once, b);
    expect(two.map((x) => x.id)).toEqual(["rel:a", "rel:b"]);
    const again = upsertWorkingSet(two, { ...a, block: "A → X\n底层边数：6" });
    expect(again.map((x) => x.id)).toEqual(["rel:b", "rel:a"]);
    expect(again[1].block).toContain("底层边数：6");
  });

  it("drops the oldest item past the cap", () => {
    let set: WorkingSetItem[] = [];
    for (let i = 0; i < WORKING_SET_CAP + 2; i += 1) {
      set = upsertWorkingSet(set, item(`rel:${i}`, `n${i}`));
    }
    expect(set).toHaveLength(WORKING_SET_CAP);
    expect(set[0].id).toBe("rel:2");
    expect(set[set.length - 1].id).toBe(`rel:${WORKING_SET_CAP + 1}`);
  });
});

describe("selectionWorkingId", () => {
  it("uses a stable id per node or edge", () => {
    expect(selectionWorkingId({ type: "none" })).toBeNull();
    expect(selectionWorkingId({
      type: "node",
      nodeId: "sym:foo",
      snapshotId: "s",
    })).toBe("node:sym:foo");
    expect(selectionWorkingId({
      type: "relation",
      relationKey: "rel:a-b",
      snapshotId: "s",
    })).toBe("edge:rel:a-b");
  });
});

describe("removeWorkingSetItem", () => {
  it("removes by id", () => {
    const set = upsertWorkingSet([item("rel:a", "A")], item("rel:b", "B"));
    expect(removeWorkingSetItem(set, "rel:a").map((x) => x.id)).toEqual(["rel:b"]);
  });
});

describe("formatWorkingSetPrompt", () => {
  it("lists discussed edges and marks the current one", () => {
    const text = formatWorkingSetPrompt(
      [item("rel:a", "scripts → (unknown)"), item("rel:b", "scripts → (root)")],
      "rel:b",
    );
    expect(text).toContain("[已讨论的图元素]");
    expect(text).toContain("scripts → (unknown)");
    expect(text).toContain("← 当前");
    expect(text.indexOf("scripts → (root)")).toBeLessThan(text.indexOf("← 当前"));
  });
});

describe("attachConversationMemory", () => {
  it("attaches the working set even when the user text is an explain prompt", () => {
    const out = attachConversationMemory(
      "请解释这条聚合边关系（已有边向上归并，不是新推断的依赖）：scripts → (root)",
      {
        workingSetBlock: "[已讨论的图元素]\n1. scripts → (unknown)\n2. scripts → (root)  ← 当前",
        currentSelectionBlock: null,
        recentTurns: "用户：这条线是什么\n助手：外部命令调用",
      },
    );
    expect(out).toContain("[已讨论的图元素]");
    expect(out).toContain("scripts → (unknown)");
    expect(out).toContain("[最近对话]");
    expect(out).toContain("外部命令调用");
  });

  it("appends the answer-style instruction", () => {
    const out = attachConversationMemory("他是干嘛用的？", {
      workingSetBlock: "",
      currentSelectionBlock: null,
      recentTurns: "",
      styleBlock: "[回答口吻] 适中\n先用人话说清楚干什么",
    });
    expect(out).toContain("[回答口吻] 适中");
  });

  it("still appends current selection for a freeform question", () => {
    const out = attachConversationMemory("两条线有什么关联？", {
      workingSetBlock: "[已讨论的图元素]\n1. A\n2. B  ← 当前",
      currentSelectionBlock: "聚合边：B",
      recentTurns: "",
    });
    expect(out.startsWith("两条线有什么关联？")).toBe(true);
    expect(out).toContain("[当前图谱选择]");
    expect(out).toContain("不要再索要 relationKey");
  });
});

describe("conversationMemoryShouldReset", () => {
  const sessA = { sessionId: "sess:1", snapshotId: "snap:a" };
  const sessB = { sessionId: "sess:2", snapshotId: "snap:b" };

  it("does not reset on the first session bind", () => {
    expect(conversationMemoryShouldReset(null, sessA)).toBe(false);
  });

  it("resets when the backend session is replaced (switch project or re-analyze)", () => {
    expect(conversationMemoryShouldReset(sessA, sessB)).toBe(true);
    expect(conversationMemoryShouldReset(sessA, { sessionId: "sess:2", snapshotId: "snap:a" })).toBe(true);
  });

  it("resets when the snapshot changes under the same session", () => {
    expect(conversationMemoryShouldReset(sessA, { sessionId: "sess:1", snapshotId: "snap:b" })).toBe(true);
  });

  it("keeps the list while chatting in the same session and snapshot", () => {
    expect(conversationMemoryShouldReset(sessA, sessA)).toBe(false);
  });

  it("resets when the session is closed", () => {
    expect(conversationMemoryShouldReset(sessA, { sessionId: "", snapshotId: "" })).toBe(true);
  });
});

describe("formatRecentTurns", () => {
  it("keeps the last few turns and truncates long assistant text", () => {
    const turns = formatRecentTurns([
      { role: "user", text: "问0" },
      { role: "assistant", text: "答0" },
      { role: "user", text: "问A" },
      { role: "assistant", text: "答A".repeat(400) },
      { role: "user", text: "问B" },
      { role: "assistant", text: "答B" },
    ], 2);
    expect(turns).toContain("问B");
    expect(turns).toContain("答B");
    expect(turns).not.toContain("问0");
    expect(turns.length).toBeLessThan(900);
  });
});

describe("visibleUserChatText", () => {
  it("hides the internal explain prompt from the chat bubble", () => {
    expect(visibleUserChatText(
      "请解释这条聚合边关系（已有边向上归并，不是新推断的依赖）：scripts → (unknown)",
      "scripts → (unknown)",
    )).toBe("解释：scripts → (unknown)");
    expect(visibleUserChatText("两条线有什么关联？")).toBe("两条线有什么关联？");
  });
});
