import { useEffect, useState } from "react";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { api, type Environment } from "../api";
import { homeRelative } from "../format";
import { Button } from "../components/ui";
import { Toggle } from "../components/Toggle";
import { Icon } from "../components/Icon";
import { track } from "../analytics";
import { useT } from "../i18n";

function Check({ ok, children, note }: { ok: boolean; children: string; note?: string }) {
  return (
    <li className="flex gap-3 py-1.5">
      <span aria-hidden className={"mt-[7px] inline-block h-2 w-2 shrink-0 rounded-full " + (ok ? "bg-accent" : "border border-faint")} />
      <span>
        <span className={ok ? "" : "text-muted"}>{children}</span>
        {note && <span className="block text-[11.5px] text-faint">{note}</span>}
      </span>
    </li>
  );
}

/** One quiet page shown on first launch; everything here can be changed later in Settings. */
export function Onboarding({ env, onDone }: { env: Environment; onDone: () => void }) {
  const t = useT();
  const [hooks, setHooks] = useState<"off" | "busy" | "on">(env.hooks_installed ? "on" : "off");
  const [error, setError] = useState<string | null>(null);
  const [autostart, setAutostart] = useState(false);

  useEffect(() => {
    isEnabled().then(setAutostart, () => {});
  }, []);

  const finish = () =>
    api.finishOnboarding().then(() => {
      track("onboarding_finished", { hooks: hooks === "on", autostart });
      onDone();
    });

  return (
    <div className="flex h-full items-center justify-center bg-canvas px-8" data-tauri-drag-region>
      <div className="w-full max-w-[520px]">
        <div className="text-[13px] font-medium">
          porch<span className="text-accent">.</span>
        </div>
        <h1 className="mt-8 text-[24px] font-medium leading-snug tracking-[-0.02em]">
          {t.onboarding.headlineA}
          <br />
          {t.onboarding.headlineB}
        </h1>
        <p className="mt-3 text-muted">{t.onboarding.body}</p>

        <ul className="mt-8">
          <Check ok={env.claude_data} note={env.claude_data ? undefined : t.onboarding.claudeRecordsNote}>
            {env.claude_data ? t.onboarding.claudeRecordsFound : t.onboarding.claudeRecordsMissing}
          </Check>
          <Check
            ok={!!(env.claude_bin || env.codex_bin)}
            note={env.claude_bin || env.codex_bin ? homeRelative((env.claude_bin ?? env.codex_bin)!) : t.onboarding.writerNote}
          >
            {env.claude_bin && env.codex_bin
              ? t.onboarding.writerBoth
              : env.claude_bin
                ? t.onboarding.writerClaude
                : env.codex_bin
                  ? t.onboarding.writerCodex
                  : t.onboarding.writerNone}
          </Check>
          <Check ok={env.codex_homes.length > 0} note={env.codex_homes.length > 0 ? undefined : t.onboarding.codexNote}>
            {env.codex_homes.length > 0 ? t.onboarding.codexHomes(env.codex_homes.length) : t.onboarding.codexMissing}
          </Check>
        </ul>

        <div className="mt-8 border-t border-line-soft pt-6">
          <div className="flex items-start justify-between gap-6">
            <div>
              <div>{t.onboarding.sessionDetection}</div>
              <p className="mt-0.5 text-[12px] text-faint">{t.onboarding.sessionDetectionHint}</p>
            </div>
            {hooks === "on" ? (
              <span className="inline-flex h-7 shrink-0 items-center gap-1.5 text-[12.5px] text-fg">
                <Icon name="check" size={13} /> {t.onboarding.on}
              </span>
            ) : (
              <Button
                primary
                disabled={hooks === "busy"}
                onClick={() => {
                  setHooks("busy");
                  setError(null);
                  api.setHooks(true).then(
                    () => {
                      setHooks("on");
                      track("hooks_installed", { agent: "all", from: "onboarding" });
                    },
                    (e) => {
                      setError(String(e));
                      setHooks("off");
                    },
                  );
                }}
              >
                {hooks === "busy" ? t.onboarding.turningOn : t.onboarding.turnOn}
              </Button>
            )}
          </div>
          {hooks === "on" && env.codex_homes.length > 0 && (
            <p className="mt-2 text-[12px] text-muted">{t.onboarding.codexApproveNote}</p>
          )}
          {error && <p className="mt-2 text-[12px] text-muted">{error}</p>}

          <div className="mt-5 flex items-center justify-between gap-6">
            <div>
              <div>{t.onboarding.openAtLogin}</div>
              <p className="mt-0.5 text-[12px] text-faint">{t.onboarding.openAtLoginHint}</p>
            </div>
            <Toggle
              label={t.onboarding.openAtLogin}
              on={autostart}
              onChange={(v) => (v ? enable() : disable()).then(() => setAutostart(v), () => {})}
            />
          </div>
        </div>

        <div className="mt-10 flex items-center gap-4">
          <Button primary={hooks === "on"} onClick={finish}>
            {t.onboarding.getStarted}
          </Button>
          {hooks !== "on" && <span className="text-[12px] text-faint">{t.onboarding.sessionDetectionLater}</span>}
        </div>
      </div>
    </div>
  );
}
