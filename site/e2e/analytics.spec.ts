import { gunzipSync } from "node:zlib";
import { expect, test, type Page, type Request } from "@playwright/test";
import { PAGES } from "./pages";

// Visit events and the consent banner (ADR 0006). The test build has no PUBLIC_POSTHOG_KEY, so these tests
// serve the pages as a keyed build would and answer PostHog locally.

type Sent = { event: string; properties: Record<string, unknown> };

function decode(req: Request): Sent[] {
  const buf = req.postDataBuffer();
  if (!buf) return [];
  const raw = buf[0] === 0x1f && buf[1] === 0x8b ? gunzipSync(buf).toString("utf8") : buf.toString("utf8");
  const data = JSON.parse(raw);
  return Array.isArray(data) ? data : (data.batch ?? [data]);
}

async function withKey(page: Page): Promise<Sent[]> {
  const sent: Sent[] = [];
  // PostHog drops events from browsers that say they are automated.
  await page.addInitScript(() => {
    for (const k of ["webdriver", "userAgentData"]) Object.defineProperty(Navigator.prototype, k, { get: () => undefined });
  });
  await page.route(
    (u) => PAGES.some((p) => p.path === u.pathname),
    async (route) => {
      const res = await route.fetch();
      const body = (await res.text()).replace("data-key ", 'data-key="phc_e2e" ');
      expect(body).toContain('data-key="phc_e2e"');
      await route.fulfill({ response: res, body });
    },
  );
  await page.route(/posthog\.com/, async (route) => {
    sent.push(...decode(route.request()));
    await route.fulfill({ status: 200, contentType: "application/json", body: "{}" });
  });
  // Keep the download button on the page.
  await page.route("**/Porch.dmg", (route) => route.fulfill({ status: 204 }));
  return sent;
}

const named = (sent: Sent[], event: string) => sent.filter((s) => s.event === event);
const stored = (page: Page) =>
  Promise.all([
    page.context().cookies().then((cs) => cs.map((c) => c.name).filter((n) => n.includes("ph_"))),
    page.evaluate(() => Object.keys(localStorage).filter((k) => k.includes("ph_") && k !== "ph_debug")),
  ]).then(([cookies, local]) => ({ cookies, local }));

test("a build without a key loads no PostHog and shows no banner", async ({ page }) => {
  const outside: string[] = [];
  page.on("request", (r) => {
    if (r.url().includes("posthog") || r.url().includes("module.slim")) outside.push(r.url());
  });
  await page.goto("/");
  await page.waitForLoadState("load");
  await page.waitForTimeout(1500);
  await expect(page.locator("#consent")).toBeHidden();
  await expect(page.locator("[data-consent-open]")).toBeHidden();
  expect(outside).toEqual([]);
});

for (const p of PAGES) {
  test.describe(p.path, () => {
    test("counts the visit without a cookie and stores nothing before a choice", async ({ page }) => {
      const sent = await withKey(page);
      await page.goto(p.path);
      await expect(page.locator("#consent")).toBeVisible();
      await expect.poll(() => named(sent, "$pageview").length, { timeout: 10_000 }).toBe(1);
      expect(named(sent, "$pageview")[0].properties.$pathname).toBe(p.path);
      expect(named(sent, "$pageview")[0].properties.$cookieless_mode).toBe(true);
      expect(await stored(page)).toEqual({ cookies: [], local: [] });
    });

    test("the banner is a bottom-right card that says what each choice does, with two equal buttons", async ({ page }) => {
      await withKey(page);
      await page.setViewportSize({ width: 1440, height: 900 });
      await page.goto(p.path);
      const banner = page.locator("#consent");
      await expect(banner).toBeVisible();
      const box = (await banner.boundingBox())!;
      expect(1440 - (box.x + box.width)).toBeLessThanOrEqual(24);
      expect(900 - (box.y + box.height)).toBeLessThanOrEqual(24);
      expect(box.width).toBeLessThanOrEqual(400);
      await expect(banner.locator(".title")).toHaveText(p.lang === "ko" ? "방문 통계와 쿠키" : "Visit statistics and cookies");
      await expect(banner.locator("a.policy")).toHaveAttribute("href", p.lang === "ko" ? "/ko/privacy/" : "/privacy/");
      const [d, a] = await Promise.all(["decline", "allow"].map((c) => banner.locator(`[data-consent=${c}]`).boundingBox()));
      expect(Math.round(d!.width)).toBe(Math.round(a!.width));
      expect(await banner.locator("[data-consent]").evaluateAll((els) => new Set(els.map((e) => e.className)).size)).toBe(1);
    });
  });
}

test("Allow keeps the PostHog cookie and the banner stays away", async ({ page }) => {
  const sent = await withKey(page);
  await page.goto("/");
  await page.locator("[data-consent=allow]").click();
  await expect(page.locator("#consent")).toBeHidden();
  expect((await stored(page)).cookies).toEqual(["ph_phc_e2e_posthog"]);
  await page.reload();
  await expect.poll(() => named(sent, "$pageview").length, { timeout: 10_000 }).toBe(2);
  await expect(page.locator("#consent")).toBeHidden();
  expect(named(sent, "$pageview")[1].properties.$cookieless_mode).toBeFalsy();
});

test("Decline keeps only the choice and still counts without a cookie", async ({ page }) => {
  const sent = await withKey(page);
  await page.goto("/");
  await page.locator("[data-consent=decline]").click();
  await expect(page.locator("#consent")).toBeHidden();
  expect(await stored(page)).toEqual({ cookies: [], local: ["__ph_opt_in_out_phc_e2e"] });
  await page.reload();
  await expect.poll(() => named(sent, "$pageview").length, { timeout: 10_000 }).toBe(2);
  await expect(page.locator("#consent")).toBeHidden();
  expect(named(sent, "$pageview")[1].properties.$cookieless_mode).toBe(true);
});

test("Cookie settings brings the banner back, and declining after allowing drops the cookie", async ({ page }) => {
  await withKey(page);
  await page.goto("/");
  await page.locator("[data-consent=allow]").click();
  await page.locator("[data-consent-open]").click();
  await expect(page.locator("#consent")).toBeVisible();
  await page.locator("[data-consent=decline]").click();
  await expect(page.locator("#consent")).toBeHidden();
  expect((await stored(page)).cookies).toEqual([]);
});

test("sends the download, copy, FAQ, section and language events of ADR 0006", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  const sent = await withKey(page);
  await page.goto("/");
  await page.locator("[data-consent=decline]").click();

  for (const from of ["hero", "close", "nav"]) await page.locator(`[data-download=${from}]`).click();
  await page.locator("[data-copy-prompt]").click();
  await page.locator("#faq summary").nth(1).click();
  await expect
    .poll(() => named(sent, "download_clicked").map((s) => s.properties.from), { timeout: 10_000 })
    .toEqual(["hero", "close", "nav"]);
  await expect.poll(() => named(sent, "faq_opened").map((s) => s.properties.index)).toEqual([1]);
  expect(named(sent, "agent_prompt_copied")).toHaveLength(1);

  const sections = () => named(sent, "section_viewed").map((s) => s.properties.section);
  await page.evaluate(() => document.getElementById("compare")!.scrollIntoView({ block: "start" }));
  await expect.poll(sections, { timeout: 10_000 }).toContain("compare");
  await page.evaluate(() => scrollTo(0, document.body.scrollHeight));
  await expect.poll(sections, { timeout: 10_000 }).toEqual(expect.arrayContaining(["faq", "close"]));
  expect(new Set(sections()).size).toBe(sections().length);

  await page.locator("nav.nav [data-lang-switch]").click();
  await page.waitForURL("**/ko/");
  await expect.poll(() => named(sent, "language_switched").map((s) => s.properties.to)).toEqual(["ko"]);
});
