// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import {
  DEFAULT_EXPLAIN_STYLE,
  detectExplainStyleOverride,
  formatExplainStylePrompt,
  readExplainStyle,
  resolveExplainStyle,
  writeExplainStyle,
} from "./explain-style";

afterEach(() => {
  localStorage.removeItem("codelattice.explainStyle");
});

describe("explain style", () => {
  it("defaults to balanced and round-trips through localStorage", () => {
    expect(readExplainStyle()).toBe(DEFAULT_EXPLAIN_STYLE);
    writeExplainStyle("plain");
    expect(readExplainStyle()).toBe("plain");
  });

  it("overrides only the current turn from the user wording", () => {
    expect(detectExplainStyleOverride("没明白，他是干嘛用的？")).toBeNull();
    expect(detectExplainStyleOverride("讲人话，这是干嘛的")).toBe("plain");
    expect(detectExplainStyleOverride("讲专业点")).toBe("pro");
    expect(resolveExplainStyle("balanced", "讲人话再说一遍")).toBe("plain");
    expect(resolveExplainStyle("plain", "两条线有什么关联？")).toBe("plain");
  });

  it("writes a concrete instruction for each style", () => {
    expect(formatExplainStylePrompt("plain")).toContain("最多四句");
    expect(formatExplainStylePrompt("plain")).toContain("不要用");
    expect(formatExplainStylePrompt("balanced")).toContain("除非用户问风险");
    expect(formatExplainStylePrompt("pro")).toContain("只能当假设");
  });
});
