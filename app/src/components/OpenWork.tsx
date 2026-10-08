// Work left open across days. Pending: in progress or next up. Blocker: a
// blocker that was not resolved. Summaries open and close these; "Close manually" closes one by hand.
import { useEffect, useState } from "react";
import { api, type OpenItem } from "../api";
import { useJobs } from "../jobs";
import { Section } from "./ui";
import { useT } from "../i18n";

function short(date: string) {
  const [, m, d] = date.split("-").map(Number);
  return `${m}/${d}`;
}

/** How an item was closed (open.rs `closed_label`). */
function closedHow(how: string | null, map: Record<string, string>): string {
  return map[how ?? "done"] ?? how ?? "";
}

export function KindTag({ kind }: { kind: OpenItem["kind"] }) {
  const t = useT();
  return (
    <span
      className={
        "inline-flex h-[18px] shrink-0 items-center rounded border px-1.5 text-[10.5px] " +
        (kind === "issue" ? "border-muted text-fg/90" : "border-line text-faint")
      }
    >
      {kind === "issue" ? t.openWork.blocker : t.openWork.pending}
    </span>
  );
}

/** The ledger, reloaded when a summary finishes. */
export function useOpenItems() {
  const [items, setItems] = useState<OpenItem[] | null>(null);
  const jobs = useJobs();
  useEffect(() => {
    api.openItems().then(setItems, () => setItems([]));
  }, [jobs.length]);
  const close = (id: string) => api.closeOpenItem(id).then(setItems);
  return { items, close };
}

/** Everything still open, by project, issues first. */
export function OpenWork() {
  const t = useT();
  const { items, close } = useOpenItems();
  const open = (items ?? []).filter((i) => !i.closed_on);
  if (!items) return null;
  const byProject = new Map<string, OpenItem[]>();
  for (const i of open) byProject.set(i.project, [...(byProject.get(i.project) ?? []), i]);
  const issues = open.filter((i) => i.kind === "issue").length;

  return (
    <Section
      title={t.openWork.title}
      aside={
        <span className="text-[11px] text-faint">{t.openWork.aside(open.length - issues, issues)}</span>
      }
    >
      {open.length === 0 ? (
        <p className="py-3 text-muted">{t.openWork.empty}</p>
      ) : (
        <div className="divide-y divide-line-soft">
          {[...byProject.entries()].map(([project, list]) => (
            <div key={project} className="py-3">
              <h3 className="mb-1.5 text-[12.5px] font-medium">{project}</h3>
              <ul>
                {[...list]
                  .sort((a, b) => (a.kind === b.kind ? a.since.localeCompare(b.since) : a.kind === "issue" ? -1 : 1))
                  .map((i) => (
                    <li key={i.id} className="group flex min-h-8 items-center gap-2.5 rounded-md px-1 hover:bg-fg/[0.04]">
                      <KindTag kind={i.kind} />
                      <span className="min-w-0 flex-1 text-[12.5px] text-fg/90">{i.text}</span>
                      <span className="shrink-0 text-[11px] text-faint">{t.openWork.firstSeen(short(i.since))}</span>
                      <button
                        type="button"
                        onClick={() => close(i.id)}
                        className="h-6 shrink-0 rounded-md border border-line px-2 text-[11px] text-muted opacity-0 hover:bg-fg/5 hover:text-fg focus:opacity-100 group-hover:opacity-100"
                      >
                        {t.openWork.closeManually}
                      </button>
                    </li>
                  ))}
              </ul>
            </div>
          ))}
        </div>
      )}
      <p className="mt-2 text-[11.5px] text-faint">{t.openWork.note}</p>
    </Section>
  );
}

/** On a day: what it closed from earlier days and what it left open. */
export function DayOpenWork({ date }: { date: string }) {
  const t = useT();
  const { items } = useOpenItems();
  if (!items) return null;
  const closed = items.filter((i) => i.closed_on === date && i.closed_how !== "manual");
  const opened = items.filter((i) => i.since === date);
  if (closed.length === 0 && opened.length === 0) return null;
  return (
    <Section title={t.openWork.carried}>
      <ul className="space-y-1">
        {closed.map((i) => (
          <li key={i.id} className="flex items-center gap-2.5 text-[12.5px]">
            <span className="inline-flex h-[18px] shrink-0 items-center rounded border border-line px-1.5 text-[10.5px] text-faint">
              {closedHow(i.closed_how, t.openWork.closedHow)}
            </span>
            <span className="min-w-0 flex-1 text-muted line-through decoration-faint">{i.text}</span>
            <span className="shrink-0 text-[11px] text-faint">{t.openWork.firstSeen(short(i.since))}</span>
          </li>
        ))}
        {opened.map((i) => (
          <li key={i.id} className="flex items-center gap-2.5 text-[12.5px]">
            <KindTag kind={i.kind} />
            <span className={"min-w-0 flex-1 " + (i.closed_on ? "text-muted line-through decoration-faint" : "text-fg/90")}>{i.text}</span>
            <span className="shrink-0 text-[11px] text-faint">{i.closed_on ? `${short(i.closed_on)} ${closedHow(i.closed_how, t.openWork.closedHow)}` : i.project}</span>
          </li>
        ))}
      </ul>
    </Section>
  );
}
