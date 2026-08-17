import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

// The deploy is a distribution of the bundle, so it carries the bundle's
// terms: `web/NOTICE.md` (attribution for the statically linked wasm) and the
// repository's MPL-2.0 `LICENSE`, which the editor code itself — helix's files
// and this port's — is under. Both live elsewhere in the tree as the source of
// truth; emit copies into dist so they ship with the distributed assets, at
// stable paths next to the page (<site root>/NOTICE.txt, LICENSE.txt).
// Emitted as `.txt` so they are served as text/plain and display inline in
// every browser: GitHub Pages would serve `.md` as text/markdown, which some
// browsers download instead of showing, and an extensionless `LICENSE` has no
// extension to be typed by at all.
const legalFiles = [
  ["NOTICE.txt", fileURLToPath(new URL("../NOTICE.md", import.meta.url))],
  ["LICENSE.txt", fileURLToPath(new URL("../../LICENSE", import.meta.url))],
];

function legalNotices() {
  return {
    name: "legal-notices",
    buildStart() {
      for (const [, path] of legalFiles) {
        this.addWatchFile(path);
      }
    },
    generateBundle() {
      for (const [fileName, path] of legalFiles) {
        this.emitFile({
          type: "asset",
          fileName,
          source: readFileSync(path, "utf8"),
        });
      }
    },
  };
}

export default defineConfig({
  plugins: [legalNotices()],
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
