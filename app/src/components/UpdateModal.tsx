// The update prompt: what changed, then download progress, then restart. A
// failed install says so and offers the installer instead. Only its own
// buttons close it; the backdrop and Esc do nothing.
import type { ReactNode } from "react";
import { useJobs } from "../jobs";
import { useLang, useT } from "../i18n";
import { applyUpdate, dismissUpdate, noteItems, openDownload, useUpdate } from "../update";
import { Button } from "./ui";

/** Small label, title and one line of explanation: the top of every step. */
function Heading({ label, title, hint }: { label: ReactNode; title: string; hint: string }) {
  return (
    <header className="flex flex-col gap-2">
      {label}
      <h2 id="update-title" className="text-[19px] font-medium tracking-[-0.02em]">
        {title}
      </h2>
      <p className="text-muted">{hint}</p>
    </header>
  );
}

function Actions({ children }: { children: ReactNode }) {
  return <footer className="flex items-center justify-end gap-2 border-t border-line-soft px-6 py-4">{children}</footer>;
}

/** Download and restart: heading up top, the bar held to the bottom. */
function Busy({ heading, children }: { heading: ReactNode; children: ReactNode }) {
  return (
    <div className="flex min-h-[220px] flex-col justify-between p-6">
      {heading}
      {children}
    </div>
  );
}

/** Thin progress bar; with no known size a third of the track pulses. */
function Bar({ pct }: { pct: number | null }) {
  const known = pct !== null;
  return (
    <div className="h-1 overflow-hidden rounded-full bg-fg/10" aria-hidden>
      <div
        className={
          known
            ? "h-full rounded-full bg-accent motion-safe:transition-[width] motion-safe:duration-fast"
            : "h-full w-1/3 rounded-full bg-accent motion-safe:animate-pulse"
        }
        style={known ? { width: `${pct}%` } : undefined}
      />
    </div>
  );
}

export function UpdateModal() {
  const t = useT();
  const u = useUpdate();
  const jobs = useJobs();
  const lang = useLang();
  if (!u.open || u.status === "idle") return null;

  // English screens show the English notes, or the generic line when a release has none (never Korean notes).
  const notes = noteItems(lang === "en" ? u.notesEn : u.notes);
  const pct = u.progress === null ? null : Math.round(u.progress * 100);
  const version = <span className="font-mono text-[11px] text-faint">v{u.version}</span>;

  let step: ReactNode;
  switch (u.status) {
    case "available":
      step = (
        <>
          <div className="border-b border-line-soft px-6 pb-5 pt-6">
            <Heading label={version} title={t.update.available} hint={t.update.installHint} />
          </div>
          <section className="flex flex-col gap-2.5 px-6 py-5">
            <span className="label-ko">{t.update.whatChanged}</span>
            {notes.items.length === 0 && <p className="text-muted">{notes.rest ?? t.update.smallFixes}</p>}
            {notes.items.length > 0 && (
              <>
                <ul className="flex list-disc flex-col gap-1.5 pl-4 leading-relaxed marker:text-faint">
                  {notes.items.map((i) => (
                    <li key={i}>{i}</li>
                  ))}
                </ul>
                {notes.rest && <p className="text-[12px] text-muted">{notes.rest}</p>}
              </>
            )}
            {jobs.length > 0 && <p className="text-[12px] text-muted">{t.update.summaryRunning}</p>}
          </section>
          <Actions>
            <Button onClick={dismissUpdate}>{t.update.later}</Button>
            <Button primary onClick={() => void applyUpdate()}>
              {t.update.installNow}
            </Button>
          </Actions>
        </>
      );
      break;
    case "working":
      step = (
        <Busy heading={<Heading label={version} title={t.update.downloading} hint={t.update.downloadingHint} />}>
          <div className="flex flex-col gap-2.5">
            <Bar pct={pct} />
            {pct !== null && (
              <span role="status" className="self-end font-mono text-[11.5px] text-muted">
                {pct}%
              </span>
            )}
          </div>
        </Busy>
      );
      break;
    case "restarting":
      step = (
        <Busy
          heading={
            <Heading label={<span className="label-ko">{t.update.installed}</span>} title={t.update.restarting} hint={t.update.restartingHint} />
          }
        >
          <div className="h-1 rounded-full bg-accent" aria-hidden />
        </Busy>
      );
      break;
    case "failed":
      step = (
        <>
          <div className="p-6">
            <Heading label={<span className="label-ko">{t.update.eyebrow}</span>} title={t.update.failed} hint={t.update.failedHint} />
          </div>
          <Actions>
            <Button onClick={dismissUpdate}>{t.update.close}</Button>
            <Button primary onClick={() => void openDownload()}>
              {t.update.downloadYourself}
            </Button>
          </Actions>
        </>
      );
      break;
  }

  return (
    <div className="fixed inset-0 z-30 flex items-center justify-center bg-black/30 backdrop-blur-[2px] print:hidden">
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="update-title"
        className="w-[440px] max-w-[calc(100vw-32px)] overflow-hidden rounded-xl bg-canvas shadow-2xl"
      >
        {step}
      </div>
    </div>
  );
}
