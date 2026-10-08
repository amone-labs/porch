import { describe, expect, it } from "vitest";
import { emph, fill, fmt, labelClass } from "./util";

describe("fill", () => {
  it("expands agent lists in both languages", () => {
    expect(fill("Your work with {agents},", "en")).toBe("Your work with Claude Code and Codex,");
    expect(fill("{agents:로} 한 일을", "ko")).toBe("클로드코드와 코덱스로 한 일을");
    expect(fill("{agents:를} 지원합니다.", "ko")).toBe("클로드코드와 코덱스를 지원합니다.");
    expect(fill("이 맥의 {agents:dot} 작업 기록", "ko")).toBe("이 맥의 클로드코드·코덱스 작업 기록");
    expect(fill("{agents:amp} · MACOS", "en")).toBe("Claude Code & Codex · MACOS");
  });
  it("leaves text without placeholders alone", () => {
    expect(fill("plain", "en")).toBe("plain");
  });
});

describe("fmt", () => {
  it("replaces named values", () => {
    expect(fmt("{a} · {b}", { a: "x", b: 3 })).toBe("x · 3");
  });
  it("throws on a missing value so a typo cannot ship", () => {
    expect(() => fmt("{a} {c}", { a: "x" })).toThrow(/c/);
  });
});

describe("emph", () => {
  it("escapes HTML and bolds **text**", () => {
    expect(emph("Click **Make <summary>** now")).toBe("Click <strong>Make &lt;summary&gt;</strong> now");
  });
});

describe("labelClass", () => {
  it("keeps Korean eyebrows out of mono caps", () => {
    expect(labelClass("en")).toBe("eyebrow");
    expect(labelClass("ko")).toBe("eyebrow-ko");
  });
});

describe("emph markers", () => {
  it("turns _word_ into the serif accent and [[x]] into a chip", () => {
    expect(emph("Your work, _summarized_.")).toBe('Your work, <em class="serif">summarized</em>.');
    expect(emph("Ask for [[a summary]] today")).toBe('Ask for <mark class="chip">a summary</mark> today');
  });
  it("still escapes HTML inside markers", () => {
    expect(emph("[[<b>]]")).toBe('<mark class="chip">&lt;b&gt;</mark>');
  });
});
