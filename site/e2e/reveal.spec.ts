import { expect, test } from "@playwright/test";
import { PAGES } from "./pages";

const opacity = (sel: string) => (page: import("@playwright/test").Page) =>
  page.locator(sel).first().evaluate((e) => getComputedStyle(e).opacity);

for (const p of PAGES) {
  test.describe(p.path, () => {
    test("sections below the fold rise in when scrolled to", async ({ page }) => {
      await page.setViewportSize({ width: 1440, height: 900 });
      await page.goto(p.path);
      const faq = "#faq .wrap > *";
      expect(await opacity(faq)(page)).toBe("0");
      await page.locator("#faq").scrollIntoViewIfNeeded();
      await expect.poll(() => opacity(faq)(page)).toBe("1");
    });

    test("with reduced motion nothing is hidden", async ({ browser }) => {
      const ctx = await browser.newContext({ reducedMotion: "reduce", viewport: { width: 1440, height: 900 } });
      const page = await ctx.newPage();
      await page.goto(`http://127.0.0.1:4329${p.path}`);
      expect(await opacity("#faq .wrap > *")(page)).toBe("1");
      expect(await opacity("#stuck .story-text")(page)).toBe("1");
      await ctx.close();
    });
  });
}
