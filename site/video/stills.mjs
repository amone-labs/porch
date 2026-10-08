// Crops the hero video's screenshots (2× PNG) into the story stills: the app's main pane, WebP.
// Run after `pnpm shoot`. Usage: pnpm stills
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync } from "node:fs";

const { crop, scenes } = JSON.parse(readFileSync("video/shots.json", "utf8"));
const SRC = { en: "video/hero/assets-en", ko: "video/hero-ko/assets" };
const S = 2; // screenshots are taken at deviceScaleFactor 2

for (const [locale, dir] of Object.entries(SRC)) {
  mkdirSync(`public/shots/${locale}`, { recursive: true });
  for (const shot of new Set(Object.values(scenes))) {
    // crop.x/y trim the left/top edge; width and height follow each capture (usage is taller than the rest)
    const args = crop.x || crop.y ? ["-quiet", "-q", "80", "-crop", ...[crop.x, crop.y, crop.w, crop.h].map((v) => String(v * S))] : ["-quiet", "-q", "80"];
    execFileSync("cwebp", [...args, `${dir}/${shot}.png`, "-o", `public/shots/${locale}/${shot}.webp`]);
  }
}
console.log("stills written");
