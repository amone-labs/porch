import { describe, expect, it } from "vitest";
import { dmgEvent } from "./dmg";

const SAFARI = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15";

function req(headers: Record<string, string>, country?: string) {
  return Object.assign(new Request("https://getporch.pages.dev/Porch.dmg", { headers }), country ? { cf: { country } } : {});
}

describe("dmgEvent (ADR 0006)", () => {
  it("counts a browser download with the linking page, platform and country", () => {
    const e = dmgEvent(req({ "user-agent": SAFARI, referer: "https://getporch.pages.dev/ko/" }, "KR"), "phc_x")!;
    expect(e.event).toBe("dmg_requested");
    expect(e.api_key).toBe("phc_x");
    expect(e.properties).toEqual({
      client: "browser",
      platform: "macos",
      $referrer: "https://getporch.pages.dev/ko/",
      $referring_domain: "getporch.pages.dev",
      country: "KR",
      $geoip_disable: true,
      $process_person_profile: false,
      $lib: "porch-site",
    });
  });

  it("counts curl, which the agent install prompt uses, as a direct command-line request", () => {
    const e = dmgEvent(req({ "user-agent": "curl/8.7.1" }), "phc_x")!;
    expect(e.properties.client).toBe("cli");
    expect(e.properties).not.toHaveProperty("platform");
    expect(e.properties.$referrer).toBe("$direct");
    expect(e.properties.$referring_domain).toBe("$direct");
    expect(e.properties).not.toHaveProperty("country");
  });

  it("never sends the IP address or a lasting id", () => {
    const a = dmgEvent(req({ "user-agent": SAFARI, "cf-connecting-ip": "203.0.113.7" }), "phc_x")!;
    const b = dmgEvent(req({ "user-agent": SAFARI }), "phc_x")!;
    expect(JSON.stringify(a)).not.toContain("203.0.113.7");
    expect(a.distinct_id).not.toBe(b.distinct_id);
  });

  it("skips link previews, crawlers, empty user agents and builds without a key", () => {
    for (const ua of ["Slackbot-LinkExpanding 1.0", "Mozilla/5.0 (compatible; Googlebot/2.1)", "facebookexternalhit/1.1", "Mozilla/5.0 HeadlessChrome/120", ""]) {
      expect(dmgEvent(req({ "user-agent": ua }), "phc_x"), ua).toBeNull();
    }
    expect(dmgEvent(req({ "user-agent": SAFARI }), undefined)).toBeNull();
  });

  it("treats a malformed Referer as direct", () => {
    const e = dmgEvent(req({ "user-agent": SAFARI, referer: "not a url" }), "phc_x")!;
    expect(e.properties.$referrer).toBe("$direct");
  });
});
