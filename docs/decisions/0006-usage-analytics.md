# 0006. Usage events from the app and the landing page go to PostHog

- Status: accepted
- Date: 2026-10-01
- Revised: 2026-10-05, widened the event table: the state of features that are on (`app_active`), tabs, suggestions, exports and note sync, plus the failure reason and writing agent on summaries. Weekly and monthly automatic summaries are sent too; periods with no records, and so no summary to write, are not.
- Revised: 2026-10-06, added landing page visits and the count of installer requests (`dmg_requested`). The separate 0014 is merged into this record.

## Context

porch has started going out to other people. How many keep it running, which screens they open and whether summaries get made without failing are unknown. Without asking, there is no way to know.

Until now the rule was "the status board sends nothing out", with summaries the only exception ([0005](0005-model-written-summaries.md)). porch handles sessions, project paths, branches and request text. Usage events, if sent, must carry none of these.

The landing page (`site/`) is in the same position: how many people come, where from, and whether they reach the download button are unknown, and the app's `app_opened` says nothing about what happens before the download. The landing page is public, so its address, referrers and browsers hide nothing. But storing a cookie on a visitor's device needs consent first under EU rules, and Korea's Personal Information Protection Act asks that a site say whether it uses automatic collection and how to refuse it.

## Decision

### Shared

- The app and the landing page send to the same PostHog project (Cloud US, `https://us.i.posthog.com`). Event names do not overlap, so visits, download clicks and first app launches sit side by side on one dashboard. The site's and the app's anonymous IDs are not linked.
- Only the events in the tables below, picked by hand, are sent. People are told apart only by a random anonymous ID and are never `identify`'d. Autocapture, session recording, surveys and feature flags are off.
- The project key goes in at build time as an environment variable (`VITE_POSTHOG_KEY` for the app, `PUBLIC_POSTHOG_KEY` for the landing page). A build without a key (including the app's dev builds) sends nothing.
- To add an event or property, change the tables below first.

### App

The main window sends the app's usage events.

| Event | Properties | When |
| --- | --- | --- |
| `app_opened` | `version` | the app starts and the main window first shows |
| `app_active` | `hooks_claude`, `hooks_codex` (`installed`, `partial`, `missing`, `error`, `absent`), `writer` (`claude`, `codex`), `auto_summary`, `notify`, `mirror` (`none`, `folder`, `obsidian`), `autostart`, `cli_link`, `statusline` | the app starts, and each time the date changes while it runs |
| `screen_viewed` | `screen` (`onboarding`, `summary`, `insight`, `status`, `usage`, `projects`, `settings`) | the screen changes; `onboarding` while the first-launch screen is up |
| `tab_viewed` | `screen` (`summary`, `settings`), `tab` (Summary: `day`, `week`, `month` / Settings: `agents`, `integrations`, `general`, `notifications`, `about`) | the Summary or Settings screen opens, or its tab changes |
| `session_focused` | `ok` | a clickable session in Sessions is clicked to bring its terminal tab to the front ([ADR 0013](0013-status-board-shows-progress.md)) |
| `summary_created` | `kind` (`day`, `week`, `month`), `auto`, `ok`, `duration_ms`, `writer` (`claude`, `codex`), `refresh` (whether a saved summary was rewritten), `reason` (only on failure: `limit`, `login`, `missing`, `bad_answer`, `other`) | a summary job ends. Periods with no records, and so no summary to write, are not sent |
| `onboarding_finished` | `hooks` (whether hooks were turned on), `autostart` (open at login) | the first-launch screen is finished |
| `hooks_installed` | `agent` (`claude`, `codex`, `all`), `from` (`onboarding`, `settings`) | installing hooks succeeds |
| `notification_opened` | `kind` (`day`, `week`, `month`), `target` (`obsidian`, `app`) | a summary notification is clicked to open the note or the summary |
| `suggestions_made` | `ok`, `duration_ms`, `writer`, `added`, `dropped` (only on success), `reason` (only on failure, the same values as `summary_created`) | making suggestions ends |
| `suggestion_status` | `status` (`applied`, `dismissed`, `new`) | a suggestion is applied or dismissed; `new` when applying or dismissing is undone |
| `suggestion_copied` | none | a suggestion's text is copied |
| `report_exported` | `kind` (`day`, `week`, `month`), `format` (`md`, `pdf`), `ok` | a summary is exported to a file |
| `mirror_connected` | `target` (`folder`, `obsidian`) | a notes folder or an Obsidian vault is connected |
| `mirror_disconnected` | none | note sync is disconnected |

PostHog's default properties (OS and version, screen size, browser engine) stay.

**Not sent:** sessions, project names and paths, branches, the content of requests, answers, summaries and suggestions, token counts and cost, dates, error messages. A failure reason is sent only as one value from the table. `app_active` says only whether a feature is on; it carries no folder or vault paths or names, no automatic summary time and no model name. Pageviews are off too.

- On by default. Turn it off with "Send app usage events" in Settings › General. Turning it off stops sending at once, until it is turned back on.
- The popover, the `porch` CLI and `porch-hook` send nothing. The hook has no network code.
- Automatic summaries are made by the backend, which tells the main window when they finish. The backend also tells it when the date changes (closing the window only hides it, so the app stays up for days). The main window sends those events.

### Landing page

The landing page (`/`, `/ko/`) sends visit events.

| Event | Properties | When |
| --- | --- | --- |
| `$pageview` | PostHog default properties (address, referrer, UTM, browser and OS, screen size) | a page opens |
| `$pageleave` | PostHog default properties (time on page, scroll depth) | a page is left |
| `download_clicked` | `from` (`hero`, `nav`, `close`) | a download button is clicked |
| `agent_prompt_copied` | none | the copy button of the "paste into Claude Code or Codex" prompt is clicked |
| `section_viewed` | `section` (story blocks: `works`, `time`, `stuck`, `delivered`, `suggest`, `usage` / later sections: `compare`, `privacy`, `setup`, `faq`, `agents`, `close`) | half of a section is on screen, or a section fills half the screen. Once per section per page load |
| `faq_opened` | `index` (from 0, the question order in that build) | an FAQ question is expanded |
| `language_switched` | `to` (`en`, `ko`) | the language link at the top or bottom is clicked |
| `dmg_requested` | `client` (`browser`, `cli`), `platform` (browsers only: `macos`, `ios`, `windows`, `android`, `linux`, `other`), `$referrer`, `$referring_domain`, `country` (the country code Cloudflare provides) | `/Porch.dmg` is requested with GET. Sent by the server (a Cloudflare Pages Function). Link previews and crawlers, requests without a user agent, and HEAD requests are not counted |

Events sent from the page carry the address PostHog adds (`$current_url`, `$pathname`), so the language is told by the path (`/`, `/ko/`). PostHog adds country and city from the IP.

**Installer requests:** `/Porch.dmg` is handled by `functions/Porch.dmg.ts`, which redirects with 302 to the update bucket (Cloudflare does not apply `_redirects` to a path a Function handles, so the rule moved there). Before redirecting, the server sends `dmg_requested` to PostHog. Requests that skip the button (the README link, `curl` in the agent install prompt) are counted too.

- Nothing is stored on the device, so it counts regardless of the consent banner.
- No IP address is sent. PostHog's IP geolocation is off (`$geoip_disable`; the request comes from Cloudflare, so it would get a data centre's location). The only location sent is the country code from Cloudflare.
- Each request uses a new random ID and creates no person profile (`$process_person_profile: false`). The same person downloading several times counts several times.

**Consent:** a first visit shows a banner at the bottom of the screen with two buttons of the same style, "Allow" and "Decline". Cookies are used for nothing but analytics, so there is no per-category choice.

- Before a choice and after declining, PostHog counts in cookieless mode (`cookieless_mode: "on_reject"`). Nothing is stored on the device; PostHog's server tells visitors apart by a daily hash. Return visits on different days count as different people.
- Allowing stores PostHog's cookie, so a return visit counts as the same person.
- Declining stores only that choice on the device. The banner does not come back.
- "Cookie settings" at the bottom brings the banner back to change the choice. Switching from allow to decline deletes PostHog's cookies.
- The banner and the privacy section stay short: the site collects visit statistics, stores a cookie only if allowed, and the choice changes in Cookie settings. The service name and what is sent are in the privacy policy.

**Not sent:** the content of clicked elements, input, screen recordings, errors. Heatmaps, web experiments, performance measurement and error tracking are off too. No external scripts are loaded (`disable_external_dependency_loading`).

- A build without the key loads no PostHog and shows no banner. The key is a Cloudflare Pages environment variable. `functions/Porch.dmg.ts` reads the same variable at run time; without it, it only redirects and counts nothing.
- Cookieless mode must also be turned on in the PostHog project settings. Otherwise PostHog drops the events sent before a choice and after declining.
- The 404 page, share images (`/og/`) and scene previews (`/scenes/`) send nothing.

## Consequences

- "Sends nothing out" now applies only to the status board and the hook. The notice in the app's settings and AGENTS.md change with this.
- The PostHog key is a public key (project API key), so it may sit inside the app and the page. It is still kept out of the repository and added as an environment variable at build time.
- Offline, or with PostHog blocked, the app works as before. Failed sends are not retried.
- The landing page loads PostHog separately after the page has loaded (the slim bundle, about 52 KB gzipped), so it does not get in the way of drawing the first screen. Visits that leave before it loads are not counted.
- That cookieless counting needs no consent is PostHog's position; regulators have not settled it. If that changes, nothing will be sent after declining.
- `download_clicked` counts button clicks; `dmg_requested` counts installer requests. A request means a download started, not that it finished. App updates download from the bucket directly and are not counted.
