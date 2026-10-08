import { expect, test } from "@playwright/test";

const state = (page: import("@playwright/test").Page) =>
  page.locator(".hero-video video").evaluate((v: HTMLVideoElement) => ({ paused: v.paused, time: v.currentTime }));

// "/" at 1440×900 is the tight case: the English headline wraps longer, so less of the video is above the fold.
test("plays in view, and the button pauses it for good", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/");
  const toggle = page.locator(".hero-video .video-toggle");
  await expect(toggle).toBeVisible();
  await expect.poll(async () => (await state(page)).paused).toBe(false);
  await expect(toggle).toHaveAttribute("aria-label", "Pause");
  await toggle.click();
  await expect.poll(async () => (await state(page)).paused).toBe(true);
  await expect(toggle).toHaveAttribute("aria-label", "Play");
  await page.evaluate(() => window.scrollBy(0, 3000));
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.waitForTimeout(500);
  expect((await state(page)).paused).toBe(true);
});

test("pauses when scrolled away", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/ko/");
  await expect.poll(async () => (await state(page)).paused).toBe(false);
  await page.evaluate(() => document.getElementById("compare")!.scrollIntoView());
  await expect.poll(async () => (await state(page)).paused).toBe(true);
});

test("reduced motion: no autoplay, play button offered", async ({ browser }) => {
  const ctx = await browser.newContext({ reducedMotion: "reduce", viewport: { width: 1440, height: 900 } });
  const page = await ctx.newPage();
  await page.goto("http://127.0.0.1:4329/");
  await page.waitForTimeout(800);
  expect((await state(page)).paused).toBe(true);
  await expect(page.locator(".hero-video .video-toggle")).toHaveAttribute("aria-label", "Play");
  await page.locator(".hero-video .video-toggle").click();
  await expect.poll(async () => (await state(page)).paused).toBe(false);
  await ctx.close();
});

test("a failed video leaves the poster and a Play button", async ({ page }) => {
  await page.route(/\/video\/hero-en\.(mp4|webm)$/, (r) => r.abort());
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/");
  await page.waitForTimeout(1000);
  await expect(page.locator(".hero-video .video-toggle")).toBeVisible();
  await expect(page.locator(".hero-video .video-toggle")).toHaveAttribute("aria-label", "Play");
  await expect(page.locator(".hero-video video")).toHaveAttribute("poster", "/video/hero-en.jpg");
});

test("a rejected first source falls back to the MP4 and still plays", async ({ page }) => {
  await page.route(/\/video\/hero-en\.webm$/, (r) => r.abort());
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/");
  await expect.poll(async () => (await state(page)).paused, { timeout: 8000 }).toBe(false);
  // The webm's error must not mark the video as broken: pause, then play again.
  await page.locator(".hero-video .video-toggle").click();
  await expect.poll(async () => (await state(page)).paused).toBe(true);
  await page.locator(".hero-video .video-toggle").click();
  await expect.poll(async () => (await state(page)).paused).toBe(false);
});

test.describe("no JS", () => {
  test.use({ javaScriptEnabled: false });
  test("poster only, no button", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".hero-video .video-toggle")).toBeHidden();
    await expect(page.locator(".hero-video video")).toHaveAttribute("poster", "/video/hero-en.jpg");
  });
});
