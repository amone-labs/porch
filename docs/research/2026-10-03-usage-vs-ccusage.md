# porch usage compared with ccusage (2026-10-03)

Thirty days of usage that porch counted on the developer's Mac were compared with [ccusage](https://github.com/ccusage/ccusage) v20.0.26
(`npx ccusage@latest daily --offline --json --by-agent -s <from> -u <to> -z <time zone>`).
porch counted about 5% more tokens (Claude Code +5.9%, Codex −9.9%); all seven causes were found and fixed.
This note keeps the counting rules, how they were checked and the differences that remain; the measured amounts themselves are left out.

## Result

| | porch before the fix | porch after the fix |
| --- | --- | --- |
| Tokens, compared with ccusage | +4.7% | identical |
| Claude Code tokens | +5.9% | identical |
| Codex tokens | −9.9% | identical |
| API-equivalent cost | +4.0% | within 0.1% (DeepSeek pricing, below) |

After the fix, tokens and cost match for every model. Only two differences remain.

- **DeepSeek pricing:** porch uses the cheapest price in the OpenRouter catalogue; ccusage uses 90% of DeepSeek's direct API price
  (0.27/1.08/0.0054 per million tokens), so porch's DeepSeek cost comes out lower. The difference is intended.
- **An answer that crosses midnight:** porch puts it on the day it started, ccusage on the day it ended.

Recounting the saved day reports gave the same result as a fresh count, down to project, agent and model. Summary text and project lists did not change.

## How it was checked

1. The date × model tables of porch and ccusage were compared to find the cells where the difference concentrated.
2. A separate Python calculator that reads the raw records computed both "the porch way" and "the fixed way". The porch way reproduced porch's values exactly,
   and the fixed way matched ccusage.
3. The raw lines behind each cause were opened and checked directly.
4. The fixed porch was run against an empty `PORCH_HOME` and against a copy of the real saved reports, and compared with ccusage again.

Note: in that shell `CODEX_HOME` pointed at an Orca account folder, so ccusage read only that folder. All recent Codex records on that Mac were
in that folder, so the comparison was not affected. porch reads `~/.codex` and the Orca account folders together.

## Causes

### Claude Code

| Cause | Effect | Evidence |
| --- | --- | --- |
| Copied conversations counted again. When a session continues in another worktree (`continued-in`) or splits with `/branch`, the earlier conversation is copied into a new file with the same ids and times, and porch removed duplicates per file | most of the overcount; one continued session alone had almost every answer copied | the end of the original session file: `{"type":"continued-in","continuedInSessionId":"<id>"}` |
| The usage of an answer's first line was used. Later lines of one answer have more output tokens | output tokens undercounted across thousands of answers | `output_tokens` of one id over its lines: 37 → 37 → 211 |
| An answer that crossed midnight was counted on both days | a few answers | lines of the same id on two consecutive dates |
| Sessions in temporary folders were left out of usage too | their usage was missing | sessions with a cwd under `/private/tmp/…`. The rule that leaves them out of summaries and projects also left them out of usage |

### Codex

| Cause | Effect | Evidence |
| --- | --- | --- |
| Only the previous day's and the current day's date folders were read. Records live in the folder of the day they started, and a session can keep writing for more than ten days | usage missing on several days, at times a large share of a day | one rollout file under its start-date folder kept being written on four later days |
| A forked session inherited its parent's running total | a large overcount on the day of the fork | the first `token_count` of a forked session: `total` in the millions, `last` a single call's share |
| The call where a running total restarted was dropped | a small undercount | `total` drops to the value of `last` |

## What was fixed

The counting rules are in [ui.md](../design/ui.md) (counting usage), the record formats in
[data-sources.md](../design/data-sources.md#record-formats-to-know-when-counting-usage).

- Claude: an answer id is counted once across all of that day's sessions; usage comes from its last line and the date from its first (`Replies` in `digest.rs`).
- Codex: every date folder and `archived_sessions` are read for files modified on or after that day (`codex_rollouts_in`), and for the first event and for events where the running total
  went down, `last_token_usage` is counted (`read_codex`).
- Sessions in temporary folders go into usage through `DayDigest.scratch_usage` and show as `Temporary folders` in the Usage screen's project table.
- Day reports save usage per agent and per project (`agent_usage`, `project_usage`) and the version of the counting method (`usage_counting`).
  A report counted with an older version has only its usage recounted when opened. If any of that day's session files was deleted, the old values stay.

## Still open

- Copied conversations now count once for usage only. The copied session's prompts and turns stay, and can enter summary material twice.
- Records copied when a Codex session is resumed (`codex resume`) were not in the compared data, so they are not handled. ccusage merges the same usage events from
  different sessions (the `dedupes_matching_grouped_codex_usage_events_from_distinct_sessions` test in `rust/adapters/codex/src/lib.rs`).
- After any change that affects usage, run the same comparison again. ccusage is the reference; the pricing (DeepSeek) and midnight-boundary differences are expected.
