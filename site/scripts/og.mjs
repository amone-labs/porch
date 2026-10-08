// Renders public/og-<locale>.png from /og/<locale>/. Run after `pnpm build`.
import { spawn } from "node:child_process";
import { chromium } from "@playwright/test";

const server = spawn("node", ["scripts/serve.mjs"], { stdio: "ignore" });
try {
  for (let i = 0; i < 50; i++) {
    const ok = await fetch("http://127.0.0.1:4329/og/en/").then((r) => r.ok, () => false);
    if (ok) break;
    await new Promise((r) => setTimeout(r, 200));
  }
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1200, height: 630 }, colorScheme: "dark" });
  for (const locale of ["en", "ko"]) {
    await page.goto(`http://127.0.0.1:4329/og/${locale}/`);
    await page.screenshot({ path: `public/og-${locale}.png`, clip: { x: 0, y: 0, width: 1200, height: 630 } });
  }
  await browser.close();
} finally {
  server.kill();
}
