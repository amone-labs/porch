import { expect, test } from "@playwright/test";
import { PAGES } from "./pages";

const WIDTHS = [375, 1280];

for (const p of PAGES) {
  for (const width of WIDTHS) {
    test(`${p.path} at ${width}px: no sideways scroll, at most one primary in view`, async ({ page }) => {
      await page.setViewportSize({ width, height: 800 });
      await page.goto(p.path);
      expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
      const height = await page.evaluate(() => document.documentElement.scrollHeight);
      for (let y = 0; y < height; y += 200) {
        await page.evaluate((top) => window.scrollTo(0, top), y);
        const inView = await page.locator(".btn-primary").evaluateAll((els) =>
          els.filter((e) => {
            const r = e.getBoundingClientRect();
            return r.bottom > 0 && r.top < window.innerHeight;
          }).length,
        );
        expect(inView, `scrollY=${y}`).toBeLessThanOrEqual(1);
      }
      await page.evaluate(() => window.scrollTo(0, 0));
      await page.screenshot({ path: `test-results/shots/${p.lang}-${width}.png`, fullPage: true });
    });
  }

  test(`${p.path} has exactly two primaries, in hero and close`, async ({ page }) => {
    await page.goto(p.path);
    await expect(page.locator(".btn-primary")).toHaveCount(2);
    await expect(page.locator("#top .btn-primary, #close .btn-primary")).toHaveCount(2);
  });

  test(`${p.path} turns transitions off under reduced motion`, async ({ browser }) => {
    const context = await browser.newContext({ reducedMotion: "reduce", colorScheme: "dark" });
    const page = await context.newPage();
    await page.goto(`http://127.0.0.1:4329${p.path}`);
    const d = await page.locator('nav[aria-label="Main"] .pill').evaluate((e) => getComputedStyle(e).transitionDuration);
    expect(d.split(",").every((x) => parseFloat(x) === 0)).toBe(true);
    await context.close();
  });

  test(`${p.path} anchors resolve`, async ({ page }) => {
    await page.goto(p.path);
    const hrefs = await page.locator('a[href^="#"]').evaluateAll((els) => els.map((e) => e.getAttribute("href")!));
    for (const h of new Set(hrefs)) await expect(page.locator(h), h).toHaveCount(1);
  });
}
