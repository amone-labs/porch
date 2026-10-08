// Serves dist/ for page tests and share images. Astro 7's `preview` detaches its server,
// which Playwright then cannot stop; this one stays in the foreground.
import { createReadStream, statSync } from "node:fs";
import { createServer } from "node:http";
import { extname, join, normalize } from "node:path";

const root = new URL("../dist/", import.meta.url).pathname;
const port = Number(process.env.PORT ?? 4329);
const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".css": "text/css",
  ".js": "text/javascript",
  ".svg": "image/svg+xml",
  ".png": "image/png",
};

createServer((req, res) => {
  let path;
  try {
    path = normalize(decodeURIComponent(new URL(req.url ?? "/", "http://x").pathname));
  } catch {
    res.writeHead(400).end("bad request");
    return;
  }
  let file = join(root, path);
  try {
    if (statSync(file).isDirectory()) file = join(file, "index.html");
    statSync(file);
  } catch {
    res.writeHead(404).end("not found");
    return;
  }
  res.writeHead(200, { "content-type": TYPES[extname(file)] ?? "application/octet-stream" });
  createReadStream(file).pipe(res);
  // Loopback only: this serves an unpublished build.
}).listen(port, "127.0.0.1", () => console.log(`serving dist on http://127.0.0.1:${port}`));
