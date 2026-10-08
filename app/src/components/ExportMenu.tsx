import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import { notify } from "../notice";
import { track } from "../analytics";
import { setPaper } from "../paper";
import { Icon } from "./Icon";
import { useT } from "../i18n";

type Format = "md" | "pdf";

const ITEMS: { id: Format; label: string }[] = [
  { id: "pdf", label: "PDF" },
  { id: "md", label: "Markdown" },
];

/** Download icon that opens a small menu of formats; the file lands in Downloads. */
/** Off until there is a summary: without one there is nothing worth keeping. */
export function ExportMenu({ kind, date, disabled = false }: { kind: "day" | "week" | "month"; date: string; disabled?: boolean }) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const [working, setWorking] = useState<Format | null>(null);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const run = async (f: Format) => {
    setOpen(false);
    setWorking(f);
    // Let the menu close before the page is laid out for paper.
    await new Promise((r) => requestAnimationFrame(() => r(null)));
    // Paper has no clicks: unfold the commit and session lists while printing.
    const folded = f === "pdf" ? [...document.querySelectorAll<HTMLDetailsElement>("main details:not([open])")] : [];
    folded.forEach((d) => (d.open = true));
    if (f === "pdf") await setPaper(true);
    try {
      const path = await (f === "pdf" ? api.exportPdf(kind, date) : api.exportMarkdown(kind, date));
      const name = path.split("/").pop() ?? path;
      track("report_exported", { kind, format: f, ok: true });
      notify({ text: t.summary.exportSaved(name), action: { label: t.summary.showInFinder, run: () => api.reveal(path) } });
    } catch (e) {
      track("report_exported", { kind, format: f, ok: false });
      notify({ text: t.summary.exportFailed(String(e)) });
    } finally {
      folded.forEach((d) => (d.open = false));
      if (f === "pdf") void setPaper(false);
      setWorking(null);
    }
  };

  return (
    <div ref={root} className="relative print:hidden">
      <button
        type="button"
        aria-label={t.summary.export}
        aria-haspopup="menu"
        aria-expanded={open}
        title={disabled ? t.summary.exportDisabled : t.summary.export}
        disabled={disabled || working !== null}
        onClick={() => setOpen((o) => !o)}
        className={
          "inline-flex h-7 w-7 items-center justify-center rounded-md border border-line text-muted motion-safe:transition-colors motion-safe:duration-fast motion-safe:ease-calm hover:bg-fg/5 hover:text-fg disabled:cursor-default disabled:opacity-40 disabled:hover:bg-transparent disabled:hover:text-muted " +
          (open ? "bg-fg/10 text-fg" : "")
        }
      >
        <Icon name="download" size={14} />
      </button>
      {open && (
        <div role="menu" className="absolute right-0 top-9 z-20 w-44 rounded-lg border border-line bg-elevated p-1 shadow-lg">
          {ITEMS.map((it) => (
            <button
              key={it.id}
              type="button"
              role="menuitem"
              onClick={() => run(it.id)}
              className="flex w-full items-center rounded-md px-2.5 py-1.5 text-left text-[12.5px] text-fg/90 hover:bg-fg/5"
            >
              {it.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
