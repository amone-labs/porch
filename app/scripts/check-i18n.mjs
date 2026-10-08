// Hangul left in screens means a string that skipped the dictionary; Hangul in en.ts
// means an untranslated value. Comments are ignored. Run: pnpm check:i18n [files...]
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative, resolve } from "node:path";

const HANGUL = /[ᄀ-ᇿ㄰-㆏가-힣]/;
const root = new URL("../src", import.meta.url).pathname;
const walk = (d) => readdirSync(d).flatMap((f) => (statSync(join(d, f)).isDirectory() ? walk(join(d, f)) : [join(d, f)]));
const only = process.argv.slice(2);
// Excluded by their exact path under src, not by suffix, so a screen named e.g.
// "my-format.ts" is still checked:
//   format.ts         is not a screen: its Korean lives in the `ko` branches of its language functions.
//   i18n/endonyms.ts  is not a screen: each language is named in its own language there.
//   analytics.ts      is not a screen: its Korean lives in the regexes that classify writer errors in both languages.
//   i18n/ko.ts        is the Korean dictionary itself.
const SKIP = new Set(["i18n/ko.ts", "format.ts", "i18n/endonyms.ts", "analytics.ts"]);
const underSrc = (f) => relative(root, resolve(f)).split("\\").join("/");
const files = (only.length ? only : walk(root)).filter((f) => /\.(ts|tsx)$/.test(f) && !SKIP.has(underSrc(f)));

let bad = 0;
for (const f of files) {
  let inBlock = false;
  readFileSync(f, "utf8").split("\n").forEach((line, i) => {
    let code = line;
    if (inBlock) {
      const end = code.indexOf("*/");
      if (end < 0) return;
      code = code.slice(end + 2);
      inBlock = false;
    }
    code = code.replace(/\/\*.*?\*\//g, "");
    const open = code.indexOf("/*");
    if (open >= 0) {
      code = code.slice(0, open);
      inBlock = true;
    }
    code = code.replace(/(^|[^:])\/\/.*$/, "$1");
    if (HANGUL.test(code)) {
      bad++;
      console.log(`${f.replace(root + "/", "")}:${i + 1}: ${line.trim()}`);
    }
  });
}
if (bad) {
  console.log(`${bad} line(s) with Hangul outside i18n/ko.ts`);
  process.exit(1);
}
