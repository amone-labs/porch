import type { FeatureId } from "../agents";
import type { Feature as CompareFeature } from "../compare";

export type Locale = "en" | "ko";
export type Dot = "bright" | "grey" | "ring";
export type StoryId = "works" | "time" | "stuck" | "delivered" | "suggest" | "usage";
export type SceneId = "s2" | "s4" | "s5" | "s6" | "s8" | "s9";

export interface Copy {
  meta: { title: string; description: string };
  nav: { how: string; privacy: string; faq: string; download: string; otherLang: string };
  hero: {
    eyebrow: string;
    headline: string[]; // may contain one _serif_ word (Latin only)
    subhead: string[];
    condition: string;
    cta: string;
    aside: string; // "macOS · Apple Silicon", small plain text under the button
    agentLink: string;
    caption: string; // under the hero video
    video: { label: string; play: string; pause: string };
  };
  mock: {
    date: string;
    title: string;
    stats: string; // fmt template: {time} {projects} {sessions} {requests} {commits}
    timeLabel: string;
    stuckLabel: string;
    tasks: string[];
    stuck: string;
    resolved: string;
    projects: string[];
  };
  story: { id: StoryId; scene: SceneId; eyebrow: string; headline: string; body: string[]; caption: string }[];
  shots: { uiNote: string }; // a note under app screenshots; empty since the app follows the system language
  scenes: {
    windows: { app: string; line: string }[];
    sessions: { title: string; state: string; dot: Dot; project: string }[];
    suggest: { title: string; kind: string; effect: string };
    carried: string[];
    limits: { name: string; pct: number; label: string; reset: string }[];
    // s9's phone card: the open note in the vault, as porch writes it (video/hero/notes-<lang>.md)
    delivered: {
      vault: string;
      files: string[];
      selected: number;
      title: string;
      headline: string;
      section: string;
      rows: { task: string; time: string }[];
    };
  };
  compare: {
    eyebrow: string;
    headline: string;
    feature: string; // first column head
    features: Record<CompareFeature, string>;
    legend: string;
    under: string;
    stamp: string; // followed by the source links
  };
  privacy: { eyebrow: string; headline: string; columns: { label: string; body: string }[]; under: string; site: string; source: string; sourceLink: string };
  setup: {
    eyebrow: string;
    headline: string;
    steps: string[];
    agentLabel: string;
    prompt: string;
    copy: string;
    copied: string;
    requirements: string;
  };
  faq: { headline: string; items: { q: string; a: string }[] };
  agents: { eyebrow: string; headline: string; featureHead: string; features: Record<FeatureId, string> };
  close: { summary: string; title: string; cta: string; aside: string };
  footer: { blurb: string; getStarted: string; privacy: string; faq: string; releaseNotes: string; cookies: string; policy: string; source: string };
  // the banner (ADR 0006): what is kept, what each button does, where to change it; the vendor is in the policy
  consent: { label: string; title: string; body: string; note: string; allow: string; decline: string };
  notFound: { title: string; body: string; home: string };
}
