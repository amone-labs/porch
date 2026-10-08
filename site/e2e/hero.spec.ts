import { expect, test } from "@playwright/test";
import { PAGES } from "./pages";

const HEADLINE = {
  en: "Your work with Claude Code and Codex, summarized by day, week and month.",
  ko: "클로드코드와 코덱스로 한 일을 하루, 한 주, 한 달 단위로 정리합니다.",
};

for (const p of PAGES) {
  test.describe(p.path, () => {
    test("hero says what porch does and links the download", async ({ page }) => {
      await page.setViewportSize({ width: 1024, height: 900 });
      await page.goto(p.path);
      const h1 = page.locator("h1");
      await expect(h1).toHaveCount(1);
      expect((await h1.innerText()).replace(/\s+/g, " ")).toBe(HEADLINE[p.lang]);
      await expect(page.locator("#top .btn-primary")).toHaveAttribute("href", /Porch\.dmg$/);
      await expect(page.locator("nav .btn-primary")).toHaveCount(0);
      await expect(page.locator('nav[aria-label="Main"] .pill')).toHaveAttribute("href", /Porch\.dmg$/);
      await expect(page.locator('#top a[href="#agent-install"]')).toHaveCount(1);
    });

    test("hero video is this locale's, with poster and caption", async ({ page }) => {
      await page.goto(p.path);
      const video = page.locator("#top .hero-video video");
      await expect(video).toHaveAttribute("poster", `/video/hero-${p.lang}.jpg`);
      await expect(video).toHaveAttribute("preload", "none");
      expect(await video.evaluate((v: HTMLVideoElement) => v.muted && v.loop && v.playsInline)).toBe(true);
      const srcs = await page.locator("#top .hero-video source").evaluateAll((s) => s.map((e) => e.getAttribute("src")));
      expect(srcs).toEqual([`/video/hero-${p.lang}.webm`, `/video/hero-${p.lang}.mp4`]);
      const cap = page.locator("#top .hero-video figcaption");
      await expect(cap).toContainText(p.lang === "ko" ? "실제 앱 화면" : "Real app screens");
      // the app follows the system language now (ADR 0011), so no page says the app ships in Korean
      await expect(cap).not.toContainText("the app ships in Korean");
    });

    test("no block carries the old English-UI note", async ({ page }) => {
      await page.setViewportSize({ width: 1024, height: 900 });
      await page.goto(p.path);
      // the stills show the app in the page's own language (ADR 0011), so the note under them is gone
      await expect(page.getByText("the app ships in Korean")).toHaveCount(0);
      await expect(page.locator(".ui-note")).toHaveCount(0);
    });

    test("story starts at works and includes suggestions before usage", async ({ page }) => {
      await page.goto(p.path);
      const ids = await page.locator(".story-block").evaluateAll((els) => els.map((e) => e.id));
      expect(ids).toEqual(["works", "time", "stuck", "delivered", "suggest", "usage"]);
    });

    test("language switch points at the other locale", async ({ page }) => {
      await page.setViewportSize({ width: 1024, height: 900 });
      await page.goto(p.path);
      await expect(page.locator('nav[aria-label="Main"] a[hreflang]')).toHaveAttribute("href", p.lang === "en" ? "/ko/" : "/");
    });
  });
}
