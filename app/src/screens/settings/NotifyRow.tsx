import { api } from "../../api";
import { Button } from "../../components/ui";
import { Toggle } from "../../components/Toggle";
import { useT } from "../../i18n";
import { Row, type TabProps } from "./Row";

/** Summary notifications on or off, with a test. porch cannot read macOS's
 * permission for its notifications, so a person checks by sending one. */
export function NotifyRow({ v, busy, run, onError }: Pick<TabProps, "v" | "busy" | "run" | "onError">) {
  const t = useT();
  const s = v.settings;
  return (
    <Row label={t.settings.summaryNotifications} hint={t.settings.summaryNotificationsHint}>
      <div className="flex items-center gap-3">
        <Toggle label={t.settings.summaryNotifications} on={s.notify} disabled={busy} onChange={(on) => run(api.saveNotify(on, s.notify_open))} />
        <Button disabled={!s.notify} onClick={() => api.testNotification().catch((e) => onError(String(e)))}>
          {t.settings.sendTestNotification}
        </Button>
      </div>
      <p className="mt-2 text-[11.5px] text-faint">
        {t.settings.notificationNotArriving}{" "}
        <button type="button" className="text-muted underline-offset-2 hover:text-fg hover:underline" onClick={() => api.openNotificationSettings().catch((e) => onError(String(e)))}>
          {t.settings.openSystemSettings}
        </button>
        {t.settings.allowNotifications}
      </p>
    </Row>
  );
}
