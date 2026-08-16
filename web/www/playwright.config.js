import { defineConfig, devices } from "@playwright/test";

// Browser smoke tests for the built bundle (issue #44). The suite runs
// against `dist/` — the same artifact CI and the Pages deploy ship — so run
// `npm run build` (after `wasm-pack build web --target web`) before
// `npm test`.
export default defineConfig({
  testDir: "./tests",
  // The editor is a singleton per page, but each test gets its own page, so
  // tests are independent; still run them serially — a single shared vite
  // preview server keeps the wasm fetch warm and workers buy nothing here.
  workers: 1,
  forbidOnly: !!process.env.CI,
  // Post-action polls wait on wasm-side async work (boot, the save queue);
  // give them headroom beyond the 5s default for slow CI runners.
  expect: { timeout: 10_000 },
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : "list",
  use: {
    baseURL: "http://localhost:4173",
    trace: "on-first-retry",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
  webServer: {
    command: "npm run preview",
    url: "http://localhost:4173",
    reuseExistingServer: !process.env.CI,
  },
});
