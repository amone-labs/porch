// Writes the "works" clip compositions (story block "works", scene s2), one per language, next to the hero's:
//   video/hero/works.html     English (assets-en/)
//   video/hero-ko/works.html  Korean  (assets/)
// Three terminal windows start a session each; each session's group of the real porch list (app-now.png, cut by
// the regions `pnpm shoot` records) flies from its terminal into place and gets the terminal's name beside it; then
// the permission row is boxed and the menu bar shows the waiting count, as the app sets it on the tray.
// build-hero.mjs plays the same scene, faster, through `scene()`; styles are video/works.css.
// Run after `pnpm shoot`. Usage: pnpm works (render.mjs renders it)
import { copyFileSync, readFileSync, writeFileSync } from "node:fs";

export const TOTAL = 8;
export const POSTER_AT = 6.2; // every session in, the permission row boxed, the count shown
export const CSS = readFileSync(new URL("./works.css", import.meta.url), "utf8");

const OUT = { en: { dir: "video/hero", assets: "assets-en" }, ko: { dir: "video/hero-ko", assets: "assets" } };
// The capture fixtures' three sessions (app-capture/fixtures.ts), each started where porch's own demo says.
export const TERMS = [
  { app: "Ghostty", kind: "plain", dir: "~/work/shop-web", cmd: "claude", project: "shop-web" },
  { app: "iTerm2", kind: "tabs", dir: "~/work/shop-api", cmd: "codex", project: "shop-api" },
  { app: "VS Code", kind: "vscode", dir: "~/work/porch-site", cmd: "claude", project: "porch-site" },
];
const TERM = { top: 46, w: 352, h: 160, gap: 22, left: 40 };
const LIST = { left: 150, top: 236, w: 990 };

const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;");

function terminal(t, i, p) {
  const left = TERM.left + i * (TERM.w + TERM.gap);
  const prompt = `<span class="wk-dim">${esc(t.dir)}</span>\n❯ <span class="wk-typed" id="${p}t${i}">${t.cmd}</span><span class="wk-cursor" id="${p}k${i}"></span>\n<span class="wk-cursor2" id="${p}n${i}"></span>`;
  const bar = (title) => `<div class="wk-bar"><i></i><i></i><i></i><span class="wk-app">${t.app}</span>${title ? `<span class="wk-title">${esc(title)}</span>` : ""}</div>`;
  let inner;
  if (t.kind === "plain") inner = `${bar(t.dir)}<div class="wk-code" style="top:28px"><pre>${prompt}</pre></div>`;
  else if (t.kind === "tabs") inner = `${bar("")}<div class="wk-tabs"><span><b id="${p}tz${i}">zsh</b><b id="${p}tab${i}">${t.cmd}</b></span></div><div class="wk-code" style="top:52px"><pre>${prompt}</pre></div>`;
  else
    inner = `${bar("porch-site")}<div class="wk-vs-act"><b></b><b></b><b></b><b></b></div><div class="wk-vs-editor"><s style="width:62%"></s><s style="width:44%"></s><s style="width:70%"></s></div>
          <div class="wk-vs-panel"><div class="wk-vs-head"><span>TERMINAL</span><span>PROBLEMS</span><span>OUTPUT</span></div><pre>${prompt}</pre></div>`;
  return `        <div class="wk-term" style="left:${left}px;height:${TERM.h}px">${inner}<div class="wk-glow" id="${p}g${i}"></div></div>`;
}

/**
 * The scene for one language: its markup (inside a `.wk` stage) and GSAP lines on a timeline named `tl`.
 * `p` prefixes every id so the scene can sit in another composition; `at` is when the first terminal starts and
 * `step` the gap between terminals. `bar` widens the menu bar past the stage ({ left, width } in stage px).
 * `end` is when the count has shown (the scene can leave after it).
 */
export function scene(lang, assets, { p = "", at = 0.35, step = 1.3, bar } = {}) {
  const regions = JSON.parse(readFileSync(`video/hero/regions-${lang}.json`, "utf8"));
  const focus = JSON.parse(readFileSync(`video/hero/focus-${lang}.json`, "utf8"));
  const list = regions["app-now-list"];
  const s = LIST.w / list.w;

  const groups = TERMS.map((t, i) => {
    const g = regions[`app-now-${t.project}`];
    if (!g) throw new Error(`${lang}: no region app-now-${t.project}; run pnpm shoot`);
    const top = LIST.top + (g.y - list.y) * s;
    const h = g.h * s;
    const style = `top:${top.toFixed(1)}px;height:${h.toFixed(1)}px;background-image:url(${assets}/app-now.png);background-size:${(1180 * s).toFixed(1)}px auto;background-position:${(-list.x * s).toFixed(1)}px ${(-g.y * s).toFixed(1)}px`;
    // the tag sits beside the group's header line (project name, branch)
    const tagTop = top + 17 * s - 9;
    const termCx = TERM.left + i * (TERM.w + TERM.gap) + TERM.w / 2;
    const termCy = TERM.top + TERM.h * 0.62;
    const fly = { x: termCx - (LIST.left + LIST.w / 2), y: termCy - (top + h / 2) };
    return {
      html: `        <div class="wk-group" id="${p}s${i}" style="${style}"></div>\n        <div class="wk-tag" id="${p}a${i}" style="top:${tagTop.toFixed(1)}px;height:18px">${t.app}</div>`,
      fly,
    };
  });
  // The permission row: the box `pnpm shoot` draws on the session status screen.
  const b = focus["app-now"][0];
  const fx = LIST.left + Math.max(0, b.x - list.x) * s;
  const fw = Math.min(b.w * s, LIST.left + LIST.w - fx);
  const box = `left:${fx.toFixed(1)}px;top:${(LIST.top + (b.y - list.y) * s).toFixed(1)}px;width:${fw.toFixed(1)}px;height:${(b.h * s).toFixed(1)}px`;

  const body = [
    `        <div class="wk-menubar"${bar ? ` style="left:${bar.left}px;width:${bar.width}px"` : ""}><span class="wk-tray"><img src="${assets}/tray.png" alt=""><span class="wk-count" id="${p}count">1</span><span class="wk-ring" id="${p}ring"></span></span><span>17:40</span></div>`,
    ...TERMS.map((t, i) => terminal(t, i, p)),
    ...groups.map((g) => g.html),
    `        <div class="wk-list" style="top:${LIST.top}px;height:${(list.h * s).toFixed(1)}px"></div>`,
    `        <div class="wk-focus" id="${p}focus" style="${box}"></div>`,
  ].join("\n");

  const n = (v) => +v.toFixed(2);
  const lines = [];
  TERMS.forEach((t, i) => {
    const t0 = at + i * step;
    const { x, y } = groups[i].fly;
    lines.push(
      `      // ${t.app}: type \`${t.cmd}\`, start it, and its session lands in porch's list`,
      `      tl.to("#${p}t${i}", { width: "${t.cmd.length}ch", duration: 0.45, ease: "steps(${t.cmd.length})" }, ${n(t0)});`,
      `      tl.set("#${p}k${i}", { opacity: 0 }, ${n(t0 + 0.6)}); tl.set("#${p}n${i}", { opacity: 1 }, ${n(t0 + 0.6)});`,
      ...(t.kind === "tabs" ? [`      tl.set("#${p}tab${i}", { opacity: 1 }, ${n(t0 + 0.6)}); tl.set("#${p}tz${i}", { opacity: 0 }, ${n(t0 + 0.6)});`] : []),
      `      tl.fromTo("#${p}g${i}", { opacity: 0 }, { opacity: 1, duration: 0.2 }, ${n(t0 + 0.6)}); tl.to("#${p}g${i}", { opacity: 0, duration: 0.4 }, ${n(t0 + 1.5)});`,
      `      tl.fromTo("#${p}s${i}", { opacity: 0 }, { opacity: 1, duration: 0.2 }, ${n(t0 + 0.65)});`,
      `      tl.fromTo("#${p}s${i}", { x: ${x.toFixed(1)}, y: ${y.toFixed(1)}, scale: 0.3, boxShadow: "0 18px 40px rgba(0,0,0,0.6), 0 0 0 1px rgba(237,237,240,0.35)" }, { x: 0, y: 0, scale: 1, boxShadow: "0 0px 0px rgba(0,0,0,0), 0 0 0 0px rgba(237,237,240,0)", duration: 0.8, ease: "power3.inOut" }, ${n(t0 + 0.65)});`,
      `      tl.fromTo("#${p}a${i}", { opacity: 0, x: -8 }, { opacity: 1, x: 0, duration: 0.35 }, ${n(t0 + 1.35)});`,
    );
  });
  const last = at + (TERMS.length - 1) * step + 1.45;
  lines.push(
    `      // the session waiting for permission: boxed, and the menu bar shows the count`,
    `      tl.fromTo("#${p}focus", { opacity: 0 }, { opacity: 1, duration: 0.4 }, ${n(last)});`,
    `      tl.fromTo("#${p}count", { opacity: 0, scale: 0.6 }, { opacity: 1, scale: 1, duration: 0.35, ease: "back.out(2)" }, ${n(last + 0.15)});`,
    `      tl.fromTo("#${p}ring", { opacity: 0 }, { opacity: 1, duration: 0.3 }, ${n(last + 0.15)});`,
    `      tl.to("#${p}ring", { opacity: 0.3, duration: 0.5, ease: "sine.inOut", yoyo: true, repeat: 1 }, ${n(last + 0.5)});`,
  );
  return { body, lines, end: last + 0.5 };
}

function build(lang) {
  const { dir, assets } = OUT[lang];
  const { body, lines } = scene(lang, assets);
  const ids = (...names) => JSON.stringify(TERMS.flatMap((_, i) => names.map((x) => `#${x}${i}`)));
  lines.push(
    `      // back to the empty start, so the loop has no jump`,
    `      tl.to(${JSON.stringify(JSON.parse(ids("s", "a", "t", "n")).concat(["#focus", "#count", "#ring"]))}, { opacity: 0, duration: 0.5, ease: "power2.in" }, ${TOTAL - 0.9});`,
    `      tl.set(${ids("t")}, { width: 0, opacity: 1 }, ${TOTAL - 0.3});`,
    `      tl.set(${ids("k")}, { opacity: 1 }, ${TOTAL - 0.3});`,
    ...TERMS.flatMap((t, i) => (t.kind === "tabs" ? [`      tl.set("#tab${i}", { opacity: 0 }, ${TOTAL - 0.3}); tl.set("#tz${i}", { opacity: 1 }, ${TOTAL - 0.3});`] : [])),
    `      tl.set("#stage", { opacity: 1 }, ${TOTAL});`,
  );
  const html = readFileSync("video/works.template.html", "utf8")
    .replaceAll("{{lang}}", lang)
    .replaceAll("{{total}}", String(TOTAL))
    .replace("{{css}}", CSS)
    .replace("{{body}}", body)
    .replace("{{timeline}}", lines.join("\n"));
  writeFileSync(`${dir}/works.html`, html);
  console.log(`${dir}/works.html: ${TOTAL}s`);
}

if (import.meta.url === `file://${process.argv[1]}`) {
  for (const { dir, assets } of Object.values(OUT)) copyFileSync("../app/src-tauri/icons/tray.png", `${dir}/${assets}/tray.png`);
  for (const lang of Object.keys(OUT)) build(lang);
  // the poster still's size, beside the app captures' (pnpm shoot rewrites this file, so this runs after it)
  const sizes = JSON.parse(readFileSync("video/hero/sizes.json", "utf8"));
  writeFileSync("video/hero/sizes.json", JSON.stringify({ ...sizes, works: [1180, 640] }, null, 2) + "\n");
}
