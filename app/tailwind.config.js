/** @type {import('tailwindcss').Config} */
// Tokens follow docs/design/ui.md: one achromatic axis, depth by brightness,
// a single near-white accent. Never add a hue here.
const t = (name) => `rgb(var(--${name}) / <alpha-value>)`;
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        canvas: t("canvas"),
        surface: t("surface"),
        elevated: t("elevated"),
        raised: t("raised"),
        line: t("border"),
        "line-soft": t("border-soft"),
        fg: t("text"),
        muted: t("muted"),
        faint: t("faint"),
        accent: t("accent"),
        "on-accent": t("on-accent"),
      },
      fontFamily: {
        sans: ["-apple-system", "BlinkMacSystemFont", '"Apple SD Gothic Neo"', '"Noto Sans KR"', "system-ui", "sans-serif"],
        mono: ["ui-monospace", '"SF Mono"', "Menlo", "monospace"],
      },
      transitionDuration: { fast: "120ms" },
      transitionTimingFunction: { calm: "ease" },
      letterSpacing: { tightish: "-0.006em", label: "0.14em" },
    },
  },
  plugins: [],
};
