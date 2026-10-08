import { expect, test } from "@playwright/test";
import { PAGES } from "./pages";

const BLOCKS = ["works", "time", "stuck", "delivered", "suggest", "usage"];

for (const p of PAGES) {
  test.describe(p.path, () => {
    test("no sticky stage: every block carries its own scene", async ({ page }) => {
      await page.setViewportSize({ width: 1440, height: 900 });
      await page.goto(p.path);
      await expect(page.locator(".story-stage")).toHaveCount(0);
      for (const id of BLOCKS) await expect(page.locator(`#${id} .scene-full`), id).toBeVisible();
    });

    test("at desktop width text and screen sit side by side, alternating sides", async ({ page }) => {
      await page.setViewportSize({ width: 1440, height: 900 });
      await page.goto(p.path);
      for (const [i, id] of BLOCKS.entries()) {
        const text = (await page.locator(`#${id} h2`).boundingBox())!;
        const scene = (await page.locator(`#${id} .scene-full`).boundingBox())!;
        // same row: the scene overlaps the heading vertically
        expect(scene.y, id).toBeLessThan(text.y + text.height);
        expect(scene.width, id).toBeGreaterThanOrEqual(640);
        if (i % 2 === 0) expect(scene.x, id).toBeGreaterThan(text.x);
        else expect(scene.x + scene.width, id).toBeLessThanOrEqual(text.x + 1);
      }
    });

    for (const [w, h] of [[769, 900], [1024, 900], [375, 800]]) {
      test(`stacks without horizontal scroll at ${w}px`, async ({ page }) => {
        await page.setViewportSize({ width: w, height: h });
        await page.goto(p.path);
        for (const id of BLOCKS) await expect(page.locator(`#${id} .scene-slot .scene:visible`).first(), id).toBeVisible();
        expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(w);
      });
    }

    test("anchor jump lands on the block", async ({ page }) => {
      await page.setViewportSize({ width: 1440, height: 900 });
      await page.goto(`${p.path}#usage`);
      await expect(page.locator("#usage h2")).toBeInViewport();
    });

    test("the nav's how-it-works link lands at the start of the story", async ({ page }) => {
      await page.setViewportSize({ width: 1440, height: 900 });
      await page.goto(`${p.path}#how`);
      await expect(page.locator("#works h2")).toBeInViewport();
    });
  });
}

test.describe("no JS", () => {
  test.use({ javaScriptEnabled: false });
  test("scenes show without JS", async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto("/");
    for (const id of BLOCKS) await expect(page.locator(`#${id} .scene-full`), id).toBeVisible();
  });
});
