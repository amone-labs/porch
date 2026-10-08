import { api } from "../../api";
import { Button, Section } from "../../components/ui";
import { Toggle } from "../../components/Toggle";
import { useT } from "../../i18n";
import { Row, type TabProps } from "./Row";

/** Claude's 5-hour and weekly limits reach porch only through its status line. */
export function ClaudeLimits({ v, busy, run }: TabProps) {
  const t = useT();
  return (
    <Section title={t.settings.limitsSection}>
      <Row
        label={t.settings.importClaudeLimits}
        hint={v.statusline_had_original ? t.settings.limitsHintOriginal : t.settings.limitsHint}
      >
        <div className="flex flex-wrap items-center gap-3">
          <Toggle
            label={t.settings.importClaudeLimits}
            on={v.statusline !== "off"}
            disabled={busy || !v.hook_binary_present}
            onChange={(on) => run(api.setStatusline(on, v.statusline_show_line))}
          />
          {v.statusline === "disconnected" && (
            <>
              <span className="text-[12px] text-fg/90">{t.settings.limitsDisconnected}</span>
              <Button disabled={busy} onClick={() => run(api.setStatusline(true, v.statusline_show_line))}>
                {t.settings.reconnect}
              </Button>
            </>
          )}
          {!v.hook_binary_present && <span className="text-[12px] text-muted">{t.settings.turnOnDetectionFirst}</span>}
        </div>
      </Row>
      {v.statusline !== "off" && v.statusline_projects.length > 0 && (
        <Row
          label={t.settings.projectStatusLines}
          hint={t.settings.projectStatusLinesHint}
        >
          <ul className="space-y-1">
            {v.statusline_projects.map((p) => (
              <li key={p.root} className="flex items-center gap-3">
                <span className="min-w-0 flex-1 truncate" title={`${p.root}\n${p.command}`}>
                  {p.root.split("/").filter(Boolean).pop()}
                  <span className="ml-2 font-mono text-[11px] text-faint">{(p.command.match(/[\w.-]+\.(?:c?js|sh|py|ts)\b/) ?? [p.command.slice(0, 32)])[0]}</span>
                </span>
                <span className={"shrink-0 text-[11.5px] " + (p.wrapped ? "text-muted" : "text-fg/90")}>
                  {p.wrapped ? t.settings.connected : p.disconnected ? t.settings.disconnected : t.settings.notConnected}
                </span>
                <Button disabled={busy} onClick={() => run(api.setProjectStatusline([p.root], !p.wrapped))}>
                  {p.wrapped ? t.settings.unwrap : t.settings.connect}
                </Button>
              </li>
            ))}
          </ul>
          {v.statusline_projects.some((p) => !p.wrapped) && (
            <div className="mt-2">
              <Button primary disabled={busy} onClick={() => run(api.setProjectStatusline([], true))}>
                {t.settings.connectAll}
              </Button>
            </div>
          )}
        </Row>
      )}
      {v.statusline === "on" && !v.statusline_had_original && (
        <Row label={t.settings.showStatusLine} hint={t.settings.showStatusLineHint}>
          <Toggle label={t.settings.showStatusLine} on={v.statusline_show_line} disabled={busy} onChange={(on) => run(api.setStatusline(true, on))} />
        </Row>
      )}
    </Section>
  );
}
