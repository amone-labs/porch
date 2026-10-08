import { expect, test } from "@playwright/test";
import { PAGES } from "./pages";

const NOT_SUPPORTED = /\b(Cursor|Gemini|Copilot|Windsurf|Aider|Cline)\b/;

for (const p of PAGES) {
  test.describe(p.path, () => {
    test("agent table has one column per supported agent", async ({ page }) => {
      await page.goto(p.path);
      const heads = page.locator("#agents thead th");
      await expect(heads).toHaveCount(3);
      await expect(heads.nth(1)).toHaveText(p.lang === "ko" ? "클로드코드" : "Claude Code");
      await expect(heads.nth(2)).toHaveText(p.lang === "ko" ? "코덱스" : "Codex");
      await expect(page.locator("#agents tbody tr")).toHaveCount(9);
    });

    test("no unsupported agent is named anywhere", async ({ page }) => {
      await page.goto(p.path);
      expect(await page.locator("body").innerText()).not.toMatch(NOT_SUPPORTED);
    });

    test("no template placeholder survives the build", async ({ page }) => {
      await page.goto(p.path);
      expect(await page.content()).not.toMatch(/\{(agents|time|projects|sessions|requests|commits)[^}]*\}/);
    });
  });
}

for (const path of ["/", "/ko/", "/og/en/", "/og/ko/", "/scenes/en/", "/scenes/ko/"]) {
  test(`${path} renders no raw emphasis markers`, async ({ page }) => {
    await page.goto(path);
    const text = await page.locator("body").innerText();
    expect(text).not.toMatch(/(^|\s)_[^_\s][^_]*_(?=[\s.,;:!?)]|$)/);
    expect(text).not.toContain("[[");
    expect(text).not.toContain("**");
  });
}
