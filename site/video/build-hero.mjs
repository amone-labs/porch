// Writes the hero video compositions, one per language, from one scene list.
//   video/hero/index.html     English (app text through the capture overlay, assets-en/)
//   video/hero-ko/index.html  Korean  (the app as it ships, assets/)
// Run `pnpm shoot` first: it writes the screenshots and focus-<lang>.json this reads.
// Usage: pnpm hero
import { cpSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { CSS as WORKS_CSS, scene as worksScene } from "./build-works.mjs";

const SCENE = 4.6; // a scene's length unless it sets `dur`
const SHOT_W = 1180; // capture window width
const SHOW_W = 1560; // width the screen is shown at in the video
const k = SHOW_W / SHOT_W;

/** Scene order: summary first, matching the hero headline. `focus` lists boxes from focus-<lang>.json in order.
 * `side`: width of the left edge that stays put while the screen drifts (the app's sidebar, 208 of 1180 px).
 * `works`: the "works" clip's scene (build-works.mjs) instead of a screenshot, its 1180 px stage centred. */
const SCENES = [
  { shot: "app-summary-top", drift: 40 },
  { shot: "app-summary-stuck", drift: 40 },
  // Obsidian's ribbon and file list are wider than the app's sidebar; keep them still while the note scrolls
  { shot: "obsidian-note", drift: 480, side: 344 },
  { shot: "app-project-api-applied", drift: 70 },
  // terminals start sessions that land in the real list; then the waiting one is boxed and the menu bar counts it
  // drawn at 1:1 and held still, so the menu bar (where the count lands) and the last list group both stay in view
  { works: true, dur: 5.8, drift: 0 },
  { shot: "app-usage", drift: 720 }, // captured taller: scroll from the hourly chart down to the limits
];

const COPY = {
  en: [
    ["Daily summary", "Your day, summarized.", "The agent you choose in Settings, Claude Code or Codex, summarizes your records. Porch shows estimated time by task."],
    ["Stuck tasks", "Where you got stuck, with evidence.", "The summary records repeated errors or requests and suggests one change. If the records do not show the cause, it says so."],
    ["Notes", "Summaries saved as Obsidian notes.", "Connect an Obsidian vault or a folder to save daily, weekly and monthly summaries as Markdown notes. Regenerating a summary overwrites its note."],
    ["Suggestions", "Suggestions for problems that keep coming back.", "The agent you choose in Settings suggests a CLAUDE.md line or a hook. You apply it; Porch compares how often the problem came up before and after, without calling it the cause."],
    ["Sessions", "Sessions from any terminal, in one list.", "Sessions waiting for permission or an answer, and sessions stopped on an error, come first; the menu bar shows their count. Only Claude Code shows Stopped on error."],
    ["Usage", "Tokens and limits in one place.", "API-equivalent cost (not your bill) and cache hits. Claude limits need Pro or Max and a setting; Codex limits are the last recorded values."],
  ],
  ko: [
    ["요약", "오늘 한 일을 AI가 정리합니다.", "설정에서 고른 클로드코드나 코덱스가 기록을 요약합니다. 작업별 추정 시간도 함께 보여 줍니다."],
    ["막힌 작업", "같은 오류나 요청이 반복된 작업을 찾아 줍니다.", "오류나 요청이 반복된 횟수와 줄일 방법 하나를 적습니다. 기록으로 원인을 알 수 없으면 알 수 없다고 씁니다."],
    ["노트", "요약이 옵시디언 노트로 쌓입니다.", "옵시디언 볼트나 폴더를 연결하면 일·주·월 요약을 마크다운 노트로 저장합니다. 요약을 다시 만들면 그 노트를 덮어씁니다."],
    ["제안", "반복되는 문제마다 바꿔 볼 점을 제안합니다.", "설정에서 고른 클로드코드나 코덱스가 CLAUDE.md 문구나 훅을 제안합니다. 직접 적용하면 같은 문제가 생긴 횟수를 전후로 비교하되, 원인으로 단정하지 않습니다."],
    ["작업 현황", "어느 터미널의 세션이든 한 목록에 모입니다.", "허락이나 답을 기다리는 세션과 오류로 멈춘 세션이 위에 오고, 메뉴바에 그 수가 뜹니다. 오류로 멈춤은 클로드코드만 표시합니다."],
    ["사용량", "토큰 사용량과 남은 한도를 한곳에서 봅니다.", "실제 결제액과 다른 API 환산 비용과 캐시 적중을 봅니다. 클로드 한도는 Pro·Max에서 설정을 켜야 나오고, 코덱스 한도는 기록에 남은 마지막 값입니다."],
  ],
};

const OUT = { en: { dir: "video/hero", assets: "assets-en" }, ko: { dir: "video/hero-ko", assets: "assets" } };

const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;");

function build(lang) {
  const { dir, assets } = OUT[lang];
  const focus = JSON.parse(readFileSync(`video/hero/focus-${lang}.json`, "utf8"));
  const durs = SCENES.map((s) => s.dur ?? SCENE);
  const starts = durs.map((_, i) => +durs.slice(0, i).reduce((a, d) => a + d, 0).toFixed(2));
  const total = +durs.reduce((a, d) => a + d, 0).toFixed(2);
  // the stage is centred in the window; its menu bar runs the window's full width, like a screen's
  const STAGE_LEFT = (SHOW_W - 1180) / 2;
  const works = SCENES.map((s, i) =>
    s.works ? worksScene(lang, assets, { p: `w${i + 1}-`, at: starts[i] + 0.45, step: 1.0, bar: { left: -STAGE_LEFT, width: SHOW_W } }) : null,
  );

  const screens = SCENES.map((s, i) => {
    if (s.works) {
      // no window controls: the window shows a desktop here, not one app
      return `          <div class="screen" id="s${i + 1}"><div class="wk-frame drift"><div class="wk" style="left:${STAGE_LEFT}px">\n${works[i].body}\n          </div></div></div>`;
    }
    const boxes = focus[s.shot].map(
      (b, j) =>
        `<div class="focus drift" id="f${i + 1}-${j}" style="left:${Math.round(b.x * k)}px;top:${Math.round(b.y * k)}px;width:${Math.round(b.w * k)}px;height:${Math.round(b.h * k)}px"></div>`,
    );
    const side = s.side ? ` style="clip-path:inset(0 ${(100 - (s.side / SHOT_W) * 100).toFixed(2)}% 0 0)"` : "";
    return `          <div class="screen" id="s${i + 1}"><img class="main drift" src="${assets}/${s.shot}.png" alt="">${boxes.join("")}<img class="side" src="${assets}/${s.shot}.png" alt=""${side}><div class="titlebar"><i></i><i></i><i></i></div></div>`;
  }).join("\n");

  const copy = COPY[lang]
    .map(
      ([label, title, sub], i) => `        <div class="copy" id="c${i + 1}">
          <div class="label">${esc(label)}</div>
          <div class="title">${esc(title)}</div>
          <div class="sub">${esc(sub)}</div>
        </div>`,
    )
    .join("\n");

  const timeline = SCENES.map((s, i) => {
    const n = i + 1;
    const at = starts[i];
    const out = +(at + durs[i]).toFixed(2);
    const lines = [`      enter(${n}, ${at}, ${s.drift}, ${durs[i]});`, `      exit(${n}, ${+(out - 0.4).toFixed(2)});`];
    if (s.works) lines.push(...works[i].lines);
    else {
      const count = focus[s.shot].length;
      if (count === 1) lines.push(`      focusIn("#f${n}-0", ${+(at + 1.1).toFixed(2)}); focusOut("#f${n}-0", ${+(out - 0.5).toFixed(2)});`);
      else
        lines.push(
          `      focusIn("#f${n}-0", ${+(at + 0.9).toFixed(2)}); focusOut("#f${n}-0", ${+(at + 2.5).toFixed(2)});`,
          `      focusIn("#f${n}-1", ${+(at + 2.6).toFixed(2)}); focusOut("#f${n}-1", ${+(out - 0.5).toFixed(2)});`,
        );
    }
    return lines.join("\n");
  }).join("\n\n");

  const html = readFileSync("video/hero.template.html", "utf8")
    .replaceAll("{{lang}}", lang)
    .replaceAll("{{total}}", String(total))
    .replaceAll("{{assets}}", assets)
    .replace("{{screens}}", screens)
    .replace("{{worksCss}}", WORKS_CSS)
    .replace("{{copy}}", copy)
    .replace("{{scene}}", String(SCENE))
    .replace("{{timeline}}", timeline);

  if (!existsSync(dir)) {
    mkdirSync(dir, { recursive: true });
    for (const f of ["hyperframes.json", "package.json", "CLAUDE.md", "AGENTS.md"]) cpSync(`video/hero/${f}`, `${dir}/${f}`);
    writeFileSync(`${dir}/meta.json`, JSON.stringify({ id: "hero-ko", name: "hero-ko" }, null, 2) + "\n");
  }
  writeFileSync(`${dir}/index.html`, html);
  console.log(`${dir}/index.html: ${SCENES.length} scenes, ${total}s`);
}

build("en");
build("ko");
