import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { en } from "./i18n/en";
import { ko } from "./i18n/ko";

// Scene s9's still is porch's own note in Obsidian (video/obsidian.mjs); the phone card is typed by hand, so tie it
// to the note porch wrote for that still and to the code that writes notes.
const mirror = readFileSync(new URL("../../crates/core/src/mirror.rs", import.meta.url), "utf8");
const note = { en: readFileSync("video/hero/notes-en.md", "utf8"), ko: readFileSync("video/hero/notes-ko.md", "utf8") };
const HANGUL = /[ᄀ-ᇿㄱ-ㆎ가-힣]/;

/** Body of `fn name(...) { ... }` up to the next top-level `fn`/`pub fn`. */
const fnBody = (src: string, name: string) => {
  const start = src.search(new RegExp(`fn ${name}\\(`));
  expect(start, name).toBeGreaterThanOrEqual(0);
  const rest = src.slice(start + 1);
  const end = rest.search(/\n(pub )?fn /);
  return rest.slice(0, end < 0 ? undefined : end);
};

describe("scene s9 follows the app", () => {
  it("the phone card shows lines from the note porch wrote", () => {
    for (const [lang, t] of [["en", en], ["ko", ko]] as const) {
      const d = t.scenes.delivered;
      const md = note[lang];
      expect(md, lang).toContain(`# ${d.title}\n`);
      expect(md, lang).toContain(`\n${d.headline}\n`);
      expect(md, lang).toContain(`## ${d.section}\n`);
      for (const r of d.rows) expect(md, `${lang} ${r.task}`).toMatch(new RegExp(`\\| ${r.task} \\| [^|]+ \\| ${r.time} \\|`));
    }
  });

  it("the English note and card have no Korean", () => {
    expect(note.en).not.toMatch(HANGUL);
    expect(JSON.stringify(en.scenes.delivered)).not.toMatch(HANGUL);
  });

  it("weekly notes link days and monthly notes link weeks, as the copy says", () => {
    expect(fnBody(mirror, "week_note")).toContain('Lang::Ko => "일별 기록"');
    expect(fnBody(mirror, "week_note")).toContain("links(title");
    expect(fnBody(mirror, "month_note")).toContain('Lang::Ko => "주별 기록"');
    expect(fnBody(mirror, "month_note")).toContain("links(title");
  });

  it("the open file is a daily note for the demo day, beside last week's note", () => {
    for (const t of [en, ko]) {
      const d = t.scenes.delivered;
      expect(d.files[d.selected]).toBe("porch/daily/2026-09-29.md");
      // the week of the 29th is still under way, so its weekly note does not exist yet
      expect(d.files.some((f) => f.startsWith("porch/weekly/2026-W40"))).toBe(false);
    }
  });
});
