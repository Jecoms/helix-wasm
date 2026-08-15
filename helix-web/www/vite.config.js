import { defineConfig } from "vite";

export default defineConfig({
  // The demo is deployed under a sub-path (gh-pages `/helix/demo`), so emit
  // relative asset URLs.
  base: "./",
  build: {
    // Don't transpile the wasm-bindgen glue below what wasm-bindgen targets.
    target: "esnext",
  },
  resolve: {
    // The wasm-bindgen glue in `../pkg` imports the xterm packages by bare
    // specifier; resolve them from this app's node_modules (the glue is
    // reached through a `file:` symlink and has no node_modules of its own).
    dedupe: ["xterm", "xterm-addon-fit", "xterm-addon-webgl"],
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
