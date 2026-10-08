// Shoots the notes still (scene s9): porch's own notes, open in the real Obsidian app.
//   1. Saves the capture fixtures' day (plus the day before and last week, headline only) as reports in a throwaway
//      PORCH_HOME and lets the real `porch mirror set` write them into a throwaway vault. Nothing here writes Markdown.
//   2. Opens that vault in /Applications/Obsidian.app with its own settings folder, so the person's Obsidian (its
//      vaults, settings and windows) is never read from or written to, apart from copying its newest app update.
//   3. Screenshots the window through the DevTools protocol at 2×, like `pnpm shoot`.
// Writes <assets>/obsidian-note.png, adds its focus boxes to video/hero/focus-<lang>.json and its size to sizes.json,
// and keeps the open note as video/hero/notes-<lang>.md, which delivered.test.ts holds the phone card to.
// Needs `cargo build --release -p porch-cli` and Obsidian. Run after `pnpm shoot`. Usage: pnpm obsidian
import { execFileSync, spawn } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { chromium } from "@playwright/test";
import { toEnglish } from "./app-capture/en.ts";
import { fixtures } from "./app-capture/fixtures.ts";

const PORCH = resolve("../target/release/porch");
const OBSIDIAN = process.env.OBSIDIAN_BIN ?? "/Applications/Obsidian.app/Contents/MacOS/Obsidian";
const WORK = resolve("video/.obsidian");
const DATA = join(WORK, "app-data"); // Obsidian's settings folder for this run only
const PORT = 9333;
const DIRS = { en: "video/hero/assets-en", ko: "video/hero-ko/assets" };
const NAME = "obsidian-note";
const W = 1180;
const H = 1140; // taller than the app captures: the note runs from its title to the end of the blocked task
// The vault's name shows at the foot of Obsidian's sidebar.
const VAULT = { en: "Notes", ko: "노트" };
const SUB = "porch";
const OPEN = `${SUB}/daily/2026-09-29.md`;
// The day before and last week exist only so the folder holds more than one note; their text never shows.
const OTHER = {
  en: { day: "Login design, alert queue cleanup", week: "Checkout redesign, alert queue rollout" },
  ko: { day: "로그인 기능 설계, 알림 큐 정리", week: "결제 화면 개편, 알림 큐 도입" },
};

for (const [what, path] of [["porch (cargo build --release -p porch-cli)", PORCH], ["Obsidian", OBSIDIAN]]) {
  if (!existsSync(path)) throw new Error(`${what} not found at ${path}`);
}

// The app never reads the day's window or a session's prompts, replies, commands or files, so the fixtures leave
// them out; a saved report has them. Fill them (from the session's own turns) so the report loads.
function asSaved(day) {
  const projects = day.digest.projects.map((p) => ({
    ...p,
    sessions: p.sessions.map((s) => ({
      cwd: p.root,
      prompts: s.turns.map((t) => t.prompt),
      replies: [],
      recaps: [],
      commands: [],
      files: [],
      tool_calls: s.turns.reduce((n, t) => n + t.tool_calls, 0),
      tool_errors: s.turns.reduce((n, t) => n + t.tool_errors, 0),
      denials: s.turns.reduce((n, t) => n + t.denials, 0),
      ...s,
    })),
  }));
  // The capture day runs midnight to midnight in KST, as in app-capture/fixtures.ts.
  const start = Date.parse(`${day.date}T00:00:00+09:00`);
  return { ...day, digest: { start, end: start + 86_400_000, ...day.digest, projects } };
}

const deep = (v, f) =>
  typeof v === "string" ? f(v) : Array.isArray(v) ? v.map((x) => deep(x, f)) : v && typeof v === "object" ? Object.fromEntries(Object.entries(v).map(([k, x]) => [k, deep(x, f)])) : v;

/** The notes porch writes for this language, in `vault`/porch. */
function writeNotes(lang, vault) {
  const home = join(WORK, `porch-home-${lang}`);
  rmSync(home, { recursive: true, force: true });
  mkdirSync(join(home, "reports"), { recursive: true });
  mkdirSync(join(vault, SUB), { recursive: true });
  let day = asSaved(fixtures.day_report());
  if (lang === "en") day = deep(day, toEnglish);
  day.summary = { ...day.summary, lang };
  const before = {
    ...day,
    date: "2026-09-28",
    generated_at: day.generated_at - 86_400_000,
    digest: { ...day.digest, date: "2026-09-28", start: day.digest.start - 86_400_000, end: day.digest.start, projects: [] },
    summary: { headline: OTHER[lang].day, lang },
  };
  const week = {
    week_start: "2026-09-21",
    generated_at: day.generated_at - 2 * 86_400_000,
    model: day.model,
    days: [],
    summary: { headline: OTHER[lang].week, lang },
    summary_error: null,
  };
  writeFileSync(join(home, "reports", `${day.date}.json`), JSON.stringify(day));
  writeFileSync(join(home, "reports", `${before.date}.json`), JSON.stringify(before));
  writeFileSync(join(home, "reports", `week-${week.week_start}.json`), JSON.stringify(week));
  writeFileSync(join(home, "settings.json"), JSON.stringify({ language: lang }));
  execFileSync(PORCH, ["mirror", "set", join(vault, SUB)], { env: { ...process.env, PORCH_HOME: home }, stdio: "pipe" });
  for (const f of [OPEN, `${SUB}/daily/2026-09-28.md`, `${SUB}/weekly/2026-W39.md`]) {
    if (!existsSync(join(vault, f))) throw new Error(`porch did not write ${f}: see the reports in ${home}`);
  }
  copyFileSync(join(vault, OPEN), `video/hero/notes-${lang}.md`);
  rmSync(home, { recursive: true, force: true });
}

/** A fresh settings folder that knows only this vault, with the person's newest Obsidian update if they have one. */
function prepareObsidian(lang, vault) {
  rmSync(DATA, { recursive: true, force: true });
  mkdirSync(DATA, { recursive: true });
  const theirs = join(homedir(), "Library/Application Support/obsidian");
  const newest = existsSync(theirs)
    ? readdirSync(theirs)
        .filter((f) => /^obsidian-[\d.]+\.asar$/.test(f))
        .sort((a, b) => a.localeCompare(b, undefined, { numeric: true }))
        .at(-1)
    : undefined;
  if (newest) copyFileSync(join(theirs, newest), join(DATA, newest));
  writeFileSync(join(DATA, "obsidian.json"), JSON.stringify({ vaults: { porchsite0000001: { path: vault, ts: Date.now(), open: true } } }));
  mkdirSync(join(vault, ".obsidian"), { recursive: true });
  writeFileSync(join(vault, ".obsidian/appearance.json"), JSON.stringify({ theme: "obsidian" }));
  writeFileSync(join(vault, ".obsidian/app.json"), JSON.stringify({ defaultViewMode: "preview", showInlineTitle: false, propertiesInDocument: "visible" }));
}

async function cdpPage() {
  for (let i = 0; i < 60; i++) {
    try {
      const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
      if (list.some((t) => t.type === "page" && t.url.startsWith("app://obsidian.md/index.html"))) break;
    } catch {}
    await new Promise((r) => setTimeout(r, 500));
  }
  const browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  const page = browser.contexts()[0].pages().find((p) => p.url().startsWith("app://obsidian.md/index.html"));
  if (!page) throw new Error("Obsidian opened no vault window");
  await page.waitForFunction(() => window.app?.workspace?.layoutReady, null, { timeout: 30_000 });
  return { browser, page };
}

async function shoot(lang) {
  const vault = join(WORK, `vault-${lang}`, VAULT[lang]);
  rmSync(join(WORK, `vault-${lang}`), { recursive: true, force: true });
  writeNotes(lang, vault);
  prepareObsidian(lang, vault);
  const proc = spawn(OBSIDIAN, [`--user-data-dir=${DATA}`, `--remote-debugging-port=${PORT}`], { stdio: "ignore" });
  try {
    let { browser, page } = await cdpPage();
    // Obsidian's own interface language lives in its local storage (unset, it follows the Mac); set it, reload once.
    if ((await page.evaluate(() => localStorage.getItem("language"))) !== lang) {
      await page.evaluate((l) => localStorage.setItem("language", l), lang);
      await page.evaluate(() => location.reload());
      await page.waitForFunction(() => window.app?.workspace?.layoutReady, null, { timeout: 30_000 });
    }
    const cdp = await page.context().newCDPSession(page);
    await cdp.send("Emulation.setDeviceMetricsOverride", { width: W, height: H, deviceScaleFactor: 2, mobile: false });
    await page.evaluate(
      async ({ open, sub }) => {
        const app = window.app;
        for (const b of document.querySelectorAll(".modal-close-button")) b.click();
        // Sync and Publish are paid services nobody set up here; their status icons would only add noise.
        for (const id of ["sync", "publish"]) await app.internalPlugins.getPluginById(id)?.disable(false);
        app.workspace.rightSplit.collapse();
        const file = app.vault.getAbstractFileByPath(open);
        await app.workspace.getLeaf(false).openFile(file, { state: { mode: "preview" } });
        const explorer = app.workspace.getLeavesOfType("file-explorer")[0].view;
        explorer.revealInFolder(file);
        for (const p of [sub, `${sub}/daily`, `${sub}/weekly`]) explorer.fileItems[p]?.setCollapsed(false);
        // revealInFolder also gives the row keyboard focus; keep only the open-file highlight
        explorer.tree?.setFocusedItem?.(null);
      },
      { open: OPEN, sub: SUB },
    );
    await page.waitForTimeout(800);
    // Fold the note's properties: the summary is what the still is about.
    await page.evaluate(() => {
      const head = document.querySelector(".markdown-reading-view .metadata-properties-heading");
      if (head && !head.closest(".metadata-container")?.classList.contains("is-collapsed")) head.click();
      document.activeElement?.blur();
    });
    await page.mouse.move(W - 1, H - 1);
    await page.waitForTimeout(800);
    // Playwright's own screenshot ignores the emulated 2× on a page it attached to; ask the protocol directly.
    const { data } = await cdp.send("Page.captureScreenshot", { format: "png" });
    writeFileSync(`${DIRS[lang]}/${NAME}.png`, Buffer.from(data, "base64"));
    // Focus boxes: the time-by-task table, then the blocked task (its heading down to its last line).
    const boxes = await page.evaluate(() => {
      const view = document.querySelector(".markdown-reading-view");
      const pad = 6;
      const box = (top, bottom) => ({ x: Math.round(top.left - pad), y: Math.round(top.top - pad), w: Math.round(top.width + pad * 2), h: Math.round(bottom.bottom - top.top + pad * 2) });
      const table = view.querySelector("table").getBoundingClientRect();
      const h3 = view.querySelector("h3");
      const list = h3.closest(".el-h3").nextElementSibling?.nextElementSibling ?? h3;
      return [box(table, table), box(h3.getBoundingClientRect(), list.getBoundingClientRect())];
    });
    for (const b of boxes) if (b.y + b.h > H) throw new Error(`${lang}: a focus box runs past the bottom of the capture (${JSON.stringify(b)})`);
    const focusFile = `video/hero/focus-${lang}.json`;
    const focus = JSON.parse(readFileSync(focusFile, "utf8"));
    focus[NAME] = boxes;
    writeFileSync(focusFile, JSON.stringify(focus, null, 2) + "\n");
    await browser.close().catch(() => {});
  } finally {
    proc.kill("SIGTERM");
    await new Promise((r) => proc.once("exit", r));
  }
}

for (const lang of Object.keys(DIRS)) await shoot(lang);
const sizesFile = "video/hero/sizes.json";
const sizes = JSON.parse(readFileSync(sizesFile, "utf8"));
writeFileSync(sizesFile, JSON.stringify({ ...sizes, [NAME]: [W, H] }, null, 2) + "\n");
rmSync(DATA, { recursive: true, force: true });
console.log("obsidian notes shot");
