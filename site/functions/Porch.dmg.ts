// Serves /Porch.dmg in place of the old _redirects rule, which Cloudflare skips for routes a Function handles.
import { BUCKET_DMG, CAPTURE_URL, dmgEvent } from "../src/dmg";

interface Context {
  request: Request & { cf?: { country?: string } };
  env: { PUBLIC_POSTHOG_KEY?: string };
  waitUntil(promise: Promise<unknown>): void;
}

export function onRequestGet(ctx: Context): Response {
  const event = dmgEvent(ctx.request, ctx.env.PUBLIC_POSTHOG_KEY);
  if (event) {
    ctx.waitUntil(
      fetch(CAPTURE_URL, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(event) }).catch(() => {}),
    );
  }
  return Response.redirect(BUCKET_DMG, 302);
}

export function onRequestHead(): Response {
  return Response.redirect(BUCKET_DMG, 302);
}
