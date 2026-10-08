import { describe, expect, it } from "vitest";
import { en } from "./i18n/en";
import { ko } from "./i18n/ko";
import { fill } from "./i18n/util";
import { GET as robots } from "./pages/robots.txt";
import { GET as sitemap } from "./pages/sitemap.xml";

const site = new URL("https://getporch.pages.dev");
const call = (route: typeof robots, s?: URL) => (route as (c: { site?: URL }) => Response)({ site: s });

describe("titles and descriptions", () => {
  const meta = { en: { t: fill(en.meta.title, "en"), d: fill(en.meta.description, "en") }, ko: { t: fill(ko.meta.title, "ko"), d: fill(ko.meta.description, "ko") } };
  it("carry no em or en dash", () => {
    for (const m of Object.values(meta)) expect(`${m.t} ${m.d}`).not.toMatch(/[—–]/);
  });
  it("stay within search-result lengths", () => {
    expect(meta.en.t.length).toBeLessThanOrEqual(60);
    expect(meta.ko.t.length).toBeLessThanOrEqual(35);
    expect(meta.en.d.length).toBeGreaterThanOrEqual(120);
    expect(meta.en.d.length).toBeLessThanOrEqual(155);
    expect(meta.ko.d.length).toBeLessThanOrEqual(110);
  });
  it("write the brand as Porch", () => {
    for (const m of Object.values(meta)) expect(m.t.startsWith("Porch: ")).toBe(true);
  });
});

describe("robots.txt and sitemap.xml", () => {
  it("robots allows everything and names the sitemap", async () => {
    const body = await call(robots, site).text();
    expect(body).toContain("Allow: /");
    expect(body).toContain("Sitemap: https://getporch.pages.dev/sitemap.xml");
  });
  it("sitemap lists both home pages with their language alternates, nothing else", async () => {
    const body = await call(sitemap, site).text();
    expect([...body.matchAll(/<loc>([^<]+)<\/loc>/g)].map((m) => m[1])).toEqual(["https://getporch.pages.dev/", "https://getporch.pages.dev/ko/"]);
    expect(body).toContain('hreflang="ko" href="https://getporch.pages.dev/ko/"');
  });
  it("without SITE_URL neither writes a relative origin", async () => {
    expect(await call(robots).text()).not.toContain("Sitemap");
    expect(await call(sitemap).text()).not.toContain("<loc>");
  });
});
