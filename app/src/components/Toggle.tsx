/** Brightness-only switch: filled knob track when on, hairline when off. */
export function Toggle({ on, onChange, label, disabled }: { on: boolean; onChange: (v: boolean) => void; label: string; disabled?: boolean }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!on)}
      className={
        "relative inline-flex h-5 w-9 shrink-0 items-center rounded-full border motion-safe:transition-colors motion-safe:duration-fast disabled:opacity-50 " +
        (on ? "border-transparent bg-accent" : "border-line bg-surface")
      }
    >
      <span
        className={
          "inline-block h-3.5 w-3.5 rounded-full motion-safe:transition-transform motion-safe:duration-fast " +
          (on ? "translate-x-[18px] bg-on-accent" : "translate-x-[2px] bg-muted")
        }
      />
    </button>
  );
}
