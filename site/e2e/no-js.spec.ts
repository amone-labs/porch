import { expect, test } from "@playwright/test";
import { PAGES } from "./pages";

test.use({ javaScriptEnabled: false });

for (const p of PAGES) {
  test(`${p.path} reads fully without JavaScript`, async ({ page }) => {
    await page.goto(p.path);
    await expect(page.locator("h1")).toBeVisible();
    await expect(page.locator("#agent-prompt")).toBeVisible();
    for (const id of ["top", "how", "works", "time", "stuck", "usage", "compare", "privacy", "setup", "faq", "agents", "close"]) {
      await expect(page.locator(`#${id}`), id).toBeVisible();
    }
    const first = page.locator("#faq details").first();
    await first.locator("summary").click();
    await expect(first).toHaveAttribute("open", "");
  });
}
