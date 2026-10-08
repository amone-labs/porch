# porch landing page

Static Astro site. English at `/`, Korean at `/ko/`.

| Task | Command |
| --- | --- |
| Install | `pnpm install && pnpm exec playwright install chromium` |
| Dev server | `pnpm dev` |
| Unit tests | `pnpm test` |
| Build (type-checks first) | `pnpm build` |
| Serve the build on :4329 | `pnpm preview` |
| Page tests (builds and serves on :4329) | `pnpm e2e` |
| Share images | `pnpm build && pnpm og` |
| Production build | `SITE_URL=https://<domain> pnpm build` |
| Hero video, story stills and the works clip | `pnpm capture` in one terminal, then `pnpm video` (needs Rust and `/Applications/Obsidian.app` for the notes still; `CAPTURE_PORT` moves the capture server off 1431) |

Set `SITE_URL` for any build you deploy. Without it, hreflang links are root-relative,
`og:image`, `og:url` and the canonical link are left out, and `robots.txt` / `sitemap.xml` name no origin.
The sitemap lists only `/` and `/ko/`; pages marked `noindex` (404, og, scenes) stay out of it.

`pnpm preview` is `scripts/serve.mjs`, not `astro preview`: Astro 7 detaches its preview server,
so Playwright could not stop it and later runs saw a stale build.

## Deploy

Cloudflare Pages builds `site/` on every push to `main`, and gives each branch and PR a
preview URL. Project settings (dashboard → Workers & Pages → porch → Settings → Build):

| Setting | Value |
| --- | --- |
| Root directory | `site` |
| Build command | `pnpm build:deploy` |
| Build output directory | `dist` |
| Build watch paths | include `site/*` |
| Environment variables (Production) | `SITE_URL=https://<domain>`, `NODE_VERSION=22`, `PUBLIC_POSTHOG_KEY=<the app's PostHog project key>` |

`/Porch.dmg` is served by `functions/Porch.dmg.ts`: it redirects to the update bucket and counts the request in
PostHog (ADR 0006). There is no `_redirects` rule for it; Cloudflare skips `_redirects` on routes a Function
handles. Try it locally with `pnpm build:deploy && pnpm dlx wrangler@4 pages dev dist`.

Without `PUBLIC_POSTHOG_KEY` the build loads no PostHog and shows no consent banner (ADR 0006), so previews
send nothing. The PostHog project must have cookieless mode turned on, or the events counted before a choice
and after Decline are dropped.

`build:deploy` is `build` without the media staleness check. The check hashes app screens, so
every app commit would otherwise fail the deploy; the site keeps serving the last committed media
until someone runs `pnpm video`. Run `pnpm build` locally to see whether the media are stale.

## Where things live

- Copy: `src/i18n/en.ts`, `src/i18n/ko.ts`. The copy appendix
  `docs/landing-page-copy.md` is what reviewers read; its hero, story blocks and scene data
  are generated from the `.ts` files with `pnpm appendix` (`appendix.test.ts` fails if they drift).
- Supported agents: `src/agents.ts` only. Lists of names in the copy are `{agents}` templates.
  Add an agent only once porch supports it.
- Mockup numbers: `src/mock.ts`. Shares are computed.
- Scenes: `src/components/scenes/` (S2, S4–S6, S8, S9). Each has a `full` (stage) and `card` (phone) variant; S2's `full`
  is a looping clip (`video/build-works.mjs`, played by `src/video-autoplay.ts` like the hero). Base styles
  are the final frame, so reduced motion shows the finished scene. Preview at `/scenes/en/` and `/scenes/ko/`.
- Story layout: `src/components/Story.astro`. Document flow by default; the sticky stage only switches on
  with JS at 1100×760 and up.

## Mockups follow the app

The mockups are drawn in HTML but must show only what the app shows. When these app files change
their labels or formats, update the mockup in the same commit or the next one:

| Mockup | App source |
| --- | --- |
| State names, dots | `app/src/components/ui.tsx` (`LABEL`, `stateLabel`, `StateDot`) — `copy.test.ts` checks the Korean session list against `LABEL` |
| Times (방금, 12분째, 2분 전) | `app/src/format.ts` |
| Summary header and sections | `app/src/components/DayView.tsx` |
| Usage labels, "다시 참" | `app/src/screens/Usage.tsx` |
| S2 clip and card | The list is the real Sessions screen (`pnpm shoot`); the terminals and the menu bar are drawn in `video/build-works.mjs` (tray icon `app/src-tauri/icons/tray.png`, count as `lib.rs` sets the tray title) — `works.test.ts` ties the card's sessions and terminals to the clip |
| S6 limits | `app/src/screens/Usage.tsx` (`LimitRow`) |
| S9 phone card | `video/hero/notes-<lang>.md`, the note porch wrote for the still (`pnpm obsidian`) — `delivered.test.ts` checks the card's title, headline and rows against it |

After copy changes, review the copy appendix for tone and facts before it ships.
