import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { englishOverlay } from "./en";
import { fixtures } from "./fixtures";

// The zoom badge is a desktop affordance; it has no place in a recording.
const style = document.createElement("style");
style.textContent = "button.fixed.bottom-3.right-3 { display: none !important; }";
document.head.append(style);

const params = new URLSearchParams(location.search);
const label = params.get("window") ?? "main";
// The app draws its own labels from its dictionaries (app/src/i18n); ?lang=en picks English.
const lang = params.get("lang") === "en" ? "en" : "ko";
mockWindows(label);
const seen = new Set<string>();
mockIPC((cmd, args) => {
  if (cmd === "get_language") return lang;
  if (cmd in fixtures) {
    const out = fixtures[cmd](args) as { summary?: object | null } | null;
    // The fixture summaries are written in Korean and shown translated in English; mark them as
    // written in the screen's language so the app does not add its "written in Korean" note.
    return out?.summary ? { ...out, summary: { ...out.summary, lang } } : out;
  }
  if (!seen.has(cmd)) {
    seen.add(cmd);
    console.warn("[capture] no fixture for", cmd, JSON.stringify(args ?? {}).slice(0, 200));
  }
  return null;
});

// ?lang=en: the fixtures' own text (summaries, task names, blockers) in English.
if (lang === "en") {
  document.documentElement.lang = "en";
  englishOverlay(document.body);
}
