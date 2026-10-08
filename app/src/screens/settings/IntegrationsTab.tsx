import { useEffect, useState, type ReactNode } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { api, type SettingsView, type Vault } from "../../api";
import { ago, homeRelative } from "../../format";
import { Button, Disclosure } from "../../components/ui";
import { useT } from "../../i18n";
import { Row, field, type TabProps } from "./Row";
import { track } from "../../analytics";

/** Mono only for Latin and digits; a path with Hangul stays in the sans stack. */
const mono = (text: string) => (/[\u3131-\uD79D]/.test(text) ? "" : " font-mono");

function Dot({ on }: { on: boolean }) {
  return (
    <span
      aria-hidden
      className={"inline-block h-2 w-2 shrink-0 translate-y-[-1px] rounded-full " + (on ? "bg-accent" : "border border-faint")}
    />
  );
}

/** Where notes go now: an Obsidian vault, a plain folder, or nowhere. */
function target(v: SettingsView): "vault" | "folder" | null {
  if (!v.settings.mirror_dir) return null;
  return v.settings.obsidian_vault ? "vault" : "folder";
}

/** Integrations planned next, in the order of the 2026-10-06 analysis (developer share). Names stay in English. */
const SOON = ["Notion", "Google Docs"];

/** The last part of a path, for the status line. */
const baseName = (p: string) => p.split("/").filter(Boolean).pop() ?? p;

/** Save the folder settings, keeping what is not given. */
function saveMirror(v: SettingsView, dir: string | null, vault: string | null, subs: Partial<Record<"mirror_daily" | "mirror_weekly" | "mirror_monthly", string>> = {}) {
  const s = { ...v.settings, ...subs };
  return api.saveMirror(dir, vault, s.mirror_daily, s.mirror_weekly, s.mirror_monthly, s.notify);
}

async function pickFolder(): Promise<string | null> {
  const dir = await open({ directory: true });
  return typeof dir === "string" ? dir : null;
}

/** Progress of writing every saved summary into the folder, while it runs. */
function useMirrorProgress(busy: boolean): { done: number; total: number } | null {
  const [p, setP] = useState<{ done: number; total: number } | null>(null);
  useEffect(() => {
    const un = listen<{ done: number; total: number }>("mirror-progress", (e) => setP(e.payload));
    return () => {
      un.then((f) => f());
    };
  }, []);
  useEffect(() => {
    if (!busy) setP(null);
  }, [busy]);
  return busy ? p : null;
}

function Progress({ done, total }: { done: number; total: number }) {
  const t = useT();
  return (
    <div className="mt-3" role="status" aria-live="polite">
      <p className="text-[11.5px] text-muted">
        {t.settings.savingNotes} <span className="tabular-nums">{done}/{total}</span>
      </p>
      <div className="mt-1 h-0.5 w-56 overflow-hidden rounded-full bg-fg/10">
        <div className="h-full bg-fg/40 motion-safe:transition-[width] motion-safe:duration-fast" style={{ width: `${total ? (done / total) * 100 : 0}%` }} />
      </div>
    </div>
  );
}

export function IntegrationsTab(props: TabProps) {
  const { v } = props;
  const mode = target(v);
  const t = useT();
  const missing = v.mirror.vault_missing;
  const dir = v.settings.mirror_dir ?? "";
  const sub =
    mode === null ? t.settings.notConnected : missing ? t.settings.vaultMissing : mode === "vault" ? t.settings.connectedTo(v.mirror.vault?.name ?? "") : t.settings.savedToFolder(baseName(dir));
  // One integration for now. The next ones are listed as coming soon and cannot be clicked:
  // no controls for something that does not exist yet.
  return (
    <div className="mt-10 grid grid-cols-[14rem_1fr] gap-8">
      <div>
        <ul className="space-y-px">
          <li>
            <button
              type="button"
              aria-current="true"
              className="flex w-full items-start gap-2.5 rounded-md bg-fg/10 px-2.5 py-2 text-left text-fg"
            >
              <span className="pt-1.5">
                <Dot on={mode !== null && !missing} />
              </span>
              <span className="min-w-0">
                <span className="block">Obsidian</span>
                <span className="block truncate text-[11.5px] text-faint">{sub}</span>
              </span>
            </button>
          </li>
        </ul>
        <p id="integrations-soon" className="mt-5 px-2.5 text-[11.5px] text-faint">
          {t.settings.comingSoon}
        </p>
        <ul aria-labelledby="integrations-soon" className="mt-1 space-y-px">
          {SOON.map((name) => (
            <li key={name} className="flex items-start gap-2.5 px-2.5 py-2 text-muted">
              <span className="pt-1.5">
                <Dot on={false} />
              </span>
              <span className="min-w-0 flex-1">{name}</span>
              <span className="text-[11.5px] text-faint">{t.settings.soon}</span>
            </li>
          ))}
        </ul>
      </div>
      <ObsidianDetail {...props} />
    </div>
  );
}

function ObsidianDetail(props: TabProps) {
  const { v, busy, run } = props;
  const mode = target(v);
  const t = useT();
  const [repick, setRepick] = useState(false);
  const [justConnected, setJustConnected] = useState(false);
  const progress = useMirrorProgress(busy);
  const connect = (dir: string, vault: string | null) => {
    setJustConnected(true);
    setRepick(false);
    run(saveMirror(v, dir, vault).then((next) => {
      track("mirror_connected", { target: vault ? "obsidian" : "folder" });
      return next;
    }));
  };
  const toFolder = async () => {
    const dir = await pickFolder();
    if (dir) connect(dir, null);
  };

  if (mode === null || (mode === "vault" && v.mirror.vault_missing) || repick) {
    return (
      <div className="min-w-0">
        <h2 className="text-[15px] font-medium">Obsidian</h2>
        <p className="mt-2 text-muted">{t.settings.obsidianIntro}</p>
        {mode === "vault" && v.mirror.vault_missing && !repick && (
          <p className="mt-2 text-muted">{t.settings.vaultGone}</p>
        )}
        <VaultPicker
          busy={busy}
          onConnect={(vault) => connect(`${vault.path}/porch`, vault.id)}
          onFolder={toFolder}
          onCancel={repick ? () => setRepick(false) : undefined}
        />
        {progress && progress.total > 0 && <Progress {...progress} />}
      </div>
    );
  }

  if (mode === "folder") {
    return (
      <Linked
        {...props}
        title="Obsidian"
        place={t.settings.folderPlace(baseName(v.settings.mirror_dir ?? ""))}
        justConnected={justConnected}
        open={{ label: t.settings.openInFinder, run: () => api.openMirrorDir() }}
        onRepick={() => setRepick(true)}
        note={t.settings.notVaultNote}
      />
    );
  }

  return (
    <Linked
      {...props}
      title="Obsidian"
      place={v.mirror.vault?.name ?? ""}
      justConnected={justConnected}
      open={{ label: t.settings.openInObsidian, run: () => api.openVault() }}
      onRepick={() => setRepick(true)}
    >
      <p className="mt-4 text-[12px] text-muted">{t.settings.notifyLocationNote}</p>
    </Linked>
  );
}

function VaultPicker({
  busy,
  onConnect,
  onFolder,
  onCancel,
}: {
  busy: boolean;
  onConnect: (v: Vault) => void;
  onFolder: () => void;
  onCancel?: () => void;
}) {
  const [vaults, setVaults] = useState<Vault[] | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [chosen, setChosen] = useState<string | null>(null);
  const t = useT();
  useEffect(() => {
    api.mirrorVaults().then(
      (r) => {
        setVaults(r.vaults);
        setNote(r.note);
        if (r.vaults.length === 1) setChosen(r.vaults[0].id);
      },
      (e) => setNote(String(e)),
    );
  }, []);
  const vault = vaults?.find((x) => x.id === chosen);

  return (
    <div className="mt-6">
      {vaults === null && !note && <p className="text-muted">{t.settings.findingVaults}</p>}
      {vaults && vaults.length > 0 && (
        <ul role="radiogroup" aria-label={t.settings.obsidianVaultsAria} className="space-y-px">
          {vaults.map((x) => (
            <li key={x.id}>
              <button
                type="button"
                role="radio"
                aria-checked={chosen === x.id}
                onClick={() => setChosen(x.id)}
                className={
                  "flex w-full items-start gap-2.5 rounded-md px-2.5 py-2 text-left motion-safe:transition-colors motion-safe:duration-fast " +
                  (chosen === x.id ? "bg-fg/10 text-fg" : "text-muted hover:bg-fg/5 hover:text-fg")
                }
              >
                <span className="pt-1.5">
                  <Dot on={chosen === x.id} />
                </span>
                <span className="min-w-0">
                  <span className="block">{x.name}</span>
                  <span className={"block truncate text-[11.5px] text-faint" + mono(x.path)} title={x.path}>
                    {homeRelative(x.path)}
                  </span>
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
      {note && <p className="text-muted">{note}</p>}
      <div className="mt-3 flex items-center gap-2">
        <Button primary disabled={busy || !vault} onClick={() => vault && onConnect(vault)}>
          {t.settings.connect}
        </Button>
        {onCancel && <Button onClick={onCancel}>{t.settings.cancel}</Button>}
        <button type="button" className="ml-2 text-[12.5px] text-muted hover:text-fg" disabled={busy} onClick={onFolder}>
          {t.settings.saveToFolderInstead}
        </button>
      </div>
      {vault && (
        <p className="mt-2 text-[11.5px] text-faint">
          <span className={mono(vault.path)}>{homeRelative(vault.path)}/porch</span> {t.settings.savedUnder}
        </p>
      )}
    </div>
  );
}

/** A connected folder: what is there, where today's notes go, and the actions. */
function Linked({
  v,
  busy,
  run,
  onError,
  title,
  place,
  justConnected,
  open: opener,
  onRepick,
  note,
  children,
}: TabProps & {
  title: string;
  place: string;
  justConnected: boolean;
  open: { label: string; run: () => Promise<void> };
  onRepick: () => void;
  note?: string;
  children?: ReactNode;
}) {
  const [confirmOff, setConfirmOff] = useState(false);
  const progress = useMirrorProgress(busy);
  const t = useT();
  const m = v.mirror;
  const dir = v.settings.mirror_dir ?? "";
  const now = Date.now();
  const facts = [place, t.settings.notesCount(m.notes), m.last_saved ? t.settings.savedAt(ago(now, m.last_saved)) : null].filter(Boolean).join(" · ");
  const subs = [
    ["mirror_daily", t.settings.daily],
    ["mirror_weekly", t.settings.weekly],
    ["mirror_monthly", t.settings.monthly],
  ] as const;

  return (
    <div className="min-w-0">
      <h2 className="text-[15px] font-medium">{title}</h2>
      <p className="mt-2 flex items-baseline gap-2 text-muted">
        <Dot on={!m.vault_missing} />
        <span>{facts}</span>
      </p>
      {note && <p className="mt-1 text-[11.5px] text-faint">{note}</p>}
      {progress && progress.total > 0 ? (
        <Progress {...progress} />
      ) : (
        justConnected && m.notes > 0 && <p className="mt-1 text-[11.5px] text-faint">{t.settings.notesMade(m.notes)}</p>
      )}
      {m.elsewhere.map((e) => (
        <p key={e.dir} className="mt-1 text-[11.5px] text-faint">
          {t.settings.previousFolder} <span className={mono(e.dir)}>{homeRelative(e.dir)}</span>{t.settings.notesLeft(e.notes)}
        </p>
      ))}

      <Row label={t.settings.folder}>
        <span className={"block truncate text-[11.5px]" + mono(dir)} title={dir}>
          {homeRelative(dir)}
        </span>
      </Row>
      <Row label={t.settings.saveLocation} hint={t.settings.saveLocationHint}>
        <ul className="space-y-0.5">
          {m.samples.map((p) => (
            <li key={p} className={"truncate text-[11.5px] text-muted" + mono(p)}>
              {p}
            </li>
          ))}
        </ul>
        <Disclosure label={t.settings.changeFolderStructure}>
          <div className="space-y-1.5">
            {subs.map(([k, label]) => (
              <label key={k} className="flex items-center gap-2">
                <span className="w-8 text-faint">{label}</span>
                <input
                  className={field + " w-48" + mono(v.settings[k])}
                  defaultValue={v.settings[k]}
                  placeholder={t.settings.emptyMeansHere}
                  disabled={busy}
                  onBlur={(e) => {
                    const next = e.target.value.trim();
                    if (next !== v.settings[k]) run(saveMirror(v, v.settings.mirror_dir, v.settings.obsidian_vault, { [k]: next }));
                  }}
                />
              </label>
            ))}
          </div>
        </Disclosure>
      </Row>
      {children}

      <div className="mt-2 flex gap-2">
        <Button disabled={busy || m.vault_missing} onClick={() => opener.run().catch((e) => onError(String(e)))}>
          {opener.label}
        </Button>
        <Button disabled={busy || m.vault_missing} onClick={() => run(api.resaveMirror())}>
          {t.settings.resaveNow}
        </Button>
        <Button disabled={busy} onClick={onRepick}>
          {t.settings.pickAgain}
        </Button>
      </div>
      {m.last_error && <p className="mt-2 text-[11.5px] text-muted">{t.settings.lastSaveFailed(m.last_error)}</p>}
      {!v.settings.auto_summary_at && <p className="mt-2 text-[11.5px] text-muted">{t.settings.autoSummaryNeeded}</p>}
      <p className="mt-2 text-[11.5px] text-faint">
        {t.settings.rewriteNote}
      </p>

      <div className="mt-8 border-t border-line-soft pt-3">
        {confirmOff ? (
          <div className="flex items-center gap-2">
            <Button
              disabled={busy}
              onClick={() =>
                run(saveMirror(v, null, null).then((next) => {
                  track("mirror_disconnected", {});
                  return next;
                }))
              }
            >
              {t.settings.confirmDisconnect}
            </Button>
            <Button onClick={() => setConfirmOff(false)}>{t.settings.cancel}</Button>
            <span className="text-[11.5px] text-faint">{t.settings.notesKept}</span>
          </div>
        ) : (
          <button type="button" className="text-[12.5px] text-faint hover:text-fg" disabled={busy} onClick={() => setConfirmOff(true)}>
            {t.settings.disconnect}
          </button>
        )}
      </div>
    </div>
  );
}
