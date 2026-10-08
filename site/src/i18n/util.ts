import { agentNames, joinNames, josa } from "../agents";
import type { Locale } from "./types";

export function fmt(template: string, vars: Record<string, string | number>): string {
  return template.replace(/\{(\w+)\}/g, (_, k: string) => {
    if (!(k in vars)) throw new Error(`fmt: no value for {${k}} in "${template}"`);
    return String(vars[k]);
  });
}

/** Expands {agents}, {agents:dot}, {agents:amp}, {agents:로|를|와} from the agent list. */
export function fill(template: string, locale: Locale): string {
  const names = agentNames(locale);
  return template.replace(/\{agents(?::([^}]+))?\}/g, (_, mod?: string) => {
    if (!mod) return joinNames(names, locale);
    if (mod === "dot") return names.join("·");
    if (mod === "amp") return names.join(" & ");
    if (mod === "로" || mod === "를" || mod === "와") return josa(joinNames(names, locale), mod);
    throw new Error(`fill: unknown modifier ${mod}`);
  });
}

export function emph(text: string): string {
  const escaped = text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  return escaped
    .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
    .replace(/\[\[(.+?)\]\]/g, '<mark class="chip">$1</mark>')
    .replace(/(^|[\s(])_(.+?)_(?=[\s.,;:!?)]|$)/g, '$1<em class="serif">$2</em>');
}

export function labelClass(locale: Locale): "eyebrow" | "eyebrow-ko" {
  return locale === "ko" ? "eyebrow-ko" : "eyebrow";
}
