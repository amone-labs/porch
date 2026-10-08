import { expect, test } from "@playwright/test";
import { PAGES } from "./pages";

// The site is dark only, like the app; a visitor whose Mac is in light mode still gets the dark canvas.
test.use({ colorScheme: "light" });

for (const p of PAGES) {
  test(`${p.path} stays dark in light mode and keeps the primary readable`, async ({ page }) => {
    await page.goto(p.path);
    expect(await page.evaluate(() => getComputedStyle(document.body).backgroundColor)).toBe("rgb(10, 10, 12)");
    const [bg, fg] = await page.locator("#top .btn-primary").evaluate((e) => {
      const s = getComputedStyle(e);
      return [s.backgroundColor, s.color];
    });
    expect(bg).not.toBe(fg);
    expect(bg).not.toBe("rgb(10, 10, 12)");
  });
}
