import { defineConfig } from "vite";

export default defineConfig({
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
