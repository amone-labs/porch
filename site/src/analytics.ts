// Visit events, exactly the landing page table in docs/decisions/0006-usage-analytics.md.
// PostHog loads after the page has, and only when the build has PUBLIC_POSTHOG_KEY (the consent banner
// carries it). Before a choice and after Decline it counts in cookieless mode and stores nothing on the device;
// Allow lets it keep its cookie so a return visit counts as the same visitor.
import type { PostHog } from "posthog-js/dist/module.slim";

type Events = {
  download_clicked: { from: "hero" | "nav" | "close" };
  agent_prompt_copied: Record<string, never>;
  section_viewed: { section: string };
  faq_opened: { index: number };
  language_switched: { to: "en" | "ko" };
};

let ready: Promise<PostHog> | null = null;

function load(key: string): Promise<PostHog> {
  return new Promise<void>((done) => {
    if (document.readyState === "complete") done();
    else addEventListener("load", () => done(), { once: true });
  })
    .then(() => import("posthog-js/dist/module.slim"))
    .then(({ default: posthog }) => {
      posthog.init(key, {
        api_host: "https://us.i.posthog.com",
        cookieless_mode: "on_reject",
        opt_out_capturing_by_default: true,
        person_profiles: "identified_only",
        capture_pageview: true,
        capture_pageleave: true,
        autocapture: false,
        rageclick: false,
        capture_performance: false,
        capture_exceptions: false,
        capture_heatmaps: false,
        capture_dead_clicks: false,
        disable_session_recording: true,
        disable_surveys: true,
        disable_web_experiments: true,
        advanced_disable_flags: true,
        disable_external_dependency_loading: true,
      });
      return posthog;
    });
}

/** `leaving`: the click navigates away, so send now rather than in the next batch. */
function track<E extends keyof Events>(event: E, props: Events[E], leaving = false) {
  ready?.then(
    (ph) => ph.capture(event, props, leaving ? { send_instantly: true, transport: "sendBeacon" } : undefined),
    () => {},
  );
}

function consent(banner: HTMLElement) {
  const show = () => (banner.hidden = false);
  const hide = () => (banner.hidden = true);
  ready?.then((ph) => {
    if (ph.get_explicit_consent_status() === "pending") show();
  }, () => {});
  banner.querySelector("[data-consent=allow]")?.addEventListener("click", () => {
    hide();
    ready?.then((ph) => ph.opt_in_capturing({ captureEventName: false }), () => {});
  });
  banner.querySelector("[data-consent=decline]")?.addEventListener("click", () => {
    hide();
    ready?.then((ph) => ph.opt_out_capturing(), () => {});
  });
  for (const open of document.querySelectorAll<HTMLElement>("[data-consent-open]")) {
    open.hidden = false;
    open.addEventListener("click", show);
  }
}

function listen() {
  for (const a of document.querySelectorAll<HTMLElement>("[data-download]")) {
    a.addEventListener("click", () => track("download_clicked", { from: a.dataset.download as Events["download_clicked"]["from"] }, true));
  }
  for (const a of document.querySelectorAll<HTMLAnchorElement>("a[data-lang-switch]")) {
    a.addEventListener("click", () => track("language_switched", { to: a.hreflang as "en" | "ko" }, true));
  }
  document.querySelector("[data-copy-prompt]")?.addEventListener("click", () => track("agent_prompt_copied", {}));
  document.querySelectorAll("#faq details").forEach((d, index) => {
    d.addEventListener("toggle", () => {
      if ((d as HTMLDetailsElement).open) track("faq_opened", { index });
    });
  });
  // A section counts once half of it is on screen, or it fills half the screen (the tall ones never show half).
  const io = new IntersectionObserver(
    (entries) => {
      for (const e of entries) {
        if (e.intersectionRatio < 0.5 && e.intersectionRect.height < innerHeight / 2) continue;
        io.unobserve(e.target);
        track("section_viewed", { section: (e.target as HTMLElement).dataset.section! });
      }
    },
    { threshold: [0, 0.1, 0.2, 0.3, 0.4, 0.5] },
  );
  for (const s of document.querySelectorAll("[data-section]")) io.observe(s);
}

export function start() {
  const banner = document.getElementById("consent");
  const key = banner?.dataset.key;
  if (!banner || !key) return;
  ready = load(key);
  consent(banner);
  listen();
}
