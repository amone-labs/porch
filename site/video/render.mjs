// Renders both hero compositions and both "works" clips and makes the web files: 1600px H.264 + VP9, no audio,
// and a poster frame. The works clip's poster is also its story still (<assets>/works.png, made a WebP by stills.mjs).
// Run after `pnpm hero` and `pnpm works`. Usage: pnpm render
import { execFileSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { POSTER_AT as WORKS_POSTER_AT } from "./build-works.mjs";

const RUNS = { en: "video/hero", ko: "video/hero-ko" };
const ASSETS = { en: "assets-en", ko: "assets" };
const POSTER_AT = "2.6"; // summary scene with its focus box on
mkdirSync("public/video", { recursive: true });

for (const [locale, dir] of Object.entries(RUNS)) {
  const master = `renders/hero-${locale}.mp4`;
  execFileSync("npx", ["hyperframes", "render", "-o", master, "--fps", "30", "-q", "standard"], { cwd: dir, stdio: "inherit" });
  const src = `${dir}/${master}`;
  const out = `public/video/hero-${locale}`;
  const ff = (...a) => execFileSync("ffmpeg", ["-y", "-loglevel", "error", ...a], { stdio: "inherit" });
  ff("-i", src, "-vf", "scale=1600:-2", "-c:v", "libx264", "-preset", "slow", "-crf", "26", "-pix_fmt", "yuv420p", "-movflags", "+faststart", "-an", `${out}.mp4`);
  ff("-i", src, "-vf", "scale=1600:-2", "-c:v", "libvpx-vp9", "-b:v", "0", "-crf", "38", "-row-mt", "1", "-an", `${out}.webm`);
  ff("-ss", POSTER_AT, "-i", src, "-frames:v", "1", "-vf", "scale=1600:-2", "-q:v", "5", `${out}.jpg`);

  // The works clip loops, so it is rendered at its own 2× size and only scaled for the web files.
  const works = `renders/works-${locale}.mp4`;
  execFileSync("npx", ["hyperframes", "render", "-c", "works.html", "-o", works, "--fps", "30", "-q", "standard"], { cwd: dir, stdio: "inherit" });
  const wsrc = `${dir}/${works}`;
  const wout = `public/video/works-${locale}`;
  ff("-i", wsrc, "-vf", "scale=1600:-2", "-c:v", "libx264", "-preset", "slow", "-crf", "26", "-pix_fmt", "yuv420p", "-movflags", "+faststart", "-an", `${wout}.mp4`);
  ff("-i", wsrc, "-vf", "scale=1600:-2", "-c:v", "libvpx-vp9", "-b:v", "0", "-crf", "38", "-row-mt", "1", "-an", `${wout}.webm`);
  ff("-ss", String(WORKS_POSTER_AT), "-i", wsrc, "-frames:v", "1", `${dir}/${ASSETS[locale]}/works.png`);
}
console.log("rendered");
