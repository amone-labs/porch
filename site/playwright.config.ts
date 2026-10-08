import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "e2e",
  // The site is dark only; theme.spec.ts switches the browser to light to check it stays dark.
  use: { baseURL: "http://127.0.0.1:4329", colorScheme: "dark" },
  webServer: {
    // exec: the shell becomes the server, so Playwright can stop it (astro preview detaches).
    command: "pnpm build && exec node scripts/serve.mjs",
    url: "http://127.0.0.1:4329",
    // Never reuse: a stale preview would serve an old build.
    reuseExistingServer: false,
    timeout: 180_000,
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
});
