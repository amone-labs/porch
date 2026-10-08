import type { APIRoute } from "astro";

// Everything may be crawled; noindex pages say so in their own head. The sitemap line needs SITE_URL.
export const GET: APIRoute = ({ site }) =>
  new Response(`User-agent: *\nAllow: /\n${site ? `\nSitemap: ${new URL("/sitemap.xml", site).href}\n` : ""}`, {
    headers: { "Content-Type": "text/plain; charset=utf-8" },
  });
