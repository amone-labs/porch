// Regenerates §1 and the story blocks (§2 on) and "S Scene data" of the copy appendix from src/i18n/*.ts, so the appendix
// (the copy's source of truth for review) never drifts from what the page ships.
// Usage: pnpm appendix
import { readFileSync, writeFileSync } from "node:fs";
import { en } from "../src/i18n/en.ts";
import { ko } from "../src/i18n/ko.ts";

const FILE = new URL("../docs/landing-page-copy.md", import.meta.url);
const L: string[] = [];
const row = (k: string, e: string, o: string) => L.push(`| ${k} | ${e} | ${o} |`);
const head = (title: string) => L.push(title, "", "| | EN | KO |", "| --- | --- | --- |");

head("## 1 Hero");
row("Eyebrow", en.hero.eyebrow, ko.hero.eyebrow);
row("Headline", en.hero.headline.join(" / "), ko.hero.headline.join(" / "));
en.hero.subhead.forEach((s, i) => row(`Subhead ${i + 1}`, s, ko.hero.subhead[i]));
row("Condition", en.hero.condition, ko.hero.condition);
row("CTA", en.hero.cta, ko.hero.cta);
row("CTA aside (serif)", en.hero.aside, ko.hero.aside);
row("Agent link", en.hero.agentLink, ko.hero.agentLink);
row("Hero video caption", en.hero.caption, ko.hero.caption);
row("Hero video label / play / pause", `${en.hero.video.label} / ${en.hero.video.play} / ${en.hero.video.pause}`, `${ko.hero.video.label} / ${ko.hero.video.play} / ${ko.hero.video.pause}`);
row("English UI note", en.shots.uiNote, ko.shots.uiNote || "(none)");
L.push("", "Templates: `{agents…}` expands from `site/src/agents.ts`; `_word_` is the serif accent (Latin only).", "");

en.story.forEach((b, i) => {
  const k = ko.story[i];
  head(`## ${i + 2} Story · ${b.id} (scene ${b.scene})`);
  row("Eyebrow", b.eyebrow, k.eyebrow);
  row("Headline", b.headline, k.headline);
  b.body.forEach((p, j) => row(`Body ${j + 1}`, p, k.body[j]));
  row("Caption", b.caption, k.caption);
  L.push("");
});

L.push("## S Scene data", "", "| Item | EN | KO |", "| --- | --- | --- |");
row("Card date", en.mock.date, ko.mock.date);
row("Card title", en.mock.title, ko.mock.title);
row("Card stats (template)", en.mock.stats, ko.mock.stats);
row("S1/S5 section labels", `${en.mock.timeLabel} / ${en.mock.stuckLabel}`, `${ko.mock.timeLabel} / ${ko.mock.stuckLabel}`);
en.mock.tasks.forEach((t, i) => row(`Card task ${i + 1}`, t, ko.mock.tasks[i]));
row("S1/S5 stuck", `${en.mock.stuck} · ${en.mock.resolved}`, `${ko.mock.stuck} · ${ko.mock.resolved}`);
row("Timeline projects", en.mock.projects.join(", "), ko.mock.projects.join(", "));
en.scenes.windows.forEach((w, i) => row(`S2 window ${i + 1}`, `${w.app} · $ ${w.line}`, `${ko.scenes.windows[i].app} · $ ${ko.scenes.windows[i].line}`));
en.scenes.sessions.forEach((s, i) => {
  const o = ko.scenes.sessions[i];
  row(`S2 session ${i + 1} (${s.dot})`, `${s.title} / ${s.state} · ${s.project}`, `${o.title} / ${o.state} · ${o.project}`);
});
row("S8 suggestion card", `${en.scenes.suggest.title} · ${en.scenes.suggest.kind} · ${en.scenes.suggest.effect}`, `${ko.scenes.suggest.title} · ${ko.scenes.suggest.kind} · ${ko.scenes.suggest.effect}`);
row("S9 vault / note files", `${en.scenes.delivered.vault} / ${en.scenes.delivered.files.join(", ")}`, `${ko.scenes.delivered.vault} / ${ko.scenes.delivered.files.join(", ")}`);
row("S9 note title / headline", `${en.scenes.delivered.title} / ${en.scenes.delivered.headline}`, `${ko.scenes.delivered.title} / ${ko.scenes.delivered.headline}`);
row("S9 note section", en.scenes.delivered.section, ko.scenes.delivered.section);
en.scenes.delivered.rows.forEach((r, i) => row(`S9 note row ${i + 1}`, `${r.task} / ${r.time}`, `${ko.scenes.delivered.rows[i].task} / ${ko.scenes.delivered.rows[i].time}`));
en.scenes.carried.forEach((c, i) => row(`S5 open item ${i + 1}`, c, ko.scenes.carried[i]));
en.scenes.limits.forEach((l, i) => {
  const o = ko.scenes.limits[i];
  row(`S6 limit ${i + 1}`, `${l.name} / ${l.label} / ${l.reset}`, `${o.name} / ${o.label} / ${o.reset}`);
});
L.push("");

const doc = readFileSync(FILE, "utf8");
const start = doc.indexOf("## 1 Hero");
const end = doc.indexOf("## 8 Compare");
if (start < 0 || end < 0) throw new Error("appendix markers '## 1 Hero' / '## 8 Compare' not found");
writeFileSync(FILE, doc.slice(0, start) + L.join("\n") + "\n" + doc.slice(end));
console.log("appendix hero, story blocks and scene data regenerated");
