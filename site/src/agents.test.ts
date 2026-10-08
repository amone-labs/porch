import { describe, expect, it } from "vitest";
import { AGENTS, FEATURES, agentNames, joinNames, josa } from "./agents";

describe("josa", () => {
  it("picks the particle from the last syllable", () => {
    expect(josa("코덱스", "로")).toBe("코덱스로");
    expect(josa("커맨드", "와")).toBe("커맨드와");
    expect(josa("깃허브", "로")).toBe("깃허브로");
    expect(josa("클로드코드", "를")).toBe("클로드코드를");
    expect(josa("수업", "로")).toBe("수업으로");
    expect(josa("수업", "와")).toBe("수업과");
    expect(josa("수업", "를")).toBe("수업을");
    expect(josa("서울", "로")).toBe("서울로"); // ㄹ takes 로
  });
  it("falls back to the vowel form after a non-Hangul last character", () => {
    expect(josa("Gemini CLI", "로")).toBe("Gemini CLI로");
  });
});

describe("joinNames", () => {
  it("joins two and three names in English", () => {
    expect(joinNames(["A", "B"], "en")).toBe("A and B");
    expect(joinNames(["A", "B", "C"], "en")).toBe("A, B and C");
  });
  it("joins with 와/과 in Korean", () => {
    expect(joinNames(["클로드코드", "코덱스"], "ko")).toBe("클로드코드와 코덱스");
    expect(joinNames(["수업", "코덱스"], "ko")).toBe("수업과 코덱스");
    expect(joinNames(["가", "나", "다"], "ko")).toBe("가, 나와 다");
  });
  it("returns a single name unchanged", () => {
    expect(joinNames(["A"], "en")).toBe("A");
  });
});

describe("AGENTS", () => {
  it("lists exactly the agents porch supports today", () => {
    expect(agentNames("en")).toEqual(["Claude Code", "Codex"]);
    expect(agentNames("ko")).toEqual(["클로드코드", "코덱스"]);
  });
  it("fills every feature in both languages and cites a source", () => {
    for (const a of AGENTS) {
      for (const f of FEATURES) {
        expect(a.features[f].en.length, `${a.id}.${f}.en`).toBeGreaterThan(0);
        expect(a.features[f].ko.length, `${a.id}.${f}.ko`).toBeGreaterThan(0);
        expect(a.sources[f].length, `${a.id}.${f} source`).toBeGreaterThan(0);
      }
    }
  });
  it("keeps Codex's question support unverified until a recording proves it", () => {
    const codex = AGENTS.find((a) => a.id === "codex")!;
    expect(codex.features.question.ko).toBe("확인 필요");
  });
});
