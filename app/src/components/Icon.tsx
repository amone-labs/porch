// 16px outline glyphs on a 24 grid with a shared 2px stroke. Use these instead
// of text glyphs (‹ › ▶).

export type IconName = "now" | "today" | "history" | "projects" | "settings" | "chevronLeft" | "chevronRight" | "chevronDown" | "check" | "refresh" | "download" | "usage" | "insight";

const PATHS: Record<IconName, JSX.Element> = {
  now: (
    <>
      <circle cx="12" cy="12" r="8.5" />
      <path d="M12 7.5V12l3 2" />
    </>
  ),
  today: (
    <>
      <rect x="4" y="5.5" width="16" height="14" rx="2.5" />
      <path d="M4 10h16M8.5 3.5v4M15.5 3.5v4" />
    </>
  ),
  history: (
    <>
      <path d="M4 6h16M4 12h16M4 18h10" />
    </>
  ),
  projects: <path d="M3.5 7.5A2 2 0 0 1 5.5 5.5h4l2 2h7a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z" />,
  settings: (
    <>
      <circle cx="12" cy="12" r="2.75" />
      <path d="M12 3.5v2.2M12 18.3v2.2M20.5 12h-2.2M5.7 12H3.5M18 6l-1.6 1.6M7.6 16.4 6 18M18 18l-1.6-1.6M7.6 7.6 6 6" />
    </>
  ),
  chevronLeft: <polyline points="15 5 8 12 15 19" />,
  chevronRight: <polyline points="9 5 16 12 9 19" />,
  chevronDown: <polyline points="5 8.5 12 15.5 19 8.5" />,
  check: <polyline points="4.5 12.5 9.5 17.5 19.5 6.5" />,
  insight: (
    <>
      <path d="M4 17.5 9 12l4 3 7-8" />
      <circle cx="20" cy="7" r="1.6" />
    </>
  ),
  usage: (
    <>
      <path d="M5 19.5V13M10 19.5V8.5M15 19.5V11M20 19.5V5" />
    </>
  ),
  download: (
    <>
      <path d="M12 4.5v10" />
      <polyline points="7.5 10.5 12 15 16.5 10.5" />
      <path d="M5 19.5h14" />
    </>
  ),
  refresh: (
    <>
      <path d="M19.5 12a7.5 7.5 0 1 1-2.2-5.3" />
      <polyline points="19.5 4.5 19.5 8.5 15.5 8.5" />
    </>
  ),
};

export function Icon({ name, size = 16, className = "" }: { name: IconName; size?: number; className?: string }) {
  return (
    <svg
      aria-hidden
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={"shrink-0 " + className}
    >
      {PATHS[name]}
    </svg>
  );
}
