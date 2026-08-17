// What `<space>f` offers (issue #74). Two things the wasm32 file-picker walk
// has to get right over a flat key space: the files boot seeds are build
// artifacts and must not be offered (while staying openable by name), and the
// `file-picker.*` options that mean something here — `hidden` and
// `max-depth` — must actually be consulted.
//
// These read the rendered terminal rather than editor state: the picker's
// candidate list is a compositor layer with no `helixState` surface, so the
// screen is the only place the answer appears (see the note on
// `terminalText` in ./helpers.js).
import { test, expect } from "@playwright/test";
import { bootEditor, getState, getText, terminalText } from "./helpers.js";

// Every seeded key is under the runtime directory and nothing else on screen
// is, so this substring is present exactly when the picker is offering them.
// It survives the path column's left-truncation of the longest of them
// (`…onfig/helix/runtime/themes/everforest_dark.toml`), which a whole path
// would not.
const SEED_MARKER = "helix/runtime/";
const THE_SEEDED_TUTOR = "/.config/helix/runtime/tutor";

// Opens the picker and waits for it to have drawn. Every negative assertion
// below needs this first: `not.toContain` on a screen the picker has not
// reached yet passes for the wrong reason.
async function openPicker(page) {
  await page.keyboard.press(" ");
  await page.keyboard.press("f");
  await expect
    .poll(() => terminalText(page), { message: "file picker did not open" })
    .toContain("example.rs");
}

async function closePicker(page) {
  await page.keyboard.press("Escape");
  await expect.poll(() => getState(page).then((s) => s.mode)).toBe("normal");
}

// `:set <option> <value>`, which is session-only here (there is no config
// file to read on wasm32) and so is the only way to reach these options.
async function set(page, option, value) {
  await page.keyboard.type(`:set ${option} ${value}`);
  await page.keyboard.press("Enter");
}

test("the picker does not offer the files boot seeds", async ({ page }) => {
  await bootEditor(page);
  await openPicker(page);

  const screen = await terminalText(page);
  // The ten bundled themes and the tutor text: everything the port writes
  // into the runtime directory at boot (web/src/themes.rs, web/src/session.rs).
  expect(screen).not.toContain(SEED_MARKER);
  // The sample files are the point of the list, and still in it.
  expect(screen).toContain("welcome.txt");
});

test("a seeded runtime file is still openable by name", async ({ page }) => {
  await bootEditor(page);

  // Not offered is not the same as not there — `:tutor` and `:theme` read
  // these keys out of the same store the picker declines to list.
  await page.keyboard.type(`:o ${THE_SEEDED_TUTOR}`);
  await page.keyboard.press("Enter");

  await expect
    .poll(() => getState(page).then((s) => s.path))
    .toBe(THE_SEEDED_TUTOR);
  await expect.poll(() => getText(page)).toContain("Helix");
});

test("file-picker.hidden is honored, and does not bring the seeded files back", async ({
  page,
}) => {
  await bootEditor(page);
  await page.evaluate(() => window.helixVfs.write("/.hidden.txt", "secret"));

  await openPicker(page);
  expect(await terminalText(page)).not.toContain(".hidden.txt");
  await closePicker(page);

  await set(page, "file-picker.hidden", "false");
  await openPicker(page);
  const screen = await terminalText(page);
  expect(screen).toContain(".hidden.txt");
  // The seed filter is not the hidden filter wearing a hat: the runtime
  // directory is dotted, so honoring `hidden` alone would have hidden it and
  // turning `hidden` off would put every theme back on the list.
  expect(screen).not.toContain(SEED_MARKER);
});

test("file-picker.max-depth is honored", async ({ page }) => {
  await bootEditor(page);
  await page.evaluate(() => window.helixVfs.write("/deep/a/b.txt", "deep"));

  await openPicker(page);
  expect(await terminalText(page)).toContain("deep/a/b.txt");
  await closePicker(page);

  // Depth is counted from the picker's root, as `WalkBuilder` counts it: at
  // 1 only the keys directly in the working directory survive.
  await set(page, "file-picker.max-depth", "1");
  await openPicker(page);
  const screen = await terminalText(page);
  expect(screen).not.toContain("deep/a/b.txt");
  expect(screen).toContain("welcome.txt");
});
