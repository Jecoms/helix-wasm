import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

// `web/NOTICE.md` (grammar attribution for the statically linked wasm
// bundle) is the source of truth; emit a copy into dist so the notice
// ships with the distributed assets, at a stable path next to the page
// (<site root>/NOTICE.txt). Emitted as `.txt` so it is served as
// text/plain and displays inline in every browser (GitHub Pages would
// serve `.md` as text/markdown, which some browsers download instead).
const noticePath = fileURLToPath(new URL("../NOTICE.md", import.meta.url));

function thirdPartyNotices() {
  return {
    name: "third-party-notices",
    buildStart() {
      this.addWatchFile(noticePath);
    },
    generateBundle() {
      this.emitFile({
        type: "asset",
        fileName: "NOTICE.txt",
        source: readFileSync(noticePath, "utf8"),
      });
    },
  };
}

export default defineConfig({
  plugins: [thirdPartyNotices()],
  // The demo will deploy under a sub-path (gh-pages), so emit relative asset
  // URLs.
  base: "./",
  build: {
    // Don't transpile the wasm-bindgen glue below what wasm-bindgen targets.
    target: "esnext",
  },
  optimizeDeps: {
    // The wasm-bindgen glue loads `helix_web_bg.wasm` via
    // `new URL(..., import.meta.url)`; esbuild pre-bundling would break that.
    exclude: ["helix-web"],
  },
  server: {
    fs: {
      // `helix-web` is a `file:../pkg` dependency, outside the Vite root.
      allow: [".."],
    },
  },
});
