import { describe, expect, it } from "vitest";
import { POLICY_EMAIL, policy } from "./policy";

describe("privacy policy", () => {
  it("has the same sections and items in both languages", () => {
    expect(policy.en.sections.map((s) => s.items.length)).toEqual(policy.ko.sections.map((s) => s.items.length));
  });
  it("names the operator, the privacy officer's address and the effective date", () => {
    for (const p of [policy.en, policy.ko]) {
      expect(p.intro).toContain("amone-labs");
      expect(JSON.stringify(p.sections)).toContain(POLICY_EMAIL);
    }
    expect(policy.ko.effective).toContain("2026년 9월 30일");
    expect(policy.en.effective).toContain("September 30, 2026");
  });
});
