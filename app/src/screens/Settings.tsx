import { useEffect, useState } from "react";
import { useKept } from "../kept";
import { api, type SettingsView } from "../api";
import { Empty } from "../components/ui";
import { NotificationsTab } from "./settings/NotificationsTab";
import { GeneralTab } from "./settings/GeneralTab";
import { AboutTab } from "./settings/AboutTab";
import { AgentsTab } from "./settings/AgentsTab";
import { IntegrationsTab } from "./settings/IntegrationsTab";
import { track } from "../analytics";
import { useT } from "../i18n";
import type { Dict } from "../i18n/ko";

export type SettingsTab = "agents" | "integrations" | "general" | "notifications" | "about";

const TABS: { id: SettingsTab; label: keyof Dict["settings"]["tabs"] }[] = [
  { id: "agents", label: "agents" },
  { id: "integrations", label: "integrations" },
  { id: "general", label: "general" },
  { id: "notifications", label: "notifications" },
  { id: "about", label: "about" },
];

export function Settings({ tab: initial, focus }: { tab?: SettingsTab; focus?: "summary" }) {
  const t = useT();
  // A tab named by the caller (a notice) wins once; otherwise the last one open.
  const [kept, keep] = useKept<SettingsTab>("settings.tab", "agents");
  const [tab, setShown] = useState<SettingsTab>(initial ?? kept);
  const setTab = (next: SettingsTab) => {
    keep(next);
    setShown(next);
  };
  useEffect(() => track("tab_viewed", { screen: "settings", tab }), [tab]);
  const [v, setV] = useState<SettingsView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api.settings().then(setV, (e) => setError(String(e)));
  }, []);

  const run = (p: Promise<SettingsView>) => {
    setBusy(true);
    setError(null);
    p.then(setV, (e) => setError(String(e))).finally(() => setBusy(false));
  };

  if (!v) return <Empty>{error ?? t.common.loading}</Empty>;
  const props = { v, busy, run, onError: setError };

  return (
    <div>
      <header className="pb-6 pt-4">
        <h1 className="text-[21px] font-medium leading-snug tracking-[-0.02em]">{t.nav.settings}</h1>
      </header>
      <div role="tablist" className="mt-6 flex gap-1">
        {TABS.map((item) => (
          <button
            key={item.id}
            type="button"
            role="tab"
            aria-selected={tab === item.id}
            onClick={() => setTab(item.id)}
            className={
              "h-7 rounded-md px-2.5 text-[12.5px] motion-safe:transition-colors motion-safe:duration-fast " +
              (tab === item.id ? "bg-fg/10 text-fg" : "text-muted hover:bg-fg/5 hover:text-fg")
            }
          >
            {t.settings.tabs[item.label]}
          </button>
        ))}
      </div>
      {error && <p className="mt-4 text-muted">{error}</p>}
      {tab === "agents" && <AgentsTab {...props} />}
      {tab === "integrations" && <IntegrationsTab {...props} />}
      {tab === "general" && <GeneralTab {...props} focus={tab === initial ? focus : undefined} />}
      {tab === "notifications" && <NotificationsTab {...props} />}
      {tab === "about" && <AboutTab />}
    </div>
  );
}
