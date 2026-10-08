import { defineConfig } from "astro/config";

export default defineConfig({
  // The domain is not decided yet. Set SITE_URL when it is.
  site: process.env.SITE_URL,
  i18n: { defaultLocale: "en", locales: ["en", "ko"], routing: { prefixDefaultLocale: false } },
});
