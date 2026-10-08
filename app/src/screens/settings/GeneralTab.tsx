import { useEffect, useState } from "react";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { api } from "../../api";
import { homeRelative } from "../../format";
import { Button, Section } from "../../components/ui";
import { Toggle } from "../../components/Toggle";
import { setShowCost } from "../../prefs";
import { setAnalytics } from "../../analytics";
import { setZoom, useZoom } from "../../zoom";
import { useEnv } from "../../env";
import { asLang, setLang, useT } from "../../i18n";
import { endonym } from "../../i18n/endonyms";
import { Row, field, type TabProps } from "./Row";

/** Evening slots every 30 minutes; a summary is most useful once the day is done. */
const TIMES = Array.from({ length: 16 }, (_, i) => `${String(16 + Math.floor(i / 2)).padStart(2, "0")}:${i % 2 ? "30" : "00"}`);

export function GeneralTab({ v, busy, run, onError, focus }: TabProps & { focus?: "summary" }) {
  const zoom = useZoom();
  const [newPath, setNewPath] = useState("");
  const [autostart, setAutostart] = useState<boolean | null>(null);
  const [budget, setBudget] = useState(v.settings.monthly_budget ? String(v.settings.monthly_budget) : "");
  const [codexModels, setCodexModels] = useState<{ slug: string; name: string }[] | null>(null);
  const env = useEnv();
  const t = useT();
  const s = v.settings;

  useEffect(() => {
    isEnabled().then(setAutostart, () => setAutostart(false));
  }, []);

  // Opened from a usage-limit notice: bring the agent choice into view.
  useEffect(() => {
    if (focus !== "summary") return;
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    document.getElementById("settings-summary")?.scrollIntoView({ block: "start", behavior: reduce ? "auto" : "smooth" });
  }, [focus]);

  useEffect(() => {
    if (s.provider === "codex" && codexModels === null) api.codexModels().then(setCodexModels, () => setCodexModels([]));
  }, [s.provider, codexModels]);

  const addPath = () => {
    if (!newPath.trim()) return;
    run(api.saveExcluded([...v.excluded, newPath.trim()]));
    setNewPath("");
  };

  return (
    <div>
      <Section title={t.settings.sessionDetection}>
        <Row label={t.lang.label} hint={t.lang.hint}>
          <select
            className={field}
            value={s.language}
            disabled={busy}
            onChange={(e) =>
              run(
                api.saveLanguage(e.target.value as "system" | "ko" | "en").then((next) => {
                  setLang(asLang(next.resolved_language));
                  return next;
                })
              )
            }
          >
            <option value="system">{t.lang.system}</option>
            <option value="ko">{endonym.ko}</option>
            <option value="en">{endonym.en}</option>
          </select>
        </Row>
        <Row label={t.settings.excluded} hint={t.settings.excludedHint}>
          <ul className="space-y-1">
            {v.excluded.length === 0 && <li className="text-muted">{t.settings.none}</li>}
            {v.excluded.map((p) => (
              <li key={p} className="flex items-center gap-3">
                <code className="min-w-0 flex-1 truncate font-mono text-[11.5px]">{homeRelative(p)}</code>
                <button
                  type="button"
                  className="text-[12px] text-faint hover:text-fg"
                  onClick={() => run(api.saveExcluded(v.excluded.filter((x) => x !== p)))}
                >
                  {t.settings.removeItem}
                </button>
              </li>
            ))}
          </ul>
          <div className="mt-2 flex gap-2">
            <input
              className={field + " min-w-0 flex-1 font-mono"}
              placeholder="~/private-repo"
              value={newPath}
              onChange={(e) => setNewPath(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && addPath()}
            />
            <Button onClick={addPath} disabled={!newPath.trim()}>
              {t.settings.add}
            </Button>
          </div>
        </Row>
      </Section>

      <Section title={t.nav.usage}>
        <Row label={t.settings.showCost} hint={t.settings.showCostHint}>
          <Toggle
            label={t.settings.showCost}
            on={s.show_cost}
            disabled={busy}
            onChange={(on) => {
              setShowCost(on);
              run(api.saveUsageSettings(s.monthly_budget, on));
            }}
          />
        </Row>
        <Row label={t.settings.monthlyBudget} hint={t.settings.monthlyBudgetHint}>
          <div className="flex items-center gap-2">
            <span className="text-muted">$</span>
            <input
              value={budget}
              onChange={(e) => setBudget(e.target.value.replace(/[^0-9.]/g, ""))}
              placeholder={t.settings.noBudget}
              inputMode="decimal"
              className="h-7 w-28 rounded-md border border-line bg-transparent px-2 text-[12.5px] outline-none focus:border-muted"
            />
            <Button
              disabled={busy || budget === (s.monthly_budget ? String(s.monthly_budget) : "")}
              onClick={() => run(api.saveUsageSettings(budget ? Number(budget) : null, s.show_cost))}
            >
              {t.settings.save}
            </Button>
          </div>
        </Row>
      </Section>

      <Section title={t.nav.summary} id="settings-summary">
        <Row
          label={t.settings.summaryAgent}
          hint={t.settings.summaryAgentHint}
        >
          <div className="flex gap-1">
            {(
              [
                ["claude", "Claude Code", env?.claude_bin],
                ["codex", "Codex", env?.codex_bin],
              ] as const
            ).map(([id, name, found]) => (
              <button
                key={id}
                type="button"
                disabled={!found && s.provider !== id}
                title={found ? undefined : t.settings.notFoundHere}
                onClick={() => run(api.saveSettings({ ...s, provider: id }))}
                className={
                  "h-7 rounded-md px-2.5 text-[12.5px] motion-safe:transition-colors motion-safe:duration-fast disabled:opacity-40 " +
                  (s.provider === id ? "bg-fg/10 text-fg" : "text-muted hover:bg-fg/5 disabled:hover:bg-transparent")
                }
              >
                {name}
              </button>
            ))}
          </div>
        </Row>
        {s.provider === "claude" ? (
          <Row label={t.settings.model} hint={t.settings.claudeModelHint}>
            <div className="flex gap-1">
              {v.models.map((m) => (
                <button
                  key={m}
                  type="button"
                  onClick={() => run(api.saveSettings({ ...s, model: m }))}
                  className={
                    "h-7 rounded-md px-2.5 text-[12.5px] motion-safe:transition-colors motion-safe:duration-fast " +
                    (s.model === m ? "bg-fg/10 text-fg" : "text-muted hover:bg-fg/5")
                  }
                >
                  {m}
                </button>
              ))}
            </div>
          </Row>
        ) : (
          <Row label={t.settings.model} hint={t.settings.codexModelHint}>
            <select
              className={field + " w-48"}
              value={s.codex_model ?? ""}
              disabled={codexModels === null}
              onChange={(e) => run(api.saveSettings({ ...s, codex_model: e.target.value || null }))}
            >
              <option value="">{codexModels?.[0] ? t.settings.defaultModelNamed(codexModels[0].name) : t.settings.defaultModel}</option>
              {(codexModels ?? []).map((m) => (
                <option key={m.slug} value={m.slug}>
                  {m.name}
                </option>
              ))}
              {/* A saved model Codex no longer lists stays visible rather than silently changing. */}
              {s.codex_model && codexModels && !codexModels.some((m) => m.slug === s.codex_model) && (
                <option value={s.codex_model}>{s.codex_model}</option>
              )}
            </select>
          </Row>
        )}
        <Row label={t.settings.autoSummary} hint={t.settings.autoSummaryHint}>
          <select className={field + " w-32"} value={s.auto_summary_at ?? ""} onChange={(e) => run(api.saveSettings({ ...s, auto_summary_at: e.target.value || null }))}>
            <option value="">{t.settings.off}</option>
            {TIMES.map((slot) => (
              <option key={slot} value={slot}>
                {slot}
              </option>
            ))}
          </select>
        </Row>
        <Row label={t.settings.insightEval} hint={t.settings.insightEvalHint}>
          <Toggle on={s.insight_eval} disabled={busy} label={t.settings.insightEval} onChange={(on) => run(api.saveInsightEval(on))} />
        </Row>
      </Section>

      <Section title={t.settings.app}>
        <Row label={t.settings.screenSize} hint={t.settings.screenSizeHint}>
          <div className="flex items-center gap-2">
            <Button onClick={() => setZoom(zoom - 10)} disabled={zoom <= 70} aria-label={t.settings.zoomOut}>−</Button>
            <span className="min-w-12 text-center tabular-nums">{zoom}%</span>
            <Button onClick={() => setZoom(zoom + 10)} disabled={zoom >= 200} aria-label={t.settings.zoomIn}>+</Button>
            <Button onClick={() => setZoom(100)} disabled={zoom === 100}>{t.settings.defaultSize}</Button>
          </div>
        </Row>
        <Row label={t.onboarding.openAtLogin} hint={t.onboarding.openAtLoginHint}>
          <Toggle
            label={t.onboarding.openAtLogin}
            on={!!autostart}
            disabled={autostart === null}
            onChange={(on) => (on ? enable() : disable()).then(() => setAutostart(on), (e) => onError(String(e)))}
          />
        </Row>
        <Row label={t.settings.terminalCommand} hint={t.settings.terminalCommandHint}>
          <Toggle
            label={t.settings.terminalCommand}
            on={v.cli_link === "linked" || v.cli_link === "stale"}
            disabled={busy || v.cli_link === "other"}
            onChange={(on) => run(api.setCliLink(on))}
          />
          {v.cli_link === "stale" && (
            <div className="mt-2 flex items-center gap-2">
              <p className="text-[11.5px] text-faint">{t.settings.cliOtherLocation}</p>
              <Button onClick={() => run(api.setCliLink(true))} disabled={busy}>
                {t.settings.switchToThisApp}
              </Button>
            </div>
          )}
          {v.cli_link === "other" && <p className="mt-2 text-[11.5px] text-faint">{t.settings.cliOtherProgram(v.cli_link_path)}</p>}
        </Row>
        <Row label={t.settings.sendUsage} hint={t.settings.sendUsageHint}>
          <Toggle
            label={t.settings.sendUsage}
            on={s.analytics}
            disabled={busy}
            onChange={(on) => {
              setAnalytics(on);
              run(api.saveAnalytics(on));
            }}
          />
        </Row>
      </Section>

      <p className="mt-12 text-[11.5px] text-faint">
        {t.settings.privacyNote}
      </p>
    </div>
  );
}
