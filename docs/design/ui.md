# Screens

The app is two Tauri windows. Every screen is drawn with React, and data comes from `porch-core` through Tauri commands. No HTML files are opened in a browser. The `porch` command stays a command-line tool.

## Menu bar popover (380px wide)

A small window that opens from the menu bar icon. It shows all open sessions in a compact form. Sessions waiting on you (Needs permission, Has a question, Stopped on error) come first, the rest below a divider. Clicking a row opens Sessions in the main window. Taking you to a session's window happens only in the main window ([ADR 0013](../decisions/0013-status-board-shows-progress.md)). At the bottom are one line of account limits and today's usage, and buttons for today's summary and opening the app. While the main window is closed and the app runs from the menu bar only, it has no Dock icon.

```
┌────────────────────────────────────────┐
│ porch.                Waiting on you 1 │
│ ● Add login                   just now │
│   Needs permission · Bash · shop-web   │
│ ────────────────────────────────────── │
│ ◐ Fix order alert error     2 min ago  │
│   Working · shop-api                   │
│ ○ Clean up checkout screen 12 min ago  │
│   Done · shop-api                      │
│ ────────────────────────────────────── │
│ Claude 5h 41% · weekly 20% · today $12 │
│ [Today's summary]         [Open app]   │
└────────────────────────────────────────┘
```

- While the app makes a summary (by hand or automatically), the menu bar icon turns once every 4 seconds. With several summaries in progress, it returns to the normal icon when the last one ends. It also stops on failure, and does not turn when records are only being read. It keeps turning with the main window closed. Summaries run separately from the CLI are not included.
- The menu bar icon shows the number of sessions in Needs permission, Has a question or Stopped on error. Hidden at 0.
- Placement follows the popover position in the implementation notes below. Everything is computed in Cocoa points so units never mix across displays.

## Main window

While the main window is open, the app shows in the Dock and ⌘Tab. It stays there when minimized, and reopening it from the Dock restores the window. The close button does not quit the app; it hides the window and returns to menu-bar-only mode. Opening only the popover does not change the Dock.

The left menu switches between six screens. Summary is the center of the product, so it comes first.

```
┌──────────┬──────────────────────────────────────────┐
│ Summary  │                                          │
│ Insights │                                          │
│ Sessions │           (the selected screen)          │
│ Usage    │                                          │
│ Projects │                                          │
│──────────│                                          │
│ Settings │                                          │
└──────────┴──────────────────────────────────────────┘
```

| Screen | What it shows |
| --- | --- |
| Summary | Day / Week / Month, like a calendar app. It opens on today's day summary. The week view shows the month as rows of weeks; clicking a row shows that week's summary below. ‹ › and ←/→ move by the unit in view, and Today returns. A month-view cell has the date, work time, one line of summary and the count of pending items and blockers first recorded that day and still open; below the calendar are all pending items and blockers. Clicking a date in the month or a weekday in a week summary switches to the day view. It merges the former "Today" and "History". A summary written in another language is shown as it is, with a line saying so and a Rewrite button |
| Insights | A week's observed numbers. ‹ › and "This week" move by week. Highlights (one line per checkpoint), four KPI cells (working time, idle time, stopped on error, tool error rate, each with small per-weekday bars), session statistics (stacked bars per weekday: Working / Done, Has a question, Needs permission / Stopped on error; working time per project; concurrent sessions; a table view) and checkpoints (up to 3 picked by rules, linking to that day's summary or the project check). Hovering a bar or focusing it with the keyboard shows the details, and hovering the legend highlights only that state. A week in progress shows the values so far. For the model-written evaluation see [ADR 0012](../decisions/0012-model-evaluates-work-style.md). The screen follows the language setting, and changing the language reloads the checkpoint sentences too. |
| Sessions | What every open session (working ones included) is doing now, at a glance. Grouped by work folder, with the uncommitted change count at the head of each folder. Each row has the state (Needs permission, Has a question, Stopped on error, Working, Done), how long it has waited for permission or a question, how long it has been working, the error count for this request, one line of the last request or permission request, the model and how full its context window is, the subagent count, and "last heard N min ago" after 10 minutes without news. The order follows the [state rules](states.md). Clicking a row or pressing `Enter` brings that session's terminal window or tab to the front (only Orca tabs for now; other terminals cannot be clicked, and the scope is not decided yet). Reading the last reply, granting permission and answering do not happen inside porch ([ADR 0013](../decisions/0013-status-board-shows-progress.md)). The menu shows the number of sessions waiting on you. Refreshes every 2 seconds |
| Usage | Day / 7 days / 30 days; the arrows and ←/→ move the period, and Today returns. API-equivalent cost and tokens, cache hits, cost per hour of work, limits per account (5-hour and weekly, with reset times), monthly budget progress, bars by hour and by date (clicking a date shows that day's usage), and breakdowns by project, model and tool. The screen says that API-equivalent cost uses public API prices and differs from what a subscription costs |
| Projects | The project list (`· N checks` when there are checks) → one project's checks (last 30 days: recurring blocker kinds, commands that failed on several days, share of time blocked, blockers with no record of being resolved; clicking the evidence opens that day's summary), a 30-day row of cells per blocker kind (only blocked days are bright; hovering shows the date and the blocker, clicking opens that day's summary; with unclassified blockers, "Classify"), and below that a flow by date (what was done that day, commits, a work-time bar). The list ends with rows for repositories edited from a session started elsewhere and for "Unknown project". These rows show no dates or commit counts, and a repository-only screen explains why instead of a flow by date. The suggestions section (above the checks): text to paste for each recurring blocker, copy, and Applied / Not applied. An applied suggestion shows the numbers for the same period before and after. Shared suggestions sit at the top of the list. A row with new suggestions shows · N suggestions |
| Settings | Five tabs. **Agents**: the Claude Code and Codex list and details (names in English) (hook status, settings files, last event time, install and remove per agent; the `claude` and `codex` command paths; for Claude Code, usage limits and the status line). **Integrations**: the integration list (Obsidian only for now, and a plain folder can be picked instead of a vault; below it, Notion and Google Docs show under "Coming soon" and cannot be clicked) and details (where summary notes are saved, a preview of the location, save again). **General**: language (follow the system / 한국어 / English), excluded folders, API-equivalent cost and monthly budget, the summary agent (Claude Code or Codex; one that was not found cannot be picked) and its model (for Codex, the `codex debug models` list), the daily automatic summary, open at login. **Notifications**: summary notifications on or off, a test notification, opening macOS notification settings, and whether a click opens the porch summary or the Obsidian note. It says other notifications are not offered yet (macOS notifications are sent only for an automatic summary finishing or failing, and for a summary you started finishing or failing while the porch window is not in front). A weekly summary notification adds `· N checkpoints` after its headline (when N is at least 1). **About**: version and checking for updates |

### Day summary layout

```
┌──────────────────────────────────────────────────┐
│ Mon, Sep 29                    [Rewrite] [↓]     │
│ Headline                                         │
│ Work time · projects · sessions · requests ·     │
│ commits                                          │
│ Timeline: a row per project, one bar per request │
├──────────────────────────────────────────────────┤
│ Time by task   task name · project   min · share │
│  ▬▬▬▬▬▬▬▬▬▬ one sentence                         │
├──────────────────────────────────────────────────┤
│ Stuck tasks    what got stuck   min   resolved?  │
│  evidence · cause · how to prevent it            │
├──────────────────────────────────────────────────┤
│ Workflow diagram (only when the summary drew     │
│                   one, at most 2)                │
├──────────────────────────────────────────────────┤
│ Project name   work time · sessions · commits [>]│
│  two or three sentences of summary               │
│  Done │ Not finished │ Next                      │
│  ▸ commits  ▸ sessions                           │
└──────────────────────────────────────────────────┘
```

The unit of a summary is the **request**. Everything from the moment a person sends a request to the next request is one unit, holding the time it took, tool calls, failures and the end of their output, whether the person interrupted, and the last answer (`digest::Turn`). The summary model **only groups the requests and names the groups**: into "Time by task" chunks and "Stuck tasks". The app adds up the minutes from each request's recorded time, so the chunks add up to the day's work time. Time in sessions run at the same time is shared out, so it is never counted twice. Request numbers or minutes the model writes into the text are removed. The model draws a Mermaid flowchart only when needed, and only plain flowcharts are kept (`keep_visuals`). Hovering a chunk or a stuck task on the timeline highlights its requests. Summaries made before this approach still show the old "Notes and suggestions".

**Monthly summary.** It sits below the month view's calendar. It reads the month's weeks: a week whose weekly summary has time chunks and lies entirely inside the month is taken from that summary, and the other weeks are taken by the dates that belong to the month, so the chunks add up to the month's work time (`build_month`). It does not reread the original conversations, so it costs less than one busy day summary. It looks like the weekly summary (time by task, blockers that span weeks, per-project Done / Not finished / Next, workflow diagrams) and exports to PDF and Markdown.

**Usage screen.** The toolbar at the top matches the summary screen: the period title (the date for a day, a date range for 7 and 30 days), previous and next arrows (←/→, moving by the period, never past today), `Today`, `This day's summary` for a day, and Day / 7 days / 30 days on the right. The period is set by its end date, and `porch usage --date` does the same. Account limits always show current values, and the budget always shows this month. Below the totals are 180px-high bars by hour or by date, with the date range and gridlines. 7 and 30 days switch between tokens and API-equivalent cost. Hovering anywhere in a bar's full height, or focusing it with the keyboard, shows the details (date, API-equivalent cost, tokens, work time); with a mouse the tooltip follows the pointer, with the keyboard it stays above the bar. Gridlines sit behind the bars, and the bars are opaque so no line shows through. Clicking a date bar switches to that day and shows its usage. Hourly records hold only cost, so tokens are not estimated and show as no record. With cost display off, a note about daily tokens replaces the hourly chart. Days whose records cannot be read are marked with a dashed outline. Below come a comparison table that switches between project, model and agent (share bars and trend lines), the current account limits, and this month's budget, in that order.

**Usage labels.** Agent names show as `Claude Code` and `Codex`. `Codex (Orca)` on a limit says the value was read from Codex records Orca manages; it does not mean a separate model or a confirmed account identity. A model with no price shows `Price unknown` instead of `$0`, and a total that mixes known prices with unknown ones shows `partial` next to the amount.

**Counting usage.** Tokens are counted once per answer. Claude Code stores one answer over several lines, with more output tokens on later lines, so the answer's last line is used. Lines whose usage is all 0 are skipped (an answer through OpenRouter starts with a placeholder line of 0). An answer id is counted once across all of that day's sessions: when a session continues elsewhere (`continued-in`) or splits with `/branch`, Claude Code copies the earlier conversation into a new session file with the same ids and times. An answer that crosses midnight goes on the day it started. Subagent files (`<session>/subagents/`) go into their parent session.
Codex is counted by how much its running total grew that day. A forked session's first running total is inherited from the parent, so only that call's share (`last_token_usage`) counts, and when a running total goes down (a restart), only that call's share counts. Codex records sit in the folder of the day the session started and keep being written for days, so every date folder and `archived_sessions` are checked and files modified on or after that day are read. Sessions run in temporary folders (`/tmp`, `/var/folders`) are not projects, but they count toward usage and show as a `Temporary folders` row in the project table. The summary runner's folder and folders the user excluded are left out of usage too.
The price list is in `usage.rs` with its reference date, and unknown models count tokens only. A day report saves usage per agent and per project separately and records the version of the counting method (`usage_counting`). A day counted with an older version has only its usage recounted when opened. If any of that day's session files was deleted, the old values stay. A day with no records stays unknown. On 2026-10-03 the last 30 days were compared with ccusage, and tokens and cost matched ([research note](../research/2026-10-03-usage-vs-ccusage.md)).
Claude Code passes Claude limits only to its status line, so when this is turned on in Settings, Porch takes the status line's place, records the values, runs the original command as it was and shows its output (a single Porch line if there was none; it can be turned off). Turning it off restores the original setting, and if another tool overwrites it, Porch reports it as `Disconnected` (`statusline.rs`). A project's own status line (`.claude/settings.json`) wins over the user setting, so sessions opened in that folder do not go through Porch. Settings lists the folders opened in the last 30 days that have their own status line, and connecting one puts Porch into that project's `settings.local.json` (personal, highest precedence), running the original command as it was. Shared settings files are left alone, and if `settings.local.json` would be tracked by git, it goes only into `.git/info/exclude`. Disconnecting restores everything and deletes the files Porch made. Codex limits are read from the last limit values in Codex's records (`limits.rs`).

**Pending items and blockers.** A summary's "Not finished" and "Next" items become pending items, and stuck tasks that were not resolved become blockers, carried across dates (`open.rs`, `open.json` in the data folder). The next day's summary includes that project's pending items and blockers with numbers, and the model answers done when the records show it finished, dropped when it was given up, and open otherwise. Summarizing the same day again first undoes that day's effect. At the start, each project begins only with the items from its most recently summarized day (whether earlier ones finished cannot be known). A calendar cell shows the count of pending items and blockers first recorded that day and still open, the area below the calendar shows all of them, and a day summary shows the items first recorded and closed that day. A person can also close one with "Close manually" (recorded reason: closed manually).

A weekly summary regroups each day's chunks and stuck tasks. Days with no chunks (older summaries, or days without a summary) use per-project work time as their chunks.

`[>]` goes to that project's screen. `[↓]` exports PDF and Markdown. A weekly summary uses the same frame, with a cell per weekday and "Done / Not finished / Next".

### Insights layout

The model-written evaluation sits below the observed numbers. From the top: last week's goal line → highlights, KPIs, session statistics, checkpoints → the boundary line "Below is an evaluation the model wrote after reading the flow of your requests" → the AI use evaluation (four score cells, a table, "Mark as wrong", "Make it next week's goal") → direction changes → rewritten requests → next week's goal. Evaluation text stays in the language it was written in (as summaries do); goal names and screen text follow the current language.

There are three ways in: `· N checkpoints` after the headline of a weekly summary notification (when N is at least 1), the card at the end of the weekly summary screen ("Insights for this week · N checkpoints · M evaluated items"), and a dot in the sidebar. The dot appears only when the most recent saved evaluation came out after the screen was last opened, and opening Insights clears it. With the evaluation turned off, there is no dot.

Screen states:

| Situation | Screen |
| --- | --- |
| First week (under 7 days of records) | observed numbers only, up to the recorded days. Without a weekly summary yet, a note that the evaluation comes after the weekly summary |
| Weekly summary but no evaluation | "Not evaluated yet." and "Make evaluation" |
| Week in progress | observed numbers so far. Where the evaluation and goal picker go, a note that they appear when the week ends |
| Days without a day summary | that day's direction changes and request notes are missing. The evaluation header shows "N days without a daily summary" |
| Fewer than 20 requests | no evaluation, observed numbers only, with a note that the week has too few requests to evaluate |
| No day with a day summary | no evaluation, observed numbers only, with a note that it was not evaluated because no day has a daily summary |
| Evaluation turned off | evaluation, direction changes and rewritten requests hidden. Observed numbers and observed-number goals only |
| Making | the menu bar icon turns. The screen keeps the previous result |
| Failure or limit | "Couldn't make the evaluation." and "Make again". On a limit, a pointer to changing the summary agent (ADR 0010) |
| No goal last week | only a prompt to pick a goal on the top line |

"Insights evaluation" in Settings › General turns it on and off. Its description: "After the weekly summary, reads the flow of your requests and evaluates how you work. When off, only the observed numbers show."

## Boundary between the app and the core code

| Tauri command | What it does |
| --- | --- |
| `board_now` | the status board (`board::build`) |
| `day_report` | reads a saved day summary, or makes one with `generate` (in the background, about 40 seconds) |
| `week_report` | the weekly summary, the same way |
| `list_reports` | the dates and weeks that have summaries (the calendar on the summary screen) |

Making a summary takes long, so it runs as an async command and the screen shows progress meanwhile.

## Mark (icon)

The **"threshold"**: a standing wave over a disc of points laid out at the golden angle. Six petals turn around an empty center (`m=6, k=3, amp=0.12, ring=0.55`).

| Output | Used for |
| --- | --- |
| `mark-mac.png` (824 grid, r185, transparent margin) | every app icon, through `pnpm tauri icon` |
| `tray.png` (44×44, black + transparent) | the menu bar template image; fewer, larger dots so it does not blur at small sizes |
| `favicon.svg` | window and web favicon |

The generator scripts are not kept in the repository; the outputs above are the source. Candidates considered included photo outlines (a dog, a cat, a lantern, a mailbox) and mathematical swirls (spirals, vortex rings, an ensō and others).

## Shared rules

| Rule | In this app |
| --- | --- |
| four achromatic token steps (`canvas → surface → elevated → raised`), depth by brightness | group with spacing and thin dividers instead of card borders |
| one accent per screen (near-white fill) | only "Make summary" is a filled button; the rest are outlined |
| compact density: 13px body, 40px rows, `h-7` buttons | a one-line summary instead of number boxes (`3h 50m · 3 projects · …`) |
| state by brightness (bright dot / grey dot / empty circle) | needs you / working / nothing to do |
| inline SVG line icons (24 grid, 2px stroke) | no text glyphs (‹ › ▶) |
| lowercase wordmark + accent dot | `porch.` |
| 120ms colour transitions, respecting reduced motion | layout never moves |
| keyboard first | `⌘1` Summary · `⌘2` Insights · `⌘3` Sessions · `⌘4` Usage · `⌘5` Projects, `⇧⌘R` Rewrite (it reruns the model, so the habitual refresh key `⌘R` is not used), `↑↓` `Enter` sessions (bring that session's window to the front), `⌘+`/`⌘=` zoom in, `⌘−` zoom out, `⌘0` default zoom |

Zoom uses the WebView's zoom to scale text and layout together. It changes in 10% steps from 70% to 200%, and the main window and the popover each remember their last zoom. The current zoom shows at the bottom right, with `100% · Default` at the original size. Clicking it resets to 100%. The Zoom item in Settings also shows, adjusts and resets it.

Not used: coloured scene graphics, a glass effect in the main window (popover only), decorative animation, spinning loaders (elapsed seconds show as text instead).

The monospace font is used only where there are only Latin letters and digits (hashes, branches, paths, shortcuts). Mixed with Hangul, the spacing breaks.

The calendar cell gets brighter with that day's work time (five steps: none, under 2 hours, under 4 hours, under 6 hours, more). A day with a summary has a small dot in the cell's corner.

- States are told apart by the brightness and shape of text and dots. Colour alone never carries information.
- Light and dark themes follow the system setting.
- The summary screen has a sentence naming the agent and model that wrote it, such as "Claude Code (sonnet) read this day's work records and wrote this".

## Build order

1. Main window frame, menu bar icon (count), **Now · Today · History** (done)
2. **Projects · Settings** (done)
3. Menu bar popover (done)

## Implementation notes

- The popover is the same set of screens shown once more in a transparent window labelled `popover`. Left-clicking the menu bar icon opens it, and right-clicking shows the menu. Clicking elsewhere closes it.
- Popover position (`place_popover`): hung from the top of the work area of the display where the click landed (Cocoa coordinates), centered under the click. Near the edge of the screen it is pushed 8pt inward.
- The project screens are built from saved day reports. "Collect the last 30 days" fills in only the record numbers, without summaries (about 1 second per day). New days are made up to 30 days back, and request files are filled to twice that, 60 days, so the two periods the checks compare are counted the same way. If the two periods could not be read the same way, the checks attach no earlier value.
- Sessions in temporary folders (`/tmp`, `/private/tmp`, `/var/folders`) show on the status board but are left out of summaries and projects.
- The hook program (`porch-hook`) is inside the app bundle (Tauri `externalBin`, prepared before the build by `app/scripts/stage-hook.sh`). On install, the app copies it to `~/Library/Application Support/porch/bin/porch-hook`, and an updated app replaces the installed copy when it starts.
- The CLI (`porch`) is bundled the same way. Turning on Settings › General › Terminal command creates a symbolic link at `/usr/local/bin/porch` pointing at the `porch` inside the bundle. Nothing is copied, so the command changes with the app's updates. If the folder is not writable, the macOS administrator password dialog appears. If a `porch` that is not a link Porch made is already there, it is neither created nor removed.
- First launch: without `onboarded` in the settings, a one-screen guide shows. It lists what was found on this Mac (Claude Code records, the `claude` command, Codex config folders) and offers turning on session detection (installing hooks) and opening at login. Everything can be changed later in Settings.
- If `claude` cannot be found, the reason shows where the summary button would be.
- Open at login uses `tauri-plugin-autostart` (a LaunchAgent).
- The daily automatic summary runs only while the app is running (the time is checked every 30 seconds).

### Public prices and recounting past usage

`deepseek/deepseek-v4-flash` uses a comparison price built in from OpenRouter's public model catalogue
(`https://openrouter.ai/api/v1/models`, fetched 2026-10-01).
USD per million tokens: input 0.04186, output 0.08372, cache read 0.008372.
`deepseek/deepseek-v4.1-flash`, which appears in real records, uses input 0.03, output 0.5 and cache read 0.01, checked separately in the same catalogue. The two versions' prices are never mixed.
There is no separate cache write price, so writes are converted at the normal input price.
It applies only to the exact model ID and does not extend to `:free` or other versions.
This price is the cheapest provider's, which the catalogue shows as representative. OpenRouter picks a provider per request
(measured 2026-10-01: Fireworks 0.22/0.66/0.007, 0.45/1.8/0.009, Together 0.3/1.2/0.006),
and the transcript keeps neither the provider nor the amount billed. Checking a day of calls against the generation API, tokens
matched exactly, while the billed amount was more than twice the converted one. So the screen presents this amount as a minimum
and says the actual bill may be higher.
Prices are never fetched over the network at run time.

Coding models used through Claude Code (`ANTHROPIC_BASE_URL`) or Codex (model provider settings) also have built-in prices as of 2026-10-02.
IDs connected to the vendor directly (`glm-5.3`, `glm-5.3-flash`, `kimi-k3`, `kimi-k2.7-code`,
`qwen3.7-max`, `minimax-m3`) use each vendor's official price list, and OpenRouter IDs (`z-ai/glm-5.3`,
`moonshotai/kimi-k3`, `qwen/qwen3.7-max`, `minimax/minimax-m3` and others) use the catalogue price as above.
The two differ even for the same model, so the screen never merges them into one row. Amounts for routed models are presented as
minimums. All apply only to exact IDs (case and Claude Code's `[1m]` suffix ignored) and do not extend to sibling models such as `-flashx`,
`-highspeed` or `:batch`. MiniMax-M3 uses the tier for up to 512k input tokens per request,
and Qwen the international (Singapore) region's price, so long requests or other regions are counted lower than actual.
Local models (Codex `--oss`'s `gpt-oss:20b`, LM Studio's `openai/gpt-oss-20b`) cannot be placed by name alone,
so they stay unpriced (LM Studio's default is letter for letter the same as a paid OpenRouter ID).

Opening a saved day report recomputes the API-equivalent cost of the day, projects and sessions, and the unpriced token count, from the per-model tokens.
This works even after the original records are deleted, as long as the per-model tokens are complete. Old records whose per-model totals do not match the total tokens are not overwritten.
Hourly cost for past records whose prices changed is left blank, since there are no per-model hourly tokens
(the 7- and 30-day charts use daily totals, and today's hourly chart is counted afresh from the originals).
Summary text and per-request cost are not changed. A request's model field records only the last model,
so a request that used several models is not recomputed as that one model.

### Keeping the summary view

While the app runs, the Summary screen's Day / Week / Month tab and the chosen reference date are remembered. They stay when moving to another screen and back, or when closing and reopening the main window. Restarting the app starts on today's day view.
Opening a particular date's summary from a notification, Projects or Usage takes priority with that date and view,
and the menu bar's today's summary returns to today's day view. Scroll position is not saved or restored.
The summary view state lives in App, and the screen itself is unmounted when switching away so that a hidden screen's keyboard events do not overlap.
