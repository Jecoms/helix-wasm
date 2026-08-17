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

// The directory boot seeds into (web/src/themes.rs, web/src/session.rs, and
// `helix_loader::runtime_dirs` on the rust side; `vfsList` in ./helpers.js
// filters the same prefix). `seededKeys` reads it back out of the store,
// because every negative assertion below is worthless without it: a stale
// marker would match nothing whether the picker filtered or not.
const SEED_DIR = "/.config/helix/runtime/";
const THE_SEEDED_TUTOR = `${SEED_DIR}tutor`;

const seededKeys = (page) =>
  page
    .evaluate(() => window.helixVfs.list())
    .then((paths) => paths.filter((path) => path.startsWith(SEED_DIR)));

// The substring a seeded key renders as in the picker's path column. Not the
// whole path: the column truncates from the left, so the longest of them
// arrives as `…onfig/helix/runtime/themes/everforest_dark.toml`.
const SEED_MARKER = "helix/runtime/";

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

// The picker is a compositor layer, not a mode — `helixState` reports
// `normal` throughout — so the only evidence it has gone is the screen no
// longer carrying its list.
async function closePicker(page) {
  await page.keyboard.press("Escape");
  await expect
    .poll(() => terminalText(page), { message: "file picker did not close" })
    .not.toContain("example.rs");
}

// `:set <option> <value>`, which is session-only here (there is no config
// file to read on wasm32) and so is the only way to reach these options.
async function set(page, option, value) {
  await page.keyboard.type(`:set ${option} ${value}`);
  await page.keyboard.press("Enter");
}

test("the picker does not offer the files boot seeds", async ({ page }) => {
  await bootEditor(page);

  // There is something to filter: the ten bundled themes and the tutor text.
  // Without this the assertion below would go green on a picker that had
  // stopped filtering and a seed set that had moved.
  expect(await seededKeys(page)).toContain(THE_SEEDED_TUTOR);
  expect((await seededKeys(page)).length).toBeGreaterThan(10);

  await openPicker(page);
  const screen = await terminalText(page);
  expect(screen).not.toContain(SEED_MARKER);
  // The sample files are the point of the list, and still in it.
  expect(screen).toContain("welcome.txt");
});

test("the picker offers the seeded files from inside the runtime directory", async ({
  page,
}) => {
  await bootEditor(page);

  // Not offered where nothing asked for them is not the same as unreachable:
  // `:cd`-ing in is asking by name, and an empty list would be a dead end in
  // a directory with eleven files in it.
  await page.keyboard.type(`:cd ${SEED_DIR}themes`);
  await page.keyboard.press("Enter");

  await page.keyboard.press(" ");
  await page.keyboard.press("f");
  await expect.poll(() => terminalText(page)).toContain("gruvbox.toml");
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
