import { expect, test } from "@playwright/test";
import { PAGES } from "./pages";

for (const p of PAGES) {
  test.describe(p.path, () => {
    test("nothing focusable hides inside aria-hidden scenes", async ({ page }) => {
      await page.goto(p.path);
      const n = await page.evaluate(
        () => document.querySelectorAll('[aria-hidden="true"] :is(a[href], button, input, [tabindex]:not([tabindex="-1"]))').length,
      );
      expect(n).toBe(0);
    });

    test("scene motion replays in flow layout when the scene scrolls in", async ({ page }) => {
      await page.setViewportSize({ width: 1024, height: 900 });
      await page.goto(p.path);
      await page.waitForTimeout(3000);
      await page.evaluate(() => document.querySelector('#usage .scene-slot .scene-full')!.scrollIntoView({ block: "center" }));
      await page.waitForTimeout(300);
      const t = await page.locator("#usage .scene-slot .scene-full .focus").first().evaluate((e) => Number(e.getAnimations()[0]?.currentTime ?? 1e9));
      expect(t).toBeLessThan(600);
    });

    test("privacy has three columns and compare carries its date stamp", async ({ page }) => {
      await page.goto(p.path);
      await expect(page.locator("#privacy .col")).toHaveCount(3);
      await expect(page.locator("#compare")).toContainText(p.lang === "ko" ? "2026년 10월 1일 기준" : "As of Oct 1, 2026");
    });
  });
}
