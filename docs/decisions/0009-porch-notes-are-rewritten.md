# 0009. porch rewrites the notes in its folder

- Status: accepted
- Date: 2026-10-03
- Supersedes: "notes a person edited are not overwritten" in [0008](0008-mirror-to-a-chosen-folder.md)

## Context

0008 did not overwrite notes a person had edited after porch wrote them. In use, pressing "Rewrite" left the note as it was, so porch and the note drifted apart and it was not visible why.

People edit a note for one of two reasons: to correct the AI summary, or to add their own thoughts. For the first, the rewritten summary is usually right; the second must not be erased. One rule that handles both would need separate areas inside the note, and the writer would have to know the rule.

## Decision

- Notes in the connected folder (usually `<vault>/porch`) belong to porch. They are written anew every time a summary is saved or rewritten, edits included.
- A person's own notes go in another note (a daily note, for example) and link to porch's, like `[[2026-W40]]`.
- This rule is stated at the top of each note body and on the connection screen.
- The hash that recognised edited notes is no longer used. `mirror.json` keeps only, per folder, the list of notes porch wrote (used for the notice about an earlier folder).
- If feedback asks for it, a porch area inside the note (rewriting only between markers) gets another look.

## Consequences

- One rule: Rewrite means the note gets the new summary too.
- Notes written into a porch note by someone who missed the notice disappear on the next save. The notice at the top of the note lowers that risk.
