import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { useT } from "../i18n";
import { checkUpdate, showUpdate, useUpdate } from "../update";
import { Button } from "./ui";

/** Version and a manual check; the app also checks by itself at launch and daily (update.ts). */
export function UpdateRow() {
  const t = useT();
  const [version, setVersion] = useState("");
  const [note, setNote] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const u = useUpdate();

  useEffect(() => {
    getVersion().then(setVersion);
  }, []);

  const run = () => {
    setChecking(true);
    setNote(null);
    checkUpdate().then((r) => {
      setChecking(false);
      if (r === "latest") setNote(t.update.latest);
      else if (r === "unreachable") setNote(t.update.unreachable);
    });
  };

  return (
    <div className="flex flex-wrap items-center gap-3">
      <code className="font-mono text-[12px]">{version && `v${version}`}</code>
      {u.status !== "idle" ? (
        <Button primary onClick={showUpdate}>
          {t.update.viewVersion(u.version ?? "")}
        </Button>
      ) : (
        <Button onClick={run} disabled={checking}>
          {checking ? t.update.checking : t.update.check}
        </Button>
      )}
      {note && <span className="text-[12px] text-faint">{note}</span>}
    </div>
  );
}
