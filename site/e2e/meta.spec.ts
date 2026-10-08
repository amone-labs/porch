import { expect, test } from "@playwright/test";
import { PAGES } from "./pages";

for (const p of PAGES) {
  test(`${p.path} never ships the build's localhost into absolute URLs`, async ({ page }) => {
    await page.goto(p.path);
    expect(await page.content()).not.toContain("localhost:4321");
  });

  test(`${p.path} description is filled from the agent list`, async ({ page }) => {
    await page.goto(p.path);
    const d = await page.locator('meta[name="description"]').getAttribute("content");
    expect(d).toContain(p.lang === "ko" ? "클로드코드와 코덱스" : "Claude Code and Codex");
    expect(d).not.toContain("{");
  });

  test(`${p.path} link previews are a large card`, async ({ page }) => {
    await page.goto(p.path);
    await expect(page.locator('meta[name="twitter:card"]')).toHaveAttribute("content", "summary_large_image");
    await expect(page.locator('meta[property="og:type"]')).toHaveAttribute("content", "website");
    await expect(page.locator('meta[property="og:locale"]')).toHaveAttribute("content", p.lang === "ko" ? "ko_KR" : "en_US");
  });

  test(`${p.path} sentences use muted, not faint`, async ({ page }) => {
    await page.goto(p.path);
    const muted = await page.evaluate(() => {
      const v = getComputedStyle(document.documentElement).getPropertyValue("--muted").trim().split(/\s+/).join(", ");
      return `rgb(${v})`;
    });
    for (const sel of ["#top .platform", "#top .hero-video .caption", "#privacy .under", "#setup .requirements", "footer .about p"]) {
      await expect(page.locator(sel), sel).toHaveCSS("color", muted);
    }
  });
}
