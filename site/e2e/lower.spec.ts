import { expect, test } from "@playwright/test";
import { PAGES } from "./pages";

for (const p of PAGES) {
  test.describe(p.path, () => {
    test("compare is an O/X table with Porch first and every other tool linked to its source", async ({ page }) => {
      await page.goto(p.path);
      const head = await page.locator("#compare thead th").allInnerTexts();
      expect(head.slice(1)).toEqual(["Porch", "Chit", "ClaudeUsageBar", "claudebill", "Orca", "Langfuse"]);
      await expect(page.locator("#compare tbody tr")).toHaveCount(9);
      const ours = await page.locator("#compare tbody td.ours").allInnerTexts();
      expect(ours).toEqual(["O", "O", "O", "O", "O", "O", "O", "O", "X"]);
      const bg = await page.locator("#compare tbody td.ours").first().evaluate((e) => getComputedStyle(e).backgroundColor);
      expect(bg).not.toBe("rgba(0, 0, 0, 0)");
      for (const cell of await page.locator("#compare tbody td.mark").allInnerTexts()) expect(["O", "X", "△", "–"]).toContain(cell);
      await expect(page.locator("#compare .sources a[href^='https://']")).toHaveCount(5);
      await expect(page.locator("#compare")).not.toContainText("✓");
    });

    test("compare table fits without scrolling on a desktop", async ({ page }) => {
      await page.setViewportSize({ width: 1440, height: 900 });
      await page.goto(p.path);
      const over = await page.locator("#compare .table-wrap").evaluate((e) => e.scrollWidth - e.clientWidth);
      expect(over).toBeLessThanOrEqual(0);
    });

    test("compare table scrolls inside its box on a phone, never the page", async ({ page }) => {
      await page.setViewportSize({ width: 375, height: 800 });
      await page.goto(p.path);
      expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(375);
    });

    test("FAQ rows have a chevron that turns when open", async ({ page }) => {
      await page.goto(p.path);
      const first = page.locator("#faq details").first();
      const chev = first.locator(".chev");
      await expect(chev).toBeVisible();
      await first.locator("summary").click();
      await expect(chev).toHaveCSS("transform", /matrix\(-1/);
    });

    test("close is the sample day's timeline, then the summary and one download", async ({ page }) => {
      await page.goto(p.path);
      // one lane per demo project, no cards
      await expect(page.locator("#close .lane")).toHaveCount(3);
      await expect(page.locator("#close .lane .proj")).toHaveText(["shop-web", "shop-api", "porch-site"]);
      await expect(page.locator("#close .card")).toHaveCount(0);
      await expect(page.locator("#close .btn-primary")).toHaveCount(1);
      // same Apple mark as the hero button
      await expect(page.locator("#close .btn-primary svg")).toHaveCount(1);
      await expect(page.locator("#close .meta")).toContainText("macOS · Apple Silicon");
    });

    test("close's bars draw in when it scrolls into view, and stand still without motion", async ({ browser }) => {
      for (const reducedMotion of ["no-preference", "reduce"] as const) {
        const ctx = await browser.newContext({ reducedMotion, viewport: { width: 1440, height: 900 } });
        const page = await ctx.newPage();
        await page.goto(`http://127.0.0.1:4329${p.path}`);
        const bar = page.locator("#close .bar").first();
        await page.locator("#close").scrollIntoViewIfNeeded();
        await expect.poll(() => bar.evaluate((e) => getComputedStyle(e).transform), { message: reducedMotion }).toMatch(/^(none|matrix\(1, 0, 0, 1, 0, 0\))$/);
        await ctx.close();
      }
    });
  });
}
