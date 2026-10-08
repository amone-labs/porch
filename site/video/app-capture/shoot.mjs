// Shoots the app screens for the hero video, in English and Korean, and records where the focus boxes go.
// Needs the capture server: `pnpm capture` (port 1431, or CAPTURE_PORT for both). Usage: pnpm shoot, then pnpm hero.
// Writes <dir>/<screen>.png per language, video/hero/focus-<lang>.json (boxes in CSS px of the 1180×760 window)
// and video/hero/regions-<lang>.json (areas video/build-works.mjs cuts out of the screenshots).
import { mkdirSync, writeFileSync } from "node:fs";
import { chromium } from "@playwright/test";

const PORT = process.env.CAPTURE_PORT ?? 1431;

/** Per language: where the screenshots go, navigation labels, and the text that anchors each focus box. */
const LANGS = {
  en: {
    out: "video/hero/assets-en/",
    query: "&lang=en",
    nav: { status: "Sessions", usage: "Usage", projects: "Projects", stuck: "Stuck tasks", limits: "Latest recorded account limits" },
    anchors: {
      time: ["Email verification and lockout", "1h 14m"],
      stuck: ["Order alerts sent twice", "Next time"],
      now: ["Needs permission", "Permission request"],
      limits: ["Claude Max", "Codex"],
      chart: ["Usage by hour", "One bar is one hour"],
      card: ["A hook that starts local Redis", "Dismiss"],
      effect: ["Test for orderId before touching retries", "14 days after"],
    },
  },
  ko: {
    out: "video/hero-ko/assets/",
    query: "",
    nav: { status: "작업 현황", usage: "사용량", projects: "프로젝트", stuck: "막힌 작업", limits: "현재 계정 한도" },
    anchors: {
      time: ["이메일 인증과 실패 잠금", "1시간 14분"],
      stuck: ["주문 알림이 두 번 감", "예방법"],
      now: ["허락 필요", "허락 요청"],
      limits: ["Claude Max", "Codex"],
      chart: ["시간별 사용량", "막대 하나가 한 시간"],
      card: ["로컬 Redis를 테스트 전에 띄우는 훅", "안 함"],
      effect: ["재시도 전에 orderId를 확인하는 테스트부터", "적용 후 14일:"],
    },
  },
};

const USAGE_H = 1080;
const browser = await chromium.launch();
const view = (w, h) => browser.newContext({ viewport: { width: w, height: h }, colorScheme: "dark", deviceScaleFactor: 2, timezoneId: "Asia/Seoul" });

for (const [lang, L] of Object.entries(LANGS)) {
  mkdirSync(L.out, { recursive: true });
  const page = await (await view(1180, 760)).newPage();
  const focus = {};
  const regions = {};

  /** Smallest element containing `a`, widened to the nearest ancestor that also contains `b`. */
  const box = (a, b) =>
    page.evaluate(
      ([a, b]) => {
        const all = [...document.querySelectorAll("body *")].filter((e) => e.textContent?.includes(a));
        let el = all.sort((x, y) => x.textContent.length - y.textContent.length)[0];
        if (!el) return null;
        if (b) while (el.parentElement && !el.textContent.includes(b)) el = el.parentElement;
        const r = el.getBoundingClientRect();
        const pad = 8;
        return { x: Math.round(r.left - pad), y: Math.round(r.top - pad), w: Math.round(r.width + pad * 2), h: Math.round(r.height + pad * 2) };
      },
      [a, b],
    );
  const shoot = async (name, keys) => {
    await page.screenshot({ path: `${L.out}${name}.png` });
    focus[name] = [];
    for (const key of keys) {
      const r = await box(...L.anchors[key]);
      if (!r) throw new Error(`${lang} ${name}: no element with "${L.anchors[key][0]}"`);
      focus[name].push(r);
    }
  };
  const go = async (label) => {
    await page.getByText(label, { exact: true }).first().click();
    await page.waitForTimeout(1200);
  };

  await page.goto(`http://localhost:${PORT}/?window=main${L.query}`, { waitUntil: "networkidle" });
  await page.waitForTimeout(1500);
  await shoot("app-summary-top", ["time"]);

  await page.mouse.move(700, 400);
  await page.evaluate((h) => {
    [...document.querySelectorAll("*")].find((e) => e.textContent === h && e.children.length === 0)?.scrollIntoView({ block: "start" });
  }, L.nav.stuck);
  await page.mouse.wheel(0, -90);
  await page.waitForTimeout(800);
  await shoot("app-summary-stuck", ["stuck"]);

  await go(L.nav.status);
  await shoot("app-now", ["now"]);
  // the whole session list, for the "works" clip (video/build-works.mjs)
  {
    const all = await box("shop-web", "porch-site");
    const head = await box("shop-web");
    regions["app-now-list"] = { x: all.x, y: head.y, w: all.w, h: all.y + all.h - head.y };
    // each project's group, its header down to the next header, for the "works" clip (video/build-works.mjs)
    const names = ["shop-web", "shop-api", "porch-site"];
    const tops = await page.evaluate(
      (names) =>
        names.map((n) => {
          const el = [...document.querySelectorAll("body *")].find((e) => e.children.length === 0 && e.textContent === n);
          return el ? Math.round(el.getBoundingClientRect().top) : null;
        }),
      names,
    );
    if (tops.includes(null)) throw new Error(`${lang} app-now: a project header is missing (${tops})`);
    const list = regions["app-now-list"];
    names.forEach((n, i) => {
      const y = i === 0 ? list.y : tops[i] - (tops[0] - list.y);
      const end = i < names.length - 1 ? tops[i + 1] - (tops[0] - list.y) : list.y + list.h;
      regions[`app-now-${n}`] = { x: list.x, y, w: list.w, h: end - y };
    });
  }

  await go(L.nav.usage);
  // The usage screen runs long (hourly chart, breakdown, limits); a taller window shows all of it unscrolled.
  await page.setViewportSize({ width: 1180, height: USAGE_H });
  await page.waitForTimeout(800);
  await shoot("app-usage", ["chart", "limits"]);
  await page.setViewportSize({ width: 1180, height: 760 });

  await go(L.nav.projects);
  await page.getByText("shop-api", { exact: true }).first().click();
  await page.waitForTimeout(1200);
  await page.mouse.move(700, 400);
  await page.mouse.wheel(0, 250);
  await page.waitForTimeout(800);
  // box 1: the whole proposed hook card (title, code, Applied/Dismiss); box 2: the measured effect of one applied earlier
  await shoot("app-project-api-applied", ["card", "effect"]);


  writeFileSync(`video/hero/focus-${lang}.json`, JSON.stringify(focus, null, 2) + "\n");
  writeFileSync("video/hero/sizes.json", JSON.stringify({ default: [1180, 760], "app-usage": [1180, USAGE_H] }, null, 2) + "\n");
  writeFileSync(`video/hero/regions-${lang}.json`, JSON.stringify(regions, null, 2) + "\n");
  console.log(lang, Object.keys(focus).length, "screens");
}
await browser.close();
