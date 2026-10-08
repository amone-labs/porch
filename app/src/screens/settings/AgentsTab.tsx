import { useEffect, useState } from "react";
import { useKept } from "../../kept";
import { api, worst, type AgentView, type LastSeen } from "../../api";
import { ago, hhmm, homeRelative } from "../../format";
import { Button } from "../../components/ui";
import { useEnv } from "../../env";
import { ClaudeLimits } from "./ClaudeLimits";
import { track } from "../../analytics";
import { useT } from "../../i18n";
import { Row, type TabProps } from "./Row";

type Agent = AgentView["agent"];

const NAME: Record<Agent, string> = { claude: "Claude Code", codex: "Codex" };
function Dot({ on }: { on: boolean }) {
  return (
    <span
      aria-hidden
      className={"inline-block h-2 w-2 shrink-0 translate-y-[-1px] rounded-full " + (on ? "bg-accent" : "border border-faint")}
    />
  );
}

/** Polls last events while mounted; the list and detail both read it. */
function useLastEvents(): { seen: LastSeen[] | null; now: number; reload: () => void } {
  const [seen, setSeen] = useState<LastSeen[] | null>(null);
  const [now, setNow] = useState(Date.now());
  const [nonce, setNonce] = useState(0);
  useEffect(() => {
    const load = () =>
      api.lastEvents().then(
        (s) => {
          setSeen(s);
          setNow(Date.now());
        },
        () => setSeen(null),
      );
    load();
    const id = setInterval(load, 5000);
    return () => clearInterval(id);
  }, [nonce]);
  return { seen, now, reload: () => setNonce((n) => n + 1) };
}

export function AgentsTab(props: TabProps) {
  const { v } = props;
  const [picked, setPicked] = useKept<Agent>("settings.agent", "claude");
  const { seen, now, reload } = useLastEvents();
  const t = useT();
  /** undefined: not known yet (still loading, or the call failed); null: known, nothing in the last week. */
  const lastOf = (a: Agent): number | null | undefined =>
    seen === null ? undefined : (seen.find((s) => s.agent === a)?.t ?? null);
  const agent = v.agents.find((a) => a.agent === picked) ?? v.agents[0];

  return (
    <div className="mt-10 grid grid-cols-[14rem_1fr] gap-8">
      <ul className="space-y-px">
        {v.agents.map((a) => {
          const state = worst(a.targets);
          const at = lastOf(a.agent);
          return (
            <li key={a.agent} className={a.present ? "" : "opacity-50"}>
              <button
                type="button"
                aria-current={picked === a.agent ? "true" : undefined}
                onClick={() => setPicked(a.agent)}
                className={
                  "flex w-full items-start gap-2.5 rounded-md px-2.5 py-2 text-left motion-safe:transition-colors motion-safe:duration-fast " +
                  (picked === a.agent ? "bg-fg/10 text-fg" : "text-muted hover:bg-fg/5 hover:text-fg")
                }
              >
                <span className="pt-1.5">
                  <Dot on={a.present && state === "installed"} />
                </span>
                <span className="min-w-0">
                  <span className="block">{NAME[a.agent]}</span>
                  <span className="block text-[11.5px] text-faint">
                    {a.present ? t.settings.hookState[state] : t.settings.notOnThisMac} · {at == null ? "—" : ago(now, at)}
                  </span>
                </span>
              </button>
            </li>
          );
        })}
      </ul>
      <AgentDetail {...props} agent={agent} last={lastOf(agent.agent)} now={now} afterChange={reload} />
    </div>
  );
}

function AgentDetail({
  v,
  busy,
  run,
  onError,
  agent,
  last,
  now,
  afterChange,
}: TabProps & { agent: AgentView; last: number | null | undefined; now: number; afterChange: () => void }) {
  const [confirmRemove, setConfirmRemove] = useState(false);
  const env = useEnv();
  const t = useT();
  const bin = agent.agent === "claude" ? env?.claude_bin : env?.codex_bin;
  const state = worst(agent.targets);
  const installed = state === "installed";
  const change = (on: boolean) => {
    setConfirmRemove(false);
    const done = api.setHooks(on, agent.agent).then((v) => {
      if (on) track("hooks_installed", { agent: agent.agent, from: "settings" });
      return v;
    });
    run(done.finally(afterChange));
  };

  useEffect(() => setConfirmRemove(false), [agent.agent]);

  // Nothing to install into or report on: every row below would only repeat "none".
  if (!agent.present) {
    return (
      <div className="min-w-0">
        <h2 className="text-[15px] font-medium">{NAME[agent.agent]}</h2>
        <p className="mt-2 text-muted">
          {t.settings.notFoundLookedIn} <code className="font-mono text-[11.5px]">{t.settings.lookedIn[agent.agent]}</code>
        </p>
      </div>
    );
  }

  return (
    <div className="min-w-0">
      <h2 className="text-[15px] font-medium">{NAME[agent.agent]}</h2>

      <Row label={t.settings.hook} hint={t.settings.hookHint}>
        <div className="flex items-baseline gap-2">
          <Dot on={installed} />
          <span>{t.settings.hookState[state]}</span>
        </div>
        {agent.targets
          .filter((t2) => t2.detail)
          .map((t2) => (
            <p key={t2.file} className="mt-0.5 text-[11.5px] text-faint">
              {t2.detail}
            </p>
          ))}
      </Row>
      <Row label={t.settings.configFiles}>
        <ul className="space-y-1">
          {agent.targets.length === 0 && <li className="text-muted">{t.settings.none}</li>}
          {agent.targets.map((t2) => (
            <li key={t2.file} className="flex items-baseline gap-3">
              <code className="min-w-0 flex-1 truncate font-mono text-[11.5px]" title={t2.file}>
                {homeRelative(t2.file)}
              </code>
              {agent.targets.length > 1 && <span className="shrink-0 text-[11.5px] text-faint">{t.settings.hookState[t2.state]}</span>}
            </li>
          ))}
        </ul>
      </Row>
      <Row label={t.settings.lastEvent}>
        {last === undefined ? (
          <span className="text-muted">—</span>
        ) : last === null ? (
          <span className="text-muted">{t.settings.noRecentEvents}</span>
        ) : (
          <span>
            {ago(now, last)} <span className="font-mono text-[11.5px] text-faint">({hhmm(last)})</span>
          </span>
        )}
      </Row>
      <Row label={t.settings.command} hint={t.settings.commandHint}>
        {bin ? (
          <code className="font-mono text-[11.5px] text-muted">{homeRelative(bin)}</code>
        ) : (
          <span className="text-muted">{agent.agent === "claude" ? t.settings.commandMissingClaude : t.settings.commandMissingCodex}</span>
        )}
      </Row>

      <div className="mt-2 flex gap-2">
        <Button primary={!installed} disabled={busy} onClick={() => change(true)}>
          {installed ? t.settings.reinstall : t.settings.install}
        </Button>
        {confirmRemove ? (
          <>
            <Button disabled={busy} onClick={() => change(false)}>
              {t.settings.confirmRemove}
            </Button>
            <Button onClick={() => setConfirmRemove(false)}>{t.settings.cancel}</Button>
          </>
        ) : (
          <Button
            disabled={busy || agent.targets.every((t2) => t2.state === "missing")}
            onClick={() => setConfirmRemove(true)}
          >
            {t.settings.remove}
          </Button>
        )}
      </div>
      {agent.agent === "codex" && agent.targets.some((t2) => t2.state === "installed") && (
        <p className="mt-2 text-[11.5px] text-faint">{t.settings.codexApproveNote}</p>
      )}
      {!v.hook_binary_present && (
        <p className="mt-2 text-[11.5px] text-muted">{t.settings.noHookBinary}</p>
      )}

      {agent.agent === "claude" && <ClaudeLimits v={v} busy={busy} run={run} onError={onError} />}
    </div>
  );
}
