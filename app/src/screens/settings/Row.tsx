import type { ReactNode } from "react";
import type { SettingsView } from "../../api";

export const field = "h-7 rounded-md border border-line bg-surface px-2 text-[12.5px] text-fg outline-none focus-visible:border-muted";

export interface TabProps {
  v: SettingsView;
  busy: boolean;
  run: (p: Promise<SettingsView>) => void;
  onError: (e: string) => void;
}

export function Row({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <div className="grid grid-cols-[11rem_1fr] items-start gap-6 py-3">
      <div>
        <div>{label}</div>
        {hint && <div className="mt-0.5 text-[11.5px] text-faint">{hint}</div>}
      </div>
      <div className="min-w-0">{children}</div>
    </div>
  );
}
