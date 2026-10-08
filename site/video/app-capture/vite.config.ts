// Serves the real porch app UI (app/src) in a plain browser with Tauri IPC mocked by fixtures,
// so videos show the actual desktop app screens without anyone's real sessions or costs.
import react from "@vitejs/plugin-react";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const appRoot = fileURLToPath(new URL("../../../app/", import.meta.url));
// Tailwind resolves its `content` globs from the working directory; run as if from app/.
process.chdir(appRoot);
const here = fileURLToPath(new URL("./", import.meta.url));
const fromApp = createRequire(appRoot + "package.json");
const tailwind = fromApp("tailwindcss");
const autoprefixer = fromApp("autoprefixer");

export default defineConfig({
  root: appRoot,
  plugins: [
    react(),
    {
      name: "porch-capture-mock",
      transformIndexHtml(html) {
        return html.replace(
          '<script type="module" src="/src/main.tsx">',
          `<script type="module" src="/@fs${here}mock.ts"></script>\n    <script type="module" src="/src/main.tsx">`,
        );
      },
    },
  ],
  // One copy of @tauri-apps/api for the app and the mock, so mockIPC patches the invoke the app calls.
  resolve: { alias: { "@tauri-apps/api": appRoot + "node_modules/@tauri-apps/api" } },
  css: { postcss: { plugins: [tailwind({ config: appRoot + "tailwind.config.js" }), autoprefixer()] } },
  server: { port: Number(process.env.CAPTURE_PORT ?? 1431), strictPort: true, fs: { allow: [appRoot, here] } },
});
