// Shared plumbing for the browser suites (smoke.spec.js, tutor.spec.js, ...):
// boot, and the three read surfaces the specs assert through. Not a spec —
// Playwright's default testMatch only picks up `*.spec.js`.
import { expect } from "@playwright/test";

// Read-only editor state (issue #18): `state()` is the structured view
// (mode, path, cursor, selections), `text()` the focused buffer's live text.
export const getState = (page) => page.evaluate(() => window.helixState.state());

export const getText = (page) => page.evaluate(() => window.helixState.text());

export const vfsRead = (page, path) =>
  page.evaluate((p) => window.helixVfs.read(p), path);

// Every key in the virtual file system, minus the ones boot seeds (the
// runtime themes and the tutor text under `/.config/helix/runtime`), so a
// spec can assert on the whole store without restating the seed set.
export const vfsList = (page) =>
  page
    .evaluate(() => window.helixVfs.list())
    .then((paths) =>
      paths.filter((path) => !path.startsWith("/.config/helix/runtime/")),
    );

// The rendered terminal, as text. Reach for this only where the screen is
// the only place the answer appears: pixels being the point (boot proving a
// statusline drew at all, the theme tests proving a theme painted the
// screen), state the inspection API does not report (the tutor split tests
// counting views — it sees the focused view only), or a statusline message,
// which has no `helixState` surface at all and is the sole evidence a
// command ran and said something (the `jobs.spec.js` errors, the refused
// `:move` in `vfs.spec.js`). Everything else reads editor state, not pixels:
// the state-over-scraping rule from the issue #18 inspection API.
export const terminalText = (page) =>
  page.evaluate(() => {
    const buffer = window.__helixTerminal.buffer.active;
    const lines = [];
    for (let i = 0; i < buffer.length; i += 1) {
      lines.push(buffer.getLine(i).translateToString(true));
    }
    return lines.join("\n");
  });

// The top-left cell's background as an RGB number, or -1 while it still has
// the terminal's (non-RGB) default background. Theme tests only — see the
// note on terminalText.
export const topLeftBg = (page) =>
  page.evaluate(() => {
    const cell = window.__helixTerminal.buffer.active.getLine(0).getCell(0);
    return cell.isBgRGB() ? cell.getBgColor() : -1;
  });

// Boot with `config` as the page's `config.toml` (issue #75). The host page
// reads `window.helixConfig` and hands it to `start`, which seeds it where
// helix reads the user config from — so this has to be set before the module
// runs, which is what `addInitScript` is for.
export async function bootWithConfig(page, config, options) {
  await page.addInitScript((text) => {
    window.helixConfig = text;
  }, config);
  await bootEditor(page, options);
}

// Wait out the wasm fetch + instantiation, then make sure keystrokes land in
// xterm's hidden textarea. `focus: false` skips that click, for the one
// assertion it would invalidate: helix clears the statusline on the first
// event it handles, and a click is an event.
export async function bootEditor(page, { focus = true } = {}) {
  await page.goto("/");
  await expect
    .poll(async () => page.evaluate(() => window.helixState?.state()?.mode), {
      message: "editor did not reach normal mode after boot",
      timeout: 30_000,
    })
    .toBe("normal");
  if (focus) {
    await page.locator("#terminal").click();
  }
}
