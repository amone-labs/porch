import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { en } from "./en";
import { ko } from "./ko";

const appendix = readFileSync(new URL("../../docs/landing-page-copy.md", import.meta.url), "utf8");

function strings(v: unknown): string[] {
  if (typeof v === "string") return [v];
  if (Array.isArray(v)) return v.flatMap(strings);
  if (v && typeof v === "object") return Object.values(v).flatMap(strings);
  return [];
}

describe("copy appendix", () => {
  it("contains every mock and scene string the page ships", () => {
    for (const t of [en, ko]) {
      for (const s of [...strings(t.mock), ...strings(t.scenes)]) {
        if (["bright", "grey", "ring"].includes(s) || s.includes("{")) continue;
        expect(appendix.includes(s), s).toBe(true);
      }
    }
  });
});
