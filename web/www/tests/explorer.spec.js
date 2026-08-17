// The two directory surfaces that used to probe the real file system
// (issue #105): `<space>e`, which answered "Workspace directory does not
// exist" because `Path::exists` cannot see a virtual root, and `:o <dir>`,
// which gated on `std::fs::canonicalize(..).is_dir()` and so opened an empty
// buffer named after the directory instead of a picker.
//
// Both read the rendered terminal: a picker is a compositor layer with no
// `helixState` surface, so the screen is the only place its list appears (see
// the note on `terminalText` in ./helpers.js). The `:o`-opens-a-file cases at
// the bottom are the exception — those do reach editor state.
import { test, expect } from "@playwright/test";
import { bootEditor, getState, terminalText } from "./helpers.js";

// The two files boot seeds outside the runtime directory (web/src/samples.rs).
const SAMPLE = "welcome.txt";
const SEED_DIR = "/.config/helix/runtime/";

// `<space>e`, rooted at the working directory: `find_workspace` finds no
// `.git`/`.jj`/`.helix` to stop at here, so it falls back to the cwd.
async function openExplorer(page) {
  await page.keyboard.press(" ");
  await page.keyboard.press("e");
  await expect
    .poll(() => terminalText(page), { message: "explorer did not open" })
    .toContain(SAMPLE);
}

// Type into the picker's filter and open the highlighted row. Filtering
// rather than arrowing, so a row that moved does not make the test open the
// wrong one.
//
// `narrowed` is the match counter the filter should leave behind (`1/4`),
// and waiting on it is the whole point of the helper: every filter here is
// typed *towards* a row already on the unfiltered screen, so waiting for the
// filter text to appear would be satisfied before a key was pressed and
// `Enter` could open whichever row happened to be highlighted.
async function pick(page, filter, narrowed) {
  await page.keyboard.type(filter);
  await expect
    .poll(() => terminalText(page), {
      message: `filter did not narrow to ${narrowed}`,
    })
    .toContain(narrowed);
  await page.keyboard.press("Enter");
}

test("<space>e lists the vfs instead of calling the workspace missing", async ({
  page,
}) => {
  await bootEditor(page);
  await openExplorer(page);

  const screen = await terminalText(page);
  // The old symptom, pinned: the explorer never reached its listing.
  expect(screen).not.toContain("Workspace directory does not exist");
  expect(screen).toContain("example.rs");
  expect(screen).toContain(SAMPLE);
  // Directories are the prefixes the keys extend, marked with a separator —
  // the seeded runtime files put `/.config` there.
  expect(screen).toContain(".config/");
  // The root of the store has no parent, so no `..` row, exactly as native
  // helix shows none at `/`.
  expect(screen).not.toContain("../");
});

test("the explorer descends into a prefix and `..` comes back up", async ({
  page,
}) => {
  await bootEditor(page);
  await page.evaluate(() => window.helixVfs.write("/proj/alpha.txt", "a"));

  await openExplorer(page);
  // Four rows at the root — `.config/`, `proj/` and the two samples — and
  // only `proj/` matches.
  await pick(page, "proj", "1/4");

  // Polling on `../` rather than on `alpha.txt`: the root explorer already
  // has `alpha.txt` on screen, in the preview pane beside the highlighted
  // `proj/`, so that would go green before anything descended. A parent row
  // only appears below the root.
  await expect
    .poll(() => terminalText(page), { message: "did not descend into /proj" })
    .toContain("../");
  const inside = await terminalText(page);
  expect(inside).toContain("alpha.txt");
  // The match counter, which the preview pane cannot forge the way it can a
  // file name: two rows here, `../` and the one key under the prefix. The
  // root's four (`.config/`, `proj/` and the two samples) are on the screen
  // below, because `../` highlights first and previews the parent.
  expect(inside).toContain("2/2");

  await pick(page, "..", "1/2");
  // `/proj/..` normalized back to `/` rather than becoming a third level.
  await expect
    .poll(() => terminalText(page), { message: "`..` did not go back up" })
    .toContain("4/4");
  const back = await terminalText(page);
  expect(back).toContain(SAMPLE);
  expect(back).toContain("proj/");
});

test("the explorer on a prefix no key lives under is empty, not an error", async ({
  page,
}) => {
  await bootEditor(page);

  // A flat key space cannot tell an empty directory from a missing one, and
  // the explorer is told which directory to show rather than asked to find
  // one, so the empty listing is the answer for both. `:cd` accepts a
  // directory no key lives under (that is where a first `:w` lands), which
  // is how you get here.
  await page.keyboard.type(":cd /empty");
  await page.keyboard.press("Enter");
  await page.keyboard.press(" ");
  await page.keyboard.press("e");

  await expect
    .poll(() => terminalText(page), { message: "explorer did not open" })
    .toContain("1/1");
  const screen = await terminalText(page);
  expect(screen).not.toContain("does not exist");
  // Its one row is the way back out.
  expect(screen).toContain("../");
});

test("a directory row previews as its listing, not as a missing file", async ({
  page,
}) => {
  await bootEditor(page);
  await page.evaluate(() => window.helixVfs.write("/proj/alpha.txt", "a"));

  await openExplorer(page);
  await page.keyboard.type("proj");

  // The preview pane holds the entries of the highlighted directory. There
  // is no file at `/proj` to read, so before #105 this pane reported the row
  // missing.
  await expect
    .poll(() => terminalText(page), { message: "directory did not preview" })
    .toContain("alpha.txt");
});

test("the explorer shows the seeded runtime files, unlike <space>f", async ({
  page,
}) => {
  await bootEditor(page);

  // `<space>f` drops these (issue #74) because it lists everything below its
  // root at once and the bundled themes would crowd out the rest. The
  // explorer shows one named directory at a time, which is the case #74
  // already makes an exception of, so it filters nothing.
  await page.keyboard.type(`:cd ${SEED_DIR}themes`);
  await page.keyboard.press("Enter");

  await page.keyboard.press(" ");
  await page.keyboard.press("e");
  await expect
    .poll(() => terminalText(page), { message: "explorer did not open" })
    .toContain("gruvbox.toml");
});

test(":o on a directory opens a picker on it", async ({ page }) => {
  await bootEditor(page);
  await page.evaluate(() => {
    window.helixVfs.write("/proj/alpha.txt", "a");
    window.helixVfs.write("/proj/beta.txt", "b");
  });

  await page.keyboard.type(":o /proj/");
  await page.keyboard.press("Enter");

  await expect
    .poll(() => terminalText(page), { message: ":o did not open a picker" })
    .toContain("alpha.txt");
  const screen = await terminalText(page);
  expect(screen).toContain("beta.txt");
  // The old symptom: an ordinary empty buffer named after the directory.
  expect(await getState(page).then((s) => s.path)).not.toBe("/proj");
});

test(":o on a relative directory opens a picker with its files in it", async ({
  page,
}) => {
  await bootEditor(page);
  await page.evaluate(() => {
    window.helixVfs.write("/proj/alpha.txt", "a");
    window.helixVfs.write("/proj/beta.txt", "b");
  });

  // Store keys are absolute, and the picker measures its root against them
  // both to walk and to strip the path column, so a relative root has to be
  // resolved against the working directory first. Native helix never resolves
  // one explicitly — the directory walk does it — which is how this reached
  // the browser as an empty picker.
  await page.keyboard.type(":o proj/");
  await page.keyboard.press("Enter");

  await expect
    .poll(() => terminalText(page), { message: ":o did not open a picker" })
    .toContain("alpha.txt");
  const screen = await terminalText(page);
  expect(screen).toContain("beta.txt");
  // Two matches, not the `0/0` an unresolved root leaves; and the column
  // strips the root, so the rows are bare names.
  expect(screen).toContain("2/2");
  expect(screen).not.toContain("/proj/alpha.txt");
});

test(":o on a key that is also a prefix opens the picker", async ({ page }) => {
  await bootEditor(page);
  // A name a real file system cannot produce, but a flat key space can: both
  // a stored file and the prefix of another key (issue #96). It counts as a
  // directory, descending being the only one of the two things a picker can
  // do with it.
  await page.evaluate(() => {
    window.helixVfs.write("/proj", "stored at the prefix itself");
    window.helixVfs.write("/proj/alpha.txt", "a");
  });

  await page.keyboard.type(":o /proj");
  await page.keyboard.press("Enter");

  await expect
    .poll(() => terminalText(page), { message: ":o did not open a picker" })
    .toContain("alpha.txt");
  // The picker's root is not one of its own entries: listing it would leave
  // the path column with no file name to render, which panicked the editor.
  expect(await terminalText(page)).not.toContain("stopped responding");
});

test(":o on a file and on an unknown path is unchanged", async ({ page }) => {
  await bootEditor(page);

  // A key nothing extends is a file.
  await page.keyboard.type(`:o /${SAMPLE}`);
  await page.keyboard.press("Enter");
  await expect.poll(() => getState(page).then((s) => s.path)).toBe(`/${SAMPLE}`);

  // A path no key touches is neither, so `:o` falls through to the new
  // buffer it opens natively when `canonicalize` fails.
  await page.keyboard.type(":o /nowhere/new.txt");
  await page.keyboard.press("Enter");
  await expect
    .poll(() => getState(page).then((s) => s.path))
    .toBe("/nowhere/new.txt");
});
