# 0011. Korean or English, with one set of summary instructions per language

- Status: accepted
- Date: 2026-10-05
- Revised: 2026-10-08, every CLI command's human-readable output follows the language, not only the summary commands. `--json` output is unchanged.

## Context

porch's screens, notifications, exports and summaries all came out in Korean only. The instructions sent to the summary model (the system prompt) were Korean too and said "write in plain Korean", so an English speaker who installed porch could not read their first summary.

## Decision

- Two languages: Korean and English. The setting is `system` (the default), `ko` or `en`. With `system`, porch uses Korean when the first of macOS's preferred languages is Korean, and English otherwise. The app, the CLI and automatic summaries follow the same setting.
- Summaries (day, week, month), suggestions and blocker labelling are written in the app's language. There is no separate setting for the summary language.
- The instructions sent to the model, and the labels porch adds to a day's records ("Request:", "Failed:", "Usual", "Open items" and the like), exist as one set per language. In English the model sees only English (requests and commit messages people wrote stay in their original language).
  - Rejected: adding "write in English" to the Korean instructions. One set would be easier to edit, but Korean instructions and labels could leak into English text. Separate sets per language carry less risk (maintainer's decision, 2026-10-05).
- A summary is saved with the language it was written in (`lang`). An older summary without it is read as Korean. When the text is kept rather than rewritten (only the numbers refreshed, or a failed rewrite), its language stays too. A summary in a language other than the app's is shown as it is, with a line saying it was written in another language and a Rewrite button. It is never rewritten automatically (that spends tokens).
- Exports (PDF and Markdown) and Obsidian notes take their titles and labels from that summary's `lang`. One note never mixes languages.
- ~~The CLI follows the language only in the human-readable output of the summary commands (`today`, `week`, `month`, `suggest`, `suggestions`). Other commands and `--json` output stay as they are.~~ Revised 2026-10-08: the human-readable output of every CLI command follows the language. `--json` output is unchanged.

## Consequences

- The app's screens follow the same setting. Screen text lives in the Korean and English dictionaries in `app/src/i18n/`; the menu bar, notifications and errors are in `Shell` in `lib.rs`.
- Every change to the instructions has to be made in both sets. Tests check that both sets have the same output field names, blocker kinds and suggestion kinds, and that the English set contains no Hangul. New instructions (the weekly insight, for example) start with both sets.
- Pending items, blockers and suggestion text already saved show in the language they were first written in. They are not translated.
- Week and month summaries made after a language change may read day summaries in both languages. The result comes out in the instructions' language.
- Values written to saved files are codes tied to no language and are translated only on screen. Korean values in older files are turned into codes when read.
- Places that recognised the kind of error (usage limit, login, agent missing) from the error sentence now use a kind value independent of language. The events and property values in ADR 0006 do not change.
- No other language is added. The structure leaves room for more, but only two sets are maintained now.
- Before adding a third language, porch will not add a whole set of instructions per language. The approach to look at first, and to settle in a new ADR: keep the Korean instructions as they are, and for every other language use the English instructions plus "write in <language>" and a small per-language bundle (title length and style, fixed phrases, examples). Whether Korean also moves onto the English instructions is decided then, by comparing the two approaches on real records (maintainer's decision, 2026-10-05: two sets for now).
