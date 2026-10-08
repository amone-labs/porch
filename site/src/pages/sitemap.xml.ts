import type { APIRoute } from "astro";

// The two indexable pages, each with its language alternates. 404, og and scenes pages are noindex and left out.
// Without SITE_URL there is no origin to write, so the sitemap is empty.
const PAGES = ["/", "/ko/"];
const ALTERNATES = [
  { lang: "en", path: "/" },
  { lang: "ko", path: "/ko/" },
  { lang: "x-default", path: "/" },
];

export const GET: APIRoute = ({ site }) => {
  const at = (path: string) => (site ? new URL(path, site).href : path);
  const urls = site
    ? PAGES.map(
        (p) =>
          `  <url>\n    <loc>${at(p)}</loc>\n${ALTERNATES.map((a) => `    <xhtml:link rel="alternate" hreflang="${a.lang}" href="${at(a.path)}"/>`).join("\n")}\n  </url>`,
      ).join("\n")
    : "";
  const body = `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">\n${urls}\n</urlset>\n`;
  return new Response(body, { headers: { "Content-Type": "application/xml; charset=utf-8" } });
};
