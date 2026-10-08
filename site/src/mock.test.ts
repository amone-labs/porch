import { describe, expect, it } from "vitest";
import { MOCK, fmtCost, fmtDuration, sharePct } from "./mock";

describe("mock numbers", () => {
  it("formats durations like the app", () => {
    expect(fmtDuration(312, "en")).toBe("5h 12m");
    expect(fmtDuration(74, "en")).toBe("1h 14m");
    expect(fmtDuration(52, "en")).toBe("52m");
    expect(fmtDuration(312, "ko")).toBe("5시간 12분");
    expect(fmtDuration(52, "ko")).toBe("52분");
    expect(fmtDuration(60, "ko")).toBe("1시간");
  });
  it("rounds shares to whole percentages of the day's work time", () => {
    expect(sharePct(74, 312)).toBe(24);
    expect(sharePct(52, 312)).toBe(17);
  });
  it("keeps tasks inside the day's total", () => {
    const sum = MOCK.tasks.reduce((s, t) => s + t.min, 0);
    expect(sum).toBeLessThanOrEqual(MOCK.totalMin);
  });
  it("matches the copy appendix numbers", () => {
    expect(MOCK.totalMin).toBe(312);
    expect(MOCK.tasks.map((t) => t.min)).toEqual([74, 52]);
    expect(MOCK.tasks.map((t) => fmtCost(t.cost))).toEqual(["$8.20", "$3.10"]);
    expect([MOCK.projects, MOCK.sessions, MOCK.requests, MOCK.commits]).toEqual([3, 7, 41, 9]);
    expect(MOCK.timeline).toHaveLength(MOCK.projects);
  });
});
