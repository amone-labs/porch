import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { en } from "./en";
import { ko } from "./ko";

function shape(v: unknown): unknown {
  if (Array.isArray(v)) return v.map(shape);
  if (v && typeof v === "object") return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, shape(x)]));
  return typeof v;
}

/** The app's Korean state labels, read straight from its dictionary so the mockup can't drift from it. */
function appLabels(): string[] {
  const src = readFileSync(new URL("../../../app/src/i18n/ko.ts", import.meta.url), "utf8");
  const start = src.indexOf("\n  state: {");
  const block = src.slice(start, src.indexOf("\n  },", start));
  return [...block.matchAll(/:\s*"([^"]+)"/g)].map((m) => m[1]);
}

/** The app's Korean screen text: its dictionary, where the summary components take their labels from. */
function appSummaryText(): string {
  return readFileSync(new URL("../../../app/src/i18n/ko.ts", import.meta.url), "utf8");
}

describe("copy", () => {
  it("has the same keys and array lengths in both languages", () => {
    expect(shape(en)).toEqual(shape(ko));
  });
  it("uses the app's state labels in the Korean session list", () => {
    const labels = appLabels();
    expect(labels).toEqual(expect.arrayContaining(["허락 필요", "완료", "작업 중"]));
    for (const row of ko.scenes.sessions) {
      expect(labels.some((l) => row.state.startsWith(l)), row.state).toBe(true);
    }
  });
  it("uses the app's summary section labels in the Korean mockup", () => {
    const app = appSummaryText();
    for (const label of [ko.mock.timeLabel, ko.mock.stuckLabel]) {
      expect(app.includes(`"${label}"`), label).toBe(true);
    }
    for (const item of ko.scenes.carried) {
      const kind = item.split(" · ")[0];
      expect(app.includes(`"${kind}"`), kind).toBe(true);
    }
  });
  it("builds the meta description from the agent list", () => {
    expect(en.meta.description).toContain("{agents}");
    expect(ko.meta.description).toContain("{agents:로}");
  });
  it("has six story blocks in order, each mapped to its scene", () => {
    for (const t of [en, ko]) {
      expect(t.story.map((b) => b.id)).toEqual(["works", "time", "stuck", "delivered", "suggest", "usage"]);
      expect(t.story.map((b) => b.scene)).toEqual(["s2", "s4", "s5", "s9", "s8", "s6"]);
    }
  });
  it("keeps each block short: one-line hero subhead, one paragraph per story block", () => {
    for (const t of [en, ko]) expect(t.hero.subhead).toHaveLength(1);
    for (const b of en.story) {
      expect(b.headline.split(/\s+/).length, b.headline).toBeLessThanOrEqual(8);
      expect(b.body, b.id).toHaveLength(1);
      expect(b.body[0].split(/\s+/).length, b.id).toBeLessThanOrEqual(45);
    }
    for (const b of ko.story) {
      expect(b.body, b.id).toHaveLength(1);
      expect(b.body[0].length, b.id).toBeLessThanOrEqual(120);
    }
    for (const t of [en, ko]) for (const b of t.story) expect(b.caption, b.id).not.toMatch(/^(예시 화면입니다|Example screen)/);
  });
  it("keeps qualifiers beside the claims they limit (block text or its caption)", () => {
    const near = (t: typeof ko, id: string) => {
      const b = t.story.find((x) => x.id === id)!;
      return b.body.join(" ") + " " + b.caption;
    };
    expect(near(ko, "works")).toContain("클로드코드만");
    expect(near(en, "works")).toContain("Only Claude Code");
    expect(near(ko, "time")).toContain("추정");
    expect(near(en, "time")).toContain("Estimate");
    expect(near(ko, "usage")).toContain("Pro·Max");
    expect(near(ko, "usage")).toContain("마지막 값");
    expect(near(en, "usage")).toContain("last values");
    expect(near(ko, "stuck")).toContain("클로드코드나 코덱스가 씁니다");
    expect(ko.hero.condition).toContain("클로드코드나 코덱스 로그인");
    expect(en.hero.condition).toContain("Claude Code or Codex signed in");
    expect(ko.hero.caption).toContain("추정");
  });
  it("uses the app's limit wording", () => {
    expect(ko.scenes.limits[0].reset).toMatch(/에 다시 참$/);
  });
  it("never puts Hangul inside a serif marker", () => {
    const all = JSON.stringify(ko) + JSON.stringify(en);
    // Stop at quotes/braces so the note data's `porch_format`/`active_minutes` underscores never pair up.
    for (const m of all.matchAll(/_([^_"{}]+)_/g)) expect(m[1]).not.toMatch(/[가-힣]/);
  });
  it("privacy says what suggestions send and that usage statistics turn off in Settings (ADR 0006, 0007)", () => {
    for (const t of [en, ko]) {
      const sends = t.privacy.columns[2].body;
      expect(sends).toContain("CLAUDE.md");
      expect(sends).toContain("AGENTS.md");
    }
    expect(ko.privacy.columns[2].body).toContain("설정");
    expect(en.privacy.columns[2].body).toContain("Settings");
    const leaves = (t: typeof ko) => t.faq.items[2].a;
    expect(leaves(ko)).toContain("CLAUDE.md");
    expect(leaves(en)).toContain("CLAUDE.md");
  });
  it("says the site keeps visit statistics and where to change the cookie choice (ADR 0006)", () => {
    for (const t of [en, ko]) {
      expect(t.privacy.site).toContain(t.footer.cookies);
      expect(t.consent.note).toContain(t.footer.cookies);
    }
    // the banner says what each choice does: a cookie only when allowed, counting without one otherwise
    expect(ko.consent.body).toContain("허용하면");
    expect(ko.consent.body).toContain("고르기 전");
    expect(en.consent.body).toContain("Before you choose");
    expect(ko.consent.body).toContain("쿠키 없이");
    expect(en.consent.body).toContain("without a cookie");
    for (const t of [en, ko]) expect(JSON.stringify(t.consent)).not.toMatch(/PostHog/);
  });
  it("story runs works → time → stuck → delivered → suggest → usage", () => {
    for (const t of [en, ko]) expect(t.story.map((b) => b.id)).toEqual(["works", "time", "stuck", "delivered", "suggest", "usage"]);
    expect(en.story.find((b) => b.id === "suggest")!.scene).toBe("s8");
  });
  it("suggestions block states who proposes, who applies, and that the comparison is not a cause (ADR 0007)", () => {
    const k = ko.story.find((b) => b.id === "suggest")!.body.join(" ");
    expect(k).toContain("클로드코드나 코덱스가");
    expect(k).toContain("설정 파일을 고치지 않습니다");
    expect(k).toContain("원인");
    const e = en.story.find((b) => b.id === "suggest")!.body.join(" ");
    expect(e).toContain("Claude Code or Codex proposes");
    expect(e).toContain("never edits your config files");
    expect(e).toContain("cause");
  });
  it("English page says the app UI is shown in English; Korean page needs no note", () => {
    expect(en.shots.uiNote).toBe("");
    expect(ko.shots.uiNote).toBe("");
  });
  it("scene examples use the capture fixtures' projects", () => {
    for (const t of [en, ko]) {
      expect(t.mock.projects).toEqual(["shop-web", "shop-api", "porch-site"]);
    }
  });
  it("says the summary agent is picked in Settings, never switched by itself (ADR 0010)", () => {
    expect(en.privacy.columns[2].body).toContain("whichever you pick in Settings");
    expect(ko.privacy.columns[2].body).toContain("설정에서 고른");
    for (const t of [en, ko]) expect(JSON.stringify(t)).not.toMatch(/automatically switch|falls? back to|자동으로 (바꿉|넘어)/);
  });
  it("never uses check marks in tables", () => {
    expect(JSON.stringify(en) + JSON.stringify(ko)).not.toMatch(/[✓✔]/);
  });
});
