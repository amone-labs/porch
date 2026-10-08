# Summary note format (`porch_format: 1`)

With Obsidian sync on, porch writes day, week and month summaries as Markdown notes to the folder you picked
([ADR 0008](../decisions/0008-mirror-to-a-chosen-folder.md)). This document is the format of those notes.
It is kept as a public promise so that other tools can read the notes.

## Location

```
<sync folder>/<daily folder>/2026-10-02.md
<sync folder>/<weekly folder>/2026-W40.md
<sync folder>/<monthly folder>/2026-10.md
```

- The sync folder is `<vault>/porch` when an Obsidian vault is picked, or the folder itself when a plain folder is picked.
- The subfolder names default to `daily`, `weekly` and `monthly`. Left empty, notes go straight into the sync folder.
  Several levels such as `Journal/Daily` work, but paths with `..` or starting with `/` or `~` are refused.
- Weekly file names use ISO week numbers. porch's weeks start on Monday, so they match ISO weeks.
  The turn of the year follows the ISO rules (the week containing 2026-12-28 is `2026-W53`).

## Frontmatter

```yaml
---
porch_format: 1
type: daily
date: 2026-10-02
headline: "Landing page done, usage screen reworked"
projects: ["porch", "shop-api"]
active_minutes: 312
cost_usd: 23.40
generated: 2026-10-02T09:00:05Z
tags: [porch]
---
```

| Field | Format | Meaning |
| --- | --- | --- |
| `porch_format` | integer | the version of this format; `1` for now |
| `type` | `daily` \| `weekly` \| `monthly` | the kind of note |
| `date` | `YYYY-MM-DD` or `YYYY-MM` | the day for daily notes, that week's Monday for weekly notes, the month for monthly notes |
| `headline` | quoted string | the summary's one-line title. Line breaks become spaces; `"` and `\` are escaped |
| `projects` | list of quoted strings | the report's project names (repository names); for weekly and monthly notes, every project in the period |
| `active_minutes` | integer | active minutes; for weekly and monthly notes, the sum over the days |
| `cost_usd` | two decimal places | API-equivalent cost at public API prices. Absent when cost display is turned off in Settings |
| `generated` | RFC 3339, UTC | when the summary was made |
| `tags` | list | always `[porch]` |

Adding a field alone does not raise `porch_format`. Changing the meaning of a field, or removing one, does.

## Body

- At the top is a one-line callout saying the note is rewritten; below it is the same body as the app's Markdown export.
- A weekly note ends with links, under `## Daily notes`, to the days of that week that have a summary, like `[[2026-09-28]] · [[2026-09-29]]`.
- A monthly note ends with links, under `## Weekly notes`, to the weeks whose Monday falls in that month and that have a saved summary, like `[[2026-W40]]`.
- Links use the file name only; Obsidian finds it inside the vault.
- Headings and the callout follow the summary's language ([ADR 0011](../decisions/0011-english.md)). In Korean, the two headings are `## 일별 기록` and `## 주별 기록`.

## Rewriting

- Notes in the connected folder belong to porch. They are written anew every time a summary is saved or rewritten, edits included ([ADR 0009](../decisions/0009-porch-notes-are-rewritten.md)).
- The top of the body says so in an Obsidian callout: `> [!note] porch overwrites this note whenever the summary is saved or regenerated, replacing any edits made here. Keep your own notes in a separate note and link to this one.` (in Korean: `> [!note] porch가 요약을 만들 때마다 다시 쓰는 노트입니다. 메모는 다른 노트에 쓰고 이 노트를 링크하세요.`)
- A missing file (deleted by a person) is written again on the next save.
- If the path contains a link pointing outside the folder, nothing is written.
