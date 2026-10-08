import { expect, test } from "@playwright/test";
import { PAGES } from "./pages";

for (const p of PAGES) {
  test.describe(p.path, () => {
    test("landing type scale at 1440px", async ({ page }) => {
      await page.setViewportSize({ width: 1440, height: 900 });
      await page.goto(p.path);
      await expect(page.locator("h1")).toHaveCSS("font-size", "56px");
      await expect(page.locator(".lead").first()).toHaveCSS("font-size", "20px");
      await expect(page.locator(".lead").first()).toHaveCSS("font-weight", "300");
      const cta = page.locator("#top .btn-primary");
      expect((await cta.boundingBox())!.height).toBeGreaterThanOrEqual(60);
    });

    test("hero CTA is visible without scrolling at 1440x900", async ({ page }) => {
      await page.setViewportSize({ width: 1440, height: 900 });
      await page.goto(p.path);
      const box = (await page.locator("#top .btn-primary").boundingBox())!;
      expect(box.y + box.height).toBeLessThanOrEqual(900 - 126);
    });

    test("h1 is 56px when the story is in flow", async ({ page }) => {
      await page.setViewportSize({ width: 1440, height: 700 });
      await page.goto(p.path);
      await expect(page.locator("h1")).toHaveCSS("font-size", "56px");
    });

    test("serif never sets Hangul", async ({ page }) => {
      await page.goto(p.path);
      const bad = await page.evaluate(() => {
        const out: string[] = [];
        const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
        for (let n = walker.nextNode(); n; n = walker.nextNode()) {
          const t = n.textContent ?? "";
          if (/[가-힣]/.test(t) && getComputedStyle(n.parentElement!).fontFamily.includes("IBM Plex Serif")) out.push(t.trim());
        }
        return out;
      });
      expect(bad).toEqual([]);
    });

    test("the platform note is plain small text, not the reference's serif aside", async ({ page }) => {
      await page.goto(p.path);
      await expect(page.locator("#top .cta .serif, #top .platform.serif")).toHaveCount(0);
      const note = page.locator("#top .platform");
      await expect(note).toContainText("macOS · Apple Silicon");
      await expect(page.locator("#top .condition")).toHaveCount(0); // the condition rides on the platform line
      await expect(note).not.toHaveCSS("font-family", /IBM Plex Serif/);
      await expect(note).toHaveCSS("font-size", "13px");
    });

    test("sections breathe: hero to story, and story blocks well apart", async ({ page }) => {
      await page.setViewportSize({ width: 1440, height: 900 });
      await page.goto(p.path);
      const video = (await page.locator("#top .hero-video").boundingBox())!;
      const label = (await page.locator("#works .eyebrow, #works .eyebrow-ko").first().boundingBox())!;
      expect(label.y - (video.y + video.height)).toBeGreaterThanOrEqual(320);
      // between one block's content and the next block's content
      const gaps = await page.locator(".story-block").evaluateAll((els) => {
        const span = (e: Element) => {
          const r = [...e.querySelectorAll(".story-text, .scene-slot")].map((c) => c.getBoundingClientRect());
          return { top: Math.min(...r.map((x) => x.top)), bottom: Math.max(...r.map((x) => x.bottom)) };
        };
        return els.slice(1).map((e, i) => span(e).top - span(els[i]).bottom);
      });
      for (const g of gaps) expect(g).toBeGreaterThanOrEqual(200);
    });

    test("inside each section the heading stands apart from its content", async ({ page }) => {
      await page.setViewportSize({ width: 1440, height: 900 });
      // measure the settled layout, not the scroll-reveal offset of blocks not yet scrolled to
      await page.emulateMedia({ reducedMotion: "reduce" });
      await page.goto(p.path);
      const gaps = await page.locator("main section.block > .wrap > h2.h2, main section.block > .wrap > .stack > h2.h2, main section.block h2.h2").evaluateAll((hs) =>
        hs.map((h) => {
          const next = h.nextElementSibling;
          return { id: h.closest("section")!.id, gap: next ? next.getBoundingClientRect().top - h.getBoundingClientRect().bottom : 999 };
        }),
      );
      for (const g of gaps) expect(g.gap, g.id).toBeGreaterThanOrEqual(48);
      const h1 = (await page.locator("#top h1").boundingBox())!;
      const lead = (await page.locator("#top .lead").boundingBox())!;
      expect(lead.y - (h1.y + h1.height)).toBeGreaterThanOrEqual(24);
      const cta = (await page.locator("#top .cta").boundingBox())!;
      expect(cta.y - (lead.y + lead.height)).toBeGreaterThanOrEqual(32);
      // the video's own title must not read as the next line of the hero
      const platform = (await page.locator("#top .platform").boundingBox())!;
      const video = (await page.locator("#top .hero-video .screen").boundingBox())!;
      expect(video.y - (platform.y + platform.height)).toBeGreaterThanOrEqual(112);
      await expect(page.locator("#top .hero-video .screen")).not.toHaveCSS("border-top-width", "0px");
    });

    test("in flow layout, story blocks are well apart", async ({ page }) => {
      await page.setViewportSize({ width: 1024, height: 900 });
      await page.goto(p.path);
      const gaps = await page.locator(".story-block").evaluateAll((els) =>
        els.slice(1).map((e, i) => e.querySelector("h2")!.getBoundingClientRect().top - els[i].getBoundingClientRect().bottom),
      );
      for (const g of gaps) expect(g).toBeGreaterThanOrEqual(120);
    });

    test("nav stays at the top while scrolling and its pill is not primary", async ({ page }) => {
      await page.setViewportSize({ width: 1440, height: 900 });
      await page.goto(p.path);
      await page.evaluate(() => window.scrollTo(0, 2000));
      const box = await page.locator('nav[aria-label="Main"]').boundingBox();
      expect(box!.y).toBe(0);
      await expect(page.locator('nav[aria-label="Main"] .btn-primary')).toHaveCount(0);
      await expect(page.locator('nav[aria-label="Main"] .pill')).toHaveAttribute("href", /Porch\.dmg$/);
    });

    test("bottom fold fade is fixed and ignores pointer", async ({ page }) => {
      await page.goto(p.path);
      const fold = page.locator(".fold");
      await expect(fold).toHaveCSS("position", "fixed");
      await expect(fold).toHaveCSS("pointer-events", "none");
      await expect(fold).toHaveCSS("height", "126px");
    });

    test("html gets the js class", async ({ page }) => {
      await page.goto(p.path);
      await expect(page.locator("html")).toHaveClass(/\bjs\b/);
    });
  });
}
