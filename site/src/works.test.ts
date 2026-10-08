import { describe, expect, it } from "vitest";
import { TERMS } from "../video/build-works.mjs";
import { en } from "./i18n/en";
import { ko } from "./i18n/ko";

// Scene s2's clip draws the terminals; its phone card types the same sessions by hand. Keep the two in step.
describe("scene s2 (works clip)", () => {
  it("the card pairs each session with the terminal the clip starts it in", () => {
    for (const t of [en, ko]) {
      expect(t.scenes.windows.map((w) => w.app)).toEqual(TERMS.map((x) => x.app));
      expect(t.scenes.windows.map((w) => w.line)).toEqual(TERMS.map((x) => x.cmd));
      expect(t.scenes.sessions.map((s) => s.project)).toEqual(TERMS.map((x) => x.project));
    }
  });
  it("the card's states follow the capture fixtures' board, waiting session first", () => {
    for (const t of [en, ko]) expect(t.scenes.sessions.map((s) => s.dot)).toEqual(["bright", "ring", "grey"]);
  });
});
