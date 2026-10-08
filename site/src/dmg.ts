// The installer link (ADR 0006). /Porch.dmg redirects to the update bucket, and each GET from a browser or a
// command-line client also sends one `dmg_requested` event to PostHog from the server: the page that linked here,
// the country Cloudflare reports and the kind of client. No IP address, no cookie, nothing kept on the device.
export const BUCKET_DMG = "https://porch-updates-6f3a11bb.s3.ap-northeast-2.amazonaws.com/Porch.dmg";
export const CAPTURE_URL = "https://us.i.posthog.com/i/v0/e/";

// Link previews and crawlers fetch the link without anyone downloading.
const BOT = /bot|crawl|spider|slurp|preview|facebookexternalhit|embedly|whatsapp|telegram|discord|skype|headless/i;

type Platform = "macos" | "ios" | "windows" | "android" | "linux" | "other";

function platform(ua: string): Platform {
  if (/iPhone|iPad|iPod/.test(ua)) return "ios";
  if (/Macintosh|Mac OS X/.test(ua)) return "macos";
  if (/Windows/.test(ua)) return "windows";
  if (/Android/.test(ua)) return "android";
  if (/Linux|X11/.test(ua)) return "linux";
  return "other";
}

export type DmgEvent = {
  api_key: string;
  event: "dmg_requested";
  distinct_id: string;
  properties: Record<string, string | boolean>;
};

/** The event for one request, or null when it is not counted (no key, no user agent, a bot). */
export function dmgEvent(request: Request & { cf?: { country?: string } }, key: string | undefined): DmgEvent | null {
  const ua = request.headers.get("user-agent") ?? "";
  if (!key || !ua || BOT.test(ua)) return null;
  const browser = ua.startsWith("Mozilla/");
  const referrer = request.headers.get("referer");
  let domain = "$direct";
  try {
    if (referrer) domain = new URL(referrer).host;
  } catch {
    // A malformed Referer counts as direct.
  }
  return {
    api_key: key,
    event: "dmg_requested",
    distinct_id: crypto.randomUUID(),
    properties: {
      client: browser ? "browser" : "cli",
      ...(browser ? { platform: platform(ua) } : {}),
      $referrer: referrer && domain !== "$direct" ? referrer : "$direct",
      $referring_domain: domain,
      ...(request.cf?.country ? { country: request.cf.country } : {}),
      // The request comes from Cloudflare, so PostHog's own lookup would name a data centre, not the visitor.
      $geoip_disable: true,
      $process_person_profile: false,
      $lib: "porch-site",
    },
  };
}
