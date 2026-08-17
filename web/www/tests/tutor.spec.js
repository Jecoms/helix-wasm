// Regression guards for the browser concessions the `:tutor` audit
// (issue #65) produced. The tutorial text is helix's own file, embedded
// verbatim from helix/runtime/tutor (see web/runtime/README.md), so every
// concession lives outside it: an exit notice, the upstream `space w` alias
// for the browser-reserved `C-w`, and sample files for the picker. These
// tests cover only those contested steps, not all 60 tutor sections.
//
// Same shape as smoke.spec.js, sharing its plumbing (./helpers.js): state
// comes from `window.helixState` (issue #18), and post-action assertions
// poll, since the editor's work is asynchronous.
import { test, expect } from "@playwright/test";
import { bootEditor, getState, getText, terminalText } from "./helpers.js";

// How many views are on screen, counted by their statuslines. The
// inspection API reports the focused view only, so splits — the whole
// subject of tutor chapter 13 — can only be seen in the rendered output.
const viewCount = async (page) =>
  ((await terminalText(page)).match(/\[scratch\]/g) || []).length;

// Types a chord sequence one key at a time. No settle between keys: each
// `key_event` pushes onto the bridge queue synchronously, so the editor
// sees them in call order and a pending-key prefix (space, m, ...) resolves
// against the next key whatever the timing.
async function press(page, ...keys) {
  for (const key of keys) {
    await page.keyboard.press(key);
  }
}

test("tutor 1.2: :q exits with a notice and an exit callback, not a panic", async ({
  page,
}) => {
  const panics = [];
  page.on("pageerror", (error) => panics.push(String(error.message)));
  page.on("console", (message) => {
    if (message.text().includes("panicked")) panics.push(message.text());
  });

  await bootEditor(page);
  await page.keyboard.type(":q");
  await page.keyboard.press("Enter");

  // The exit callback is the embedder-facing half of the concession.
  await expect.poll(() => page.evaluate(() => window.helixExit)).toEqual({
    code: 0,
  });
  // The reader-facing half: `:q` in a browser tab has no shell to return
  // to, so a dead editor would otherwise be indistinguishable from a frozen
  // page. (The crash path to the same symptom is guarded in smoke.spec.js.)
  await expect
    .poll(() => terminalText(page))
    .toContain("Helix has exited. Refresh the page to start a new session.");
  await expect.poll(() => terminalText(page)).toContain("(exit code 0)");

  // Quitting used to trap in the wasm module (`Editor::close_language_servers`
  // builds a tokio timer, and tokio's clock is std's, which is unimplemented
  // on wasm32) — a teardown that panics can never reach the notice above.
  expect(panics).toEqual([]);
  // helix really is gone: inspection reports not-running.
  expect(await getState(page)).toBeUndefined();
});

test("tutor 13.x: the window menu is reachable as `space w`, without C-w", async ({
  page,
}) => {
  await bootEditor(page);
  expect(await viewCount(page)).toBe(1);

  // Chrome and Firefox keep Ctrl-w for closing the tab on Windows and
  // Linux, and a page cannot take it back — so chapter 13 rides on
  // upstream's own `space w` alias for the same menu. `space w n v` is
  // 13.1's "Ctrl-w nv".
  await press(page, " ", "w");
  await expect.poll(() => terminalText(page)).toContain("Vertical right split");

  await press(page, "n", "v");
  await expect.poll(() => viewCount(page)).toBe(2);

  // 13.2 moves between splits, 13.3 closes the extra ones.
  await press(page, " ", "w", "h");
  await press(page, " ", "w", "o");
  await expect.poll(() => viewCount(page)).toBe(1);
});

test("tutor 13.7: the file picker lists sample files and opens one in a split", async ({
  page,
}) => {
  await bootEditor(page);

  // Boot seeds sample files (web/src/samples.rs) because the picker
  // otherwise offers only the vendored runtime files, buried under a dotted
  // config path — nothing 13.7 would have a reader select.
  await press(page, " ", "f");
  await expect.poll(() => terminalText(page)).toContain("example.rs");
  await expect.poll(() => terminalText(page)).toContain("welcome.txt");

  // 13.7 has the reader type to narrow the list. Pin that on the file that
  // has to drop out of it: `example.rs` was already on screen before a key
  // was pressed, so asserting it again would pass with the filter doing
  // nothing at all.
  await page.keyboard.type("example");
  await expect.poll(() => terminalText(page)).not.toContain("welcome.txt");
  await expect.poll(() => terminalText(page)).toContain("example.rs");

  // C-v from the picker opens the selection in a vertical split. Unlike
  // C-w, xterm.js does claim C-v from the browser's paste shortcut, so this
  // step survives as written.
  await page.keyboard.press("Control+v");

  await expect
    .poll(() => getState(page).then((s) => s.path))
    .toBe("/example.rs");
  await expect.poll(() => getText(page)).toContain("fn main()");
  // Split, not replaced: the original scratch view is still on screen
  // alongside the new one.
  await expect
    .poll(async () => {
      const screen = await terminalText(page);
      return screen.includes("[scratch]") && screen.includes("/example.rs");
    })
    .toBe(true);
});
