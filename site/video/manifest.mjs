// Hash of everything the hero video and the story stills are made from. `pnpm build` runs --check, so changing
// an input (the app screens, the fixtures, the scene list) without `pnpm video` fails the build instead of
// shipping stale media. Usage: node video/manifest.mjs --write | --check
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const FILE = process.env.MANIFEST ?? resolve(ROOT, "site/video/manifest.json");

export const INPUTS = [
  "site/video/build-hero.mjs",
  "site/video/hero.template.html",
  "site/video/render.mjs",
  "site/video/stills.mjs",
  "site/video/build-works.mjs",
  "site/video/works.template.html",
  "site/video/works.css",
  "app/src-tauri/icons/tray.png",
  "site/video/obsidian.mjs",
  "site/video/shots.json",
  "site/video/hero/sizes.json",
  "site/video/app-capture/fixtures.ts",
  "site/video/app-capture/en.ts",
  "site/video/app-capture/mock.ts",
  "site/video/app-capture/shoot.mjs",
  "site/video/app-capture/vite.config.ts",
  "crates/core/src/mirror.rs",
  "crates/core/src/export.rs",
  "app/src/App.tsx",
  "app/src/i18n/index.ts",
  "app/src/i18n/ko.ts",
  "app/src/i18n/en.ts",
  "app/src/screens/Now.tsx",
  "app/src/screens/Popover.tsx",
  "app/src/screens/Projects.tsx",
  "app/src/screens/Reports.tsx",
  "app/src/screens/Usage.tsx",
  "app/src/components/DayView.tsx",
  "app/src/components/Insight.tsx",
  "app/src/components/OpenWork.tsx",
  "app/src/components/Health.tsx",
  "app/src/components/Suggestions.tsx",
  "app/src/components/ui.tsx",
  "app/src/format.ts",
  "app/src/index.css",
  "app/tailwind.config.js",
];

function hash() {
  const h = createHash("sha256");
  for (const p of INPUTS) {
    const file = resolve(ROOT, p);
    if (!existsSync(file)) throw new Error(`manifest input missing: ${p}`);
    h.update(p).update("\0").update(readFileSync(file)).update("\0");
  }
  return h.digest("hex");
}

const mode = process.argv[2];
if (mode === "--write") {
  writeFileSync(FILE, JSON.stringify({ inputs: hash() }, null, 2) + "\n");
  console.log("manifest written");
} else if (mode === "--check") {
  const saved = existsSync(FILE) ? JSON.parse(readFileSync(FILE, "utf8")).inputs : "";
  if (saved !== hash()) {
    console.error("Hero video and story stills are stale: their inputs changed. Run `pnpm capture` in one terminal, then `pnpm video`, and commit the results.");
    process.exit(1);
  }
} else {
  console.error("usage: node video/manifest.mjs --write | --check");
  process.exit(2);
}
