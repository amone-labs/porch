import { expect, test } from "@playwright/test";
import { PAGES } from "./pages";

for (const p of PAGES) {
  test(`${p.path} has lang, title and hreflang`, async ({ page }) => {
    await page.goto(p.path);
    await expect(page.locator("html")).toHaveAttribute("lang", p.lang);
    await expect(page).toHaveTitle(/Porch/);
    await expect(page.locator('link[rel="alternate"][hreflang="en"]')).toHaveCount(1);
    await expect(page.locator('link[rel="alternate"][hreflang="ko"]')).toHaveCount(1);
    await expect(page.locator('link[rel="alternate"][hreflang="x-default"]')).toHaveCount(1);
    const bg = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
    expect(bg).toBe("rgb(10, 10, 12)");
  });
}
