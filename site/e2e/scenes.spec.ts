import { expect, test } from "@playwright/test";

const IDS = ["s2", "s4", "s5", "s6", "s8", "s9"];
const SHOT_IDS = ["s4", "s5", "s6", "s8", "s9"]; // s2 is a clip: its still is the video's poster

for (const locale of ["en", "ko"]) {
  test.describe(`/scenes/${locale}/`, () => {
    test("renders every scene in both variants", async ({ page }) => {
      await page.goto(`/scenes/${locale}/`);
      for (const id of IDS) {
        await expect(page.locator(`.scene-full[data-scene="${id}"]`), id).toHaveCount(1);
        await expect(page.locator(`.scene-card[data-scene="${id}"]`), id).toHaveCount(1);
      }
    });

    test("every desktop scene is this locale's stills", async ({ page }) => {
      await page.goto(`/scenes/${locale}/`);
      for (const id of SHOT_IDS) {
        const img = page.locator(`.scene-full[data-scene="${id}"] .shot img`);
        await expect(img, id).toHaveAttribute("src", new RegExp(`^/shots/${locale}/`));
        await img.scrollIntoViewIfNeeded(); // stills are loading="lazy"
        await expect.poll(() => img.evaluate((i: HTMLImageElement) => i.complete && i.naturalWidth > 0), { message: id }).toBe(true);
      }
    });

    test("the works scene is this locale's clip, its still as the poster, with a pause button", async ({ page }) => {
      await page.goto(`/scenes/${locale}/`);
      const video = page.locator('.scene-full[data-scene="s2"] video');
      await expect(video).toHaveAttribute("poster", `/shots/${locale}/works.webp`);
      await expect(video).toHaveAttribute("aria-hidden", "true");
      expect(await video.locator("source").evaluateAll((els) => els.map((e) => e.getAttribute("src")))).toEqual([`/video/works-${locale}.webm`, `/video/works-${locale}.mp4`]);
      await expect(page.locator('.scene-full[data-scene="s2"] .video-toggle')).toBeVisible();
    });

    for (const width of [1100, 1440, 760]) {
      test(`focus boxes stay inside the still at ${width}px`, async ({ page }) => {
        await page.setViewportSize({ width, height: 900 });
        await page.goto(`/scenes/${locale}/`);
        for (const id of SHOT_IDS) {
          const fig = (await page.locator(`.scene-full[data-scene="${id}"]`).boundingBox())!;
          for (const b of await page.locator(`.scene-full[data-scene="${id}"] .focus`).all()) {
            const r = (await b.boundingBox())!;
            expect(r.x, id).toBeGreaterThanOrEqual(fig.x - 1);
            expect(r.x + r.width, id).toBeLessThanOrEqual(fig.x + fig.width + 1);
            expect(r.y + r.height, id).toBeLessThanOrEqual(fig.y + fig.height + 1);
          }
        }
      });
    }

    test("cards are legible at 375px without shrinking", async ({ page }) => {
      await page.setViewportSize({ width: 375, height: 800 });
      await page.goto(`/scenes/${locale}/`);
      const problems = await page.evaluate(() => {
        const out: string[] = [];
        for (const card of document.querySelectorAll<HTMLElement>(".scene-card")) {
          for (let el: HTMLElement | null = card; el; el = el.parentElement) {
            if (getComputedStyle(el).transform !== "none") out.push(`${card.dataset.scene}: transform on ${el.className}`);
          }
          for (const l of card.querySelectorAll<HTMLElement>(".lbl")) if (parseFloat(getComputedStyle(l).fontSize) < 12) out.push(`${card.dataset.scene}: label ${l.textContent}`);
          for (const n of card.querySelectorAll<HTMLElement>(".num")) if (parseFloat(getComputedStyle(n).fontSize) < 14) out.push(`${card.dataset.scene}: number ${n.textContent}`);
          if (card.getBoundingClientRect().right > 375) out.push(`${card.dataset.scene}: overflows`);
        }
        return out;
      });
      expect(problems).toEqual([]);
    });

    test("app stills show the whole window, uncropped and unclipped", async ({ page }) => {
      await page.setViewportSize({ width: 1440, height: 900 });
      await page.goto(`/scenes/${locale}/`);
      for (const id of ["s4", "s5", "s6", "s8", "s9"]) {
        const img = page.locator(`.scene-full[data-scene="${id}"] .shot img`);
        await img.scrollIntoViewIfNeeded();
        await expect.poll(() => img.evaluate((i: HTMLImageElement) => i.naturalWidth), { message: id }).toBeGreaterThan(0);
        // the full capture window, sidebar included: 1180 wide (2×), at least the 760 app height; usage and the
        // Obsidian note are taller so the hourly chart and limits, or the time table and blocked task, both fit
        const [w, h] = await img.evaluate((i: HTMLImageElement) => [i.naturalWidth, i.naturalHeight]);
        expect(w, id).toBe(2360);
        expect(h, id).toBeGreaterThanOrEqual(1520);
        // window controls float over the app like macOS's hidden title bar: no separate strip, no divider, real colours
        const bar = page.locator(`.scene-full[data-scene="${id}"] .titlebar`);
        await expect(bar, id).toHaveCount(1);
        await expect(bar, id).toHaveCSS("position", "absolute");
        await expect(bar, id).toHaveCSS("border-bottom-width", "0px");
        const dots = await bar.locator("i").evaluateAll((els) => els.map((e) => getComputedStyle(e).backgroundColor));
        expect(new Set(dots).size, id).toBe(3);
        for (const c of dots) {
          const [r, g, b] = c.match(/\d+/g)!.map(Number);
          expect(Math.max(r, g, b) - Math.min(r, g, b), `${id} ${c}`).toBeGreaterThan(40);
        }
        const fig = (await page.locator(`.scene-full[data-scene="${id}"]`).boundingBox())!;
        const box = (await img.boundingBox())!;
        expect(box.y + box.height, id).toBeLessThanOrEqual(fig.y + fig.height + 1);
        expect(box.x + box.width, id).toBeLessThanOrEqual(fig.x + fig.width + 1);
      }
    });

    test("reduced motion shows every focus box without animating", async ({ browser }) => {
      const ctx = await browser.newContext({ reducedMotion: "reduce", colorScheme: "dark" });
      const page = await ctx.newPage();
      await page.goto(`http://127.0.0.1:4329/scenes/${locale}/`);
      const opacities = await page.locator(".scene-full .focus").evaluateAll((els) => els.map((e) => getComputedStyle(e).opacity));
      expect(opacities.length).toBeGreaterThanOrEqual(8);
      expect(new Set(opacities)).toEqual(new Set(["1"]));
      await ctx.close();
    });

    test("focus boxes are drawn for the dark app still, whatever the page theme", async ({ browser }) => {
      for (const colorScheme of ["light", "dark"] as const) {
        const ctx = await browser.newContext({ colorScheme, reducedMotion: "reduce" });
        const page = await ctx.newPage();
        await page.goto(`http://127.0.0.1:4329/scenes/${locale}/`);
        const focus = page.locator('.scene-full[data-scene="s4"] .focus').first();
        await expect(focus, colorScheme).toHaveCSS("border-top-color", "rgba(237, 237, 240, 0.9)");
        expect(await focus.evaluate((e) => getComputedStyle(e).boxShadow), colorScheme).toContain("rgba(0, 0, 0, 0.4)");
        await ctx.close();
      }
    });

    test("usage still shows the hourly chart and the limits, each boxed", async ({ page }) => {
      await page.goto(`/scenes/${locale}/`);
      await expect(page.locator('.scene-full[data-scene="s6"] .focus')).toHaveCount(2);
    });

    test("card text follows the copy", async ({ page }) => {
      await page.goto(`/scenes/${locale}/`);
      if (locale === "ko") {
        await expect(page.locator('.scene-card[data-scene="s2"]')).toContainText("허락 필요");
        await expect(page.locator('.scene-card[data-scene="s8"]')).toContainText("적용 후 14일");
        // title and effect must belong to the same suggestion (the applied orderId one)
        await expect(page.locator('.scene-card[data-scene="s8"]')).toContainText("재시도 전에 orderId를 확인하는 테스트부터");
        await expect(page.locator('.scene-card[data-scene="s9"]')).toContainText("2026년 9월 29일 (화)");
        await expect(page.locator('.scene-card[data-scene="s9"]')).toContainText("porch/daily/2026-09-29.md");
      } else {
        await expect(page.locator('.scene-card[data-scene="s8"]')).toContainText("14 days after");
        await expect(page.locator('.scene-card[data-scene="s8"]')).toContainText("Test for orderId before touching retries");
        await expect(page.locator('.scene-card[data-scene="s9"]')).toContainText("Tue, Sep 29, 2026");
      }
    });
  });
}
