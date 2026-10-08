import { expect, test } from "@playwright/test";
import { PAGES } from "./pages";

for (const p of PAGES) {
  test.describe(p.path, () => {
    test("copy button puts the prompt on the clipboard", async ({ page, context }) => {
      await context.grantPermissions(["clipboard-read", "clipboard-write"]);
      await page.goto(p.path);
      const prompt = (await page.locator("#agent-prompt").innerText()).trim();
      await page.locator("#agent-install button").click();
      expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(prompt);
      await expect(page.locator("#agent-install button")).toHaveText(p.lang === "ko" ? "복사됨" : "Copied");
    });

    test("falls back to selecting the prompt when the clipboard is refused", async ({ page }) => {
      await page.addInitScript(() => {
        Object.defineProperty(navigator, "clipboard", { value: { writeText: () => Promise.reject(new Error("denied")) } });
      });
      await page.goto(p.path);
      await page.locator("#agent-install button").click();
      const selected = await page.evaluate(() => window.getSelection()?.toString().trim());
      expect(selected).toBe((await page.locator("#agent-prompt").innerText()).trim());
    });

    test("prompt wraps at 375px", async ({ page }) => {
      await page.setViewportSize({ width: 375, height: 800 });
      await page.goto(p.path);
      const box = await page.locator("#agent-install pre").boundingBox();
      expect(box!.x + box!.width).toBeLessThanOrEqual(375);
    });

    test("FAQ opens without JavaScript help", async ({ page }) => {
      await page.goto(p.path);
      const first = page.locator("#faq details").first();
      await expect(page.locator("#faq details")).toHaveCount(7);
      await first.locator("summary").click();
      await expect(first).toHaveAttribute("open", "");
    });

    test("release notes link stays hidden until published", async ({ page }) => {
      await page.goto(p.path);
      await expect(page.locator("footer")).not.toContainText(p.lang === "ko" ? "변경 기록" : "Release notes");
    });
  });
}
