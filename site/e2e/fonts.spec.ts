import { expect, test } from "@playwright/test";

test("no Hangul is set in the mono stack", async ({ page }) => {
  await page.goto("/ko/");
  const offenders = await page.evaluate(() => {
    const out: string[] = [];
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    for (let n = walker.nextNode(); n; n = walker.nextNode()) {
      const text = n.textContent ?? "";
      if (!/[가-힣]/.test(text)) continue;
      const el = n.parentElement!;
      if (getComputedStyle(el).fontFamily.startsWith("ui-monospace")) out.push(text.trim().slice(0, 40));
    }
    return out;
  });
  expect(offenders).toEqual([]);
});

test("Korean wraps between words, not inside them", async ({ page }) => {
  await page.goto("/ko/");
  for (const sel of ["h1", "#top .lead p", ".story-block .lead p"]) {
    const wb = await page.locator(sel).first().evaluate((e) => getComputedStyle(e).wordBreak);
    expect(wb, sel).toBe("keep-all");
  }
});
