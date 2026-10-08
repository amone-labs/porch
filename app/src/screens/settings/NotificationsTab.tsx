import { api } from "../../api";
import { Section } from "../../components/ui";
import { useT } from "../../i18n";
import { NotifyRow } from "./NotifyRow";
import { Row, field, type TabProps } from "./Row";

export function NotificationsTab(props: TabProps) {
  const { v, busy, run } = props;
  const t = useT();
  const s = v.settings;
  const hasVault = !!s.obsidian_vault;
  return (
    <div>
      <Section title={t.settings.summaryNotifications}>
        <NotifyRow {...props} />
        <Row label={t.settings.whenClickingNotification} hint={hasVault ? t.settings.openLocationHint : t.settings.connectVaultHint}>
          <select
            aria-label={t.settings.notificationOpenAria}
            className={field}
            value={s.notify_open}
            disabled={busy || !s.notify}
            onChange={(e) => run(api.saveNotify(s.notify, e.target.value as "porch" | "obsidian"))}
          >
            <option value="porch">{t.settings.porchSummary}</option>
            <option value="obsidian" disabled={!hasVault}>{t.settings.obsidianNote}</option>
          </select>
        </Row>
      </Section>
      <Section title={t.settings.otherNotifications}>
        <p className="text-[12.5px] text-muted">{t.settings.otherNotificationsNote}</p>
      </Section>
    </div>
  );
}
