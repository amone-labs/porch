import { existsSync, statSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { SHOTS, boxes, isClip, isShotScene, shotSrc, type ShotScene } from "./shots";

const SCENES = Object.keys(SHOTS.scenes) as ShotScene[];

describe("shots", () => {
  it("knows which scenes are stills", () => {
    expect(isShotScene("s4")).toBe(true);
    expect(isShotScene("s1")).toBe(false);
  });
  for (const locale of ["en", "ko"] as const) {
    for (const scene of SCENES) {
      it(`${locale} ${scene}: still exists, within budget, boxes inside the crop`, () => {
        const file = `public${shotSrc(locale, scene)}`;
        expect(existsSync(file), file).toBe(true);
        expect(statSync(file).size).toBeLessThanOrEqual(200 * 1024);
        const list = boxes(locale, scene);
        // a clip draws its own highlights; a still needs at least one box
        if (isClip(scene)) expect(list).toHaveLength(0);
        else expect(list.length).toBeGreaterThan(0);
        for (const b of list) {
          expect(b.left).toBeGreaterThanOrEqual(0);
          expect(b.top).toBeGreaterThanOrEqual(0);
          expect(b.left + b.width).toBeLessThanOrEqual(100);
          expect(b.top + b.height).toBeLessThanOrEqual(100);
        }
      });
    }
  }
  it("both languages box the same number of elements", () => {
    for (const scene of SCENES) expect(boxes("ko", scene).length).toBe(boxes("en", scene).length);
  });
  it("suggestions scene boxes the card, then the effect", () => {
    expect(boxes("en", "s8")).toHaveLength(2);
  });
  it("the works scene is a clip with its web files beside its poster", () => {
    expect(isClip("s2")).toBe(true);
    for (const locale of ["en", "ko"]) for (const ext of ["webm", "mp4"]) expect(existsSync(`public/video/works-${locale}.${ext}`)).toBe(true);
  });
  it("notes scene boxes the time table, then the blocked task", () => {
    expect(boxes("en", "s9")).toHaveLength(2);
  });
});
