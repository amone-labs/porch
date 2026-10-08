# porch — agent guide

macOS menubar app that keeps a work log for coding agents: it summarizes Claude Code and Codex
work by project, day, week and month, and shows every running session on this Mac (whatever
terminal or IDE launched it) and whether it is waiting on the user.

## Read before changing anything

- [docs/README.md](docs/README.md) — index of the design documents.
- [docs/design/states.md](docs/design/states.md) — the state rules. They are the product.
- [docs/decisions/](docs/decisions/) — settled decisions. Do not re-litigate them in code;
  write a new ADR that supersedes the old one.

## Hard rules

- **Observe, never infer.** A session's state comes only from events the agent reported
  (hooks, Claude Code's session registry). Never derive state by interpreting transcript text.
  ([ADR 0001](docs/decisions/0001-observe-not-infer.md))
- **The hook must never slow or break the agent.** `porch-hook` exits 0 on every path, prints
  nothing, adds at most a few ms over a bare process spawn, and writes only metadata (no
  prompts, no tool input).
- **No network for state.** The status board, the CLI and the hook never send anything
  anywhere. The exceptions: `porch today`/`week` send the day's digest to Claude Code
  ([ADR 0005](docs/decisions/0005-model-written-summaries.md)); `porch suggest` sends blocker
  text and the projects' CLAUDE.md/AGENTS.md to Claude Code
  ([ADR 0007](docs/decisions/0007-model-suggestions-human-applies.md)); the weekly insight
  evaluation (the app's run after the automatic weekly summary, Insights' Make evaluation,
  `porch insight --refresh`) sends the week's first requests, the day summaries' direction
  changes and request notes, the week's observed numbers (counts and times) and the days
  without a daily summary, and last week's goal, scores and disputed items to Claude Code
  ([ADR 0012](docs/decisions/0012-model-evaluates-work-style.md)); any of these goes to Codex
  instead when the person picks it in Settings ([ADR 0010](docs/decisions/0010-choose-who-writes.md)); and the main window sends the
  usage events listed in [ADR 0006](docs/decisions/0006-usage-analytics.md) to PostHog (off in
  Settings, no key = nothing sent). Never add an event or property without updating that table;
  never send paths, project names, branches, prompts, summaries, tokens or cost. The landing
  page (`site/`) sends the visit events in the same ADR's landing page table
  to the same PostHog project (cookie only
  after Allow, no key = nothing loaded); the same rule applies to that table.
- **Preserve other hooks.** Installing or removing our hooks must never touch entries that
  are not ours. Append, never insert: Codex trusts hooks by position. Never write Codex
  trust hashes.
- Agent file formats are undocumented. Unknown event kinds or format versions are counted,
  never silently mapped onto an existing state.
- **Usage counts are checked against ccusage.** After any change to how tokens are read or
  counted (`digest.rs`, `usage.rs`), compare a month of `porch usage` (with `PORCH_HOME` set to an
  empty temp dir) against `npx ccusage@latest daily --offline --json --by-agent`; they must match
  except the known differences in
  [docs/research/2026-10-03-usage-vs-ccusage.md](docs/research/2026-10-03-usage-vs-ccusage.md).

## Working on a change

- Each feature or fix gets its own git worktree, not just a branch in this checkout:
  `git worktree add ../porch-<name> -b feat/<name>` (or `fix/<name>`), then work only there.
  The main checkout may be shared with other people or agent sessions that switch branches and
  merge under you; a branch alone has let commits land on `main` unnoticed.
- Check `git branch --show-current` before each commit. Never commit to `main` directly.
- A new worktree needs `cd app && pnpm install && sh scripts/stage-hook.sh` before
  `cargo test`/`clippy` (the app bundles `porch-hook` and `porch`, which are not in git), and its first
  `cargo build` is a full one (its own `target/`).
- Remove the worktree when the branch is merged or dropped: `git worktree remove ../porch-<name>`.

## Commands

| Task | Command |
| --- | --- |
| Test | `cargo test` |
| Lint | `cargo clippy --all-targets` |
| Release build | `cargo build --release` |
| Install hooks (dry run first) | `target/release/porch install --dry-run` |
| Inspect today's events | `target/release/porch events --tail 20`, `porch keys` |
| Day summary (Claude Code writes it) | `target/release/porch today [--date YYYY-MM-DD] [--refresh] [--json]` |
| Week summary | `target/release/porch week [--date any-day-in-week] [--refresh] [--json]` |
| Weekly insight (observed numbers, checkpoints, the model's evaluation) | `target/release/porch insight [--date any-day-in-week] [--refresh] [--json]` |
| Month summary | `target/release/porch month [--date any-day-in-month] [--refresh] [--json]` |
| Tokens and cost, limits (JSON) | `target/release/porch usage [--tail DAYS]`, `porch limits` |
| Every command | `target/release/porch help` (full flags in `crates/cli/src/main.rs` header) |
| Material the summarizer reads | `target/release/porch digest [--date YYYY-MM-DD]` |
| What this Mac has (agents, hooks, claude CLI) | `target/release/porch doctor` |
| Current board | `target/release/porch now` (`--json` for machine output), live: `porch watch` |
| Cross-check against Orca's own view | `target/release/porch compare-orca` |
| Summary notes folder | `target/release/porch mirror [vaults \| set <dir> [--vault <id>] \| off]` |

Point the installer at copies with `--claude-dir` / `--codex-home` and set `PORCH_HOME` to a
temp dir; never test against the real `~/.claude/settings.json`.

App (from `app/`): `pnpm install`, then `pnpm tauri dev`. `pnpm tauri build` makes the `.app`
with `porch-hook` and `porch` inside (`scripts/stage-hook.sh` runs first). `pnpm build` type-checks the UI.
The app is a menubar
(accessory) app: its window must be focused on launch or WebKit does not paint it.

UI follows the shared rules in [docs/design/ui.md](docs/design/ui.md): achromatic tokens only (no hue), depth by brightness, one
near-white accent per screen, 13px body, `h-7` controls, white-alpha hover/active ramp, 120 ms
colour-only transitions. Mono only for Latin/digits (hashes, branches, paths, shortcuts); any
text with Hangul stays in the sans stack or its spacing breaks.

## Layout

```
crates/core/   event, install, state, board (live status), digest (one day of work per project), summary (day/week reports), writer (runs Claude Code or Codex headless to write them), health (project checks over saved reports), suggest (model-written suggestions people apply, with before/after counts), mirror (summaries as notes in a chosen folder or Obsidian vault), schedule (which periods the auto-summary owes), insight (a week of hook events replayed into time, errors and rule-picked checkpoints), insight_eval (the model's weekly evaluation, checked against the requests on record), goals (next week's goal, compared over a period of the same length); SQLite store planned
crates/hook/   porch-hook binary; depends only on core's event types
crates/cli/    `porch`: now/watch/today/week/digest/doctor/compare-orca/install/uninstall/status/events/keys/suggest/suggestions/mirror/insight
app/           Tauri 2 shell (src-tauri: commands, tray count, popover placement, auto-summary)
               + React UI (src: Summary/Insights/Sessions/Usage/Projects/Settings[Agents·Integrations·General·Notifications·About], and the popover under window label "popover")
```

Build order: hook → core → cli → app. See [docs/design/architecture.md](docs/design/architecture.md).

## Releasing

`scripts/version.sh X.Y.Z` (patch bumps by default), commit, then `scripts/release.sh`: builds,
signs with the Developer ID in the login keychain, notarizes and staples the `.dmg` through the
notarytool keychain profile `porch`, and copies it to `release/`. The updater archive and `.sig`
are signed with `~/.tauri/porch.key` (never in the repo). `scripts/publish-update.sh` then uploads
(through the aws-vault profile named in `PORCH_AWS_PROFILE`)
the build to the S3 bucket `porch-updates-6f3a11bb` and writes `latest.json`, which installed apps check at launch and
every 24 hours.

## Renaming

The product is **porch** (wordmark `porch.`, identifier `com.porch.desktop`, CLI `porch`, hook
`porch-hook`). Names reach users' machines, so any future rename must keep the old ones
recognisable: add the old hook marker to `install::LEGACY_MARKERS` and the old data dir to
`paths::LEGACY_DIR_NAMES` (moved on first launch). The first rename (from `session-board`) did
exactly this.

## Docs lifecycle

Documents and comments are written in English. Living documents under `docs/` are updated in place. Settled decisions go to
`docs/decisions/NNNN-*.md`. Unresolved questions live in `docs/open-questions.md` until
they become an ADR; that file, `docs/product/` and `docs/validation.md` are kept in the
maintainer's checkout, not in the repo.
