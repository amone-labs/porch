import { expect, test } from "@playwright/test";
import { PAGES } from "./pages";

for (const p of PAGES) {
  test(`${p.path} footer links to the privacy policy in the same language`, async ({ page }) => {
    await page.goto(p.path);
    await page.locator("footer a[href$='privacy/']").click();
    await expect(page).toHaveURL(new RegExp(`${p.path}privacy/$`));
    await expect(page.locator("html")).toHaveAttribute("lang", p.lang);
    await expect(page.locator("h1")).toHaveText(p.lang === "ko" ? "개인정보처리방침" : "Privacy Policy");
    await expect(page.locator("main")).toContainText("dev.bearjb@gmail.com");
  });
}

test("the policy fits a phone without sideways scroll", async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 800 });
  await page.goto("/ko/privacy/");
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(375);
});
