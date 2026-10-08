import focusEn from "../video/hero/focus-en.json";
import focusKo from "../video/hero/focus-ko.json";
import shots from "../video/shots.json";
import sizes from "../video/hero/sizes.json";
import type { Locale } from "./i18n/types";

/** Story stills: real app screenshots from `pnpm shoot`, porch's note in Obsidian (s9, `video/obsidian.mjs`) and the
 * "works" clip's poster (s2, `video/build-works.mjs`), written as WebP by `video/stills.mjs`. App stills keep the whole
 * window, sidebar included. A clip scene plays /video/<shot>-<locale>.{webm,mp4} with its still as the poster. */
export const SHOTS = shots as {
  crop: { x: number; y: number; w: number; h: number };
  scenes: Record<ShotScene, string>;
  composed: ShotScene[];
  clips: ShotScene[];
};
export type ShotScene = "s2" | "s4" | "s5" | "s6" | "s8" | "s9";
export interface Box { left: number; top: number; width: number; height: number }

type Raw = Record<string, { x: number; y: number; w: number; h: number }[]>;
const FOCUS: Record<Locale, Raw> = { en: focusEn as Raw, ko: focusKo as Raw };
const pct = (v: number, of: number) => Math.round((v / of) * 10000) / 100;

export function isShotScene(id: string): id is ShotScene {
  return id in SHOTS.scenes;
}

/** CSS-pixel size of a scene's still: the capture window, taller for screens that run long. */
export function shotSize(scene: ShotScene): [number, number] {
  const s = sizes as unknown as Record<string, [number, number]>;
  return s[SHOTS.scenes[scene]] ?? s.default;
}

/** Focus boxes in percent of the still, clamped to its edges. */
export function boxes(locale: Locale, scene: ShotScene): Box[] {
  const { x, y } = SHOTS.crop;
  const [w, h] = shotSize(scene);
  return (FOCUS[locale][SHOTS.scenes[scene]] ?? []).map((b) => {
    const left = Math.max(0, b.x - x);
    const top = Math.max(0, b.y - y);
    return { left: pct(left, w), top: pct(top, h), width: pct(Math.min(b.w, w - left), w), height: pct(Math.min(b.h, h - top), h) };
  });
}

export function shotSrc(locale: Locale, scene: ShotScene): string {
  return `/shots/${locale}/${SHOTS.scenes[scene]}.webp`;
}

/** True for scenes that play a looping clip, with their still as its poster. */
export function isClip(scene: ShotScene): boolean {
  return SHOTS.clips.includes(scene);
}

/** The clip's web files, WebM first. */
export function clipSrcs(locale: Locale, scene: ShotScene): { src: string; type: string }[] {
  const base = `/video/${SHOTS.scenes[scene]}-${locale}`;
  return [
    { src: `${base}.webm`, type: "video/webm" },
    { src: `${base}.mp4`, type: "video/mp4" },
  ];
}

/** True for stills that are a single app window (shown with window chrome); false for composed scenes. */
export function isAppWindow(scene: ShotScene): boolean {
  return !SHOTS.composed.includes(scene);
}
