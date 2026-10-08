import { useEffect, useRef, useState } from "react";
import { setZoom, useZoom } from "../zoom";
import { useT } from "../i18n";

const VISIBLE_MS = 1500;

/** Shows the zoom level only while it changes, then fades out. */
export function ZoomIndicator() {
  const t = useT();
  const percent = useZoom();
  const [visible, setVisible] = useState(false);
  const [hovered, setHovered] = useState(false);
  const shown = useRef(percent);

  useEffect(() => {
    if (shown.current === percent) return;
    shown.current = percent;
    setVisible(true);
  }, [percent]);

  useEffect(() => {
    if (!visible || hovered) return;
    const timer = window.setTimeout(() => setVisible(false), VISIBLE_MS);
    return () => window.clearTimeout(timer);
  }, [visible, hovered, percent]);

  return (
    <button
      type="button"
      onClick={() => setZoom(100)}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      tabIndex={visible ? 0 : -1}
      aria-hidden={!visible}
      title={t.zoom.reset}
      aria-label={t.zoom.aria(percent)}
      className={`fixed bottom-3 right-3 z-50 h-7 rounded-md bg-surface px-2 text-[11.5px] text-muted shadow-sm transition-[opacity,background-color,color] hover:bg-elevated hover:text-fg ${
        visible ? "opacity-100 duration-fast" : "pointer-events-none opacity-0 duration-300"
      }`}
    >
      <span aria-live="polite">{percent}%{percent === 100 ? t.zoom.defaultLabel : ""}</span>
    </button>
  );
}
