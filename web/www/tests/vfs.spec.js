// Document-IO behavior against the in-memory virtual file system
// (`helix_stdx::vfs`). Where a command's native implementation reaches for
// `std::fs`, these assert the wasm32 arm keeps the *store* and the buffer in
// agreement — the failure mode is silent divergence, which the terminal
// never shows, so everything here reads through `helixVfs`.
import { test, expect } from "@playwright/test";
import {
  bootEditor,
  getState,
  getText,
  terminalText,
  vfsList,
  vfsRead,
} from "./helpers.js";

// Save the current buffer under `path` and wait for the async save queue.
async function saveAs(page, path) {
  await page.keyboard.type(`:w ${path}`);
  await page.keyboard.press("Enter");
  await expect.poll(() => getState(page).then((s) => s.path)).toBe(path);
  await expect.poll(() => vfsRead(page, path)).not.toBeUndefined();
}

test(":move renames the vfs key instead of leaving a copy behind (issue #72)", async ({
  page,
}) => {
  await bootEditor(page);

  await page.keyboard.press("i");
  await page.keyboard.type("move me");
  await page.keyboard.press("Escape");
  await saveAs(page, "/a.txt");
  expect(await vfsRead(page, "/a.txt")).toContain("move me");

  await page.keyboard.type(":move /b.txt");
  await page.keyboard.press("Enter");
  await expect.poll(() => getState(page).then((s) => s.path)).toBe("/b.txt");

  // The whole point of the issue: before the fix the buffer retargeted while
  // the contents stayed at the old key, so the file existed twice — once
  // stale, once not at all — with nothing on screen to say so.
  await expect.poll(() => vfsRead(page, "/b.txt")).toContain("move me");
  expect(await vfsRead(page, "/a.txt")).toBeUndefined();
  const listed = await vfsList(page);
  expect(listed).toContain("/b.txt");
  expect(listed).not.toContain("/a.txt");

  // Renaming onto the path it already has is helix's own early return, and
  // must not consume the file on the way through.
  await page.keyboard.type(":move /b.txt");
  await page.keyboard.press("Enter");
  expect(await getState(page).then((s) => s.path)).toBe("/b.txt");
  expect(await vfsRead(page, "/b.txt")).toContain("move me");
});

test(":move carries the last-saved copy and leaves the buffer modified", async ({
  page,
}) => {
  await bootEditor(page);

  await page.keyboard.press("i");
  await page.keyboard.type("saved");
  await page.keyboard.press("Escape");
  await saveAs(page, "/m.txt");

  // Edit past the save, so the buffer and the store disagree the way they do
  // natively when you `:move` a file with unsaved changes.
  await page.keyboard.press("A");
  await page.keyboard.type(" plus more");
  await page.keyboard.press("Escape");
  await expect.poll(() => getText(page)).toContain("plus more");

  await page.keyboard.type(":move /moved.txt");
  await page.keyboard.press("Enter");
  await expect.poll(() => getState(page).then((s) => s.path)).toBe("/moved.txt");

  // What moved is what was stored — the unsaved edit is still only in the
  // buffer, exactly as `fs::rename` would leave it.
  await expect.poll(() => vfsRead(page, "/moved.txt")).toContain("saved");
  expect(await vfsRead(page, "/moved.txt")).not.toContain("plus more");
  expect(await vfsRead(page, "/m.txt")).toBeUndefined();

  // The buffer is still modified and now points at the new key, so a plain
  // `:w` converges the two there.
  await page.keyboard.type(":w");
  await page.keyboard.press("Enter");
  await expect
    .poll(async () => vfsRead(page, "/moved.txt"))
    .toBe(await getText(page));
});

test(":move of a buffer that was never saved moves the buffer only", async ({
  page,
}) => {
  await bootEditor(page);

  // `:o` on a key that isn't in the store opens an empty buffer at that path
  // — the vfs counterpart of opening a file that doesn't exist yet, which
  // native helix also renames without creating anything.
  await page.keyboard.type(":o /ghost.txt");
  await page.keyboard.press("Enter");
  await expect.poll(() => getState(page).then((s) => s.path)).toBe("/ghost.txt");
  const before = await vfsList(page);
  expect(before).not.toContain("/ghost.txt");

  await page.keyboard.type(":move /ghost-moved.txt");
  await page.keyboard.press("Enter");
  await expect
    .poll(() => getState(page).then((s) => s.path))
    .toBe("/ghost-moved.txt");
  expect(await vfsList(page)).toEqual(before);

  // And the first save lands under the new name, not the old one.
  await page.keyboard.press("i");
  await page.keyboard.type("now it exists");
  await page.keyboard.press("Escape");
  await page.keyboard.type(":w");
  await page.keyboard.press("Enter");
  await expect
    .poll(() => vfsRead(page, "/ghost-moved.txt"))
    .toContain("now it exists");
  expect(await vfsRead(page, "/ghost.txt")).toBeUndefined();
});

test(":move onto an existing key replaces it, as rename(2) does", async ({
  page,
}) => {
  await bootEditor(page);

  await page.evaluate(() => window.helixVfs.write("/target.txt", "clobbered"));

  await page.keyboard.press("i");
  await page.keyboard.type("winner");
  await page.keyboard.press("Escape");
  await saveAs(page, "/source.txt");

  await page.keyboard.type(":move /target.txt");
  await page.keyboard.press("Enter");
  await expect
    .poll(() => getState(page).then((s) => s.path))
    .toBe("/target.txt");

  await expect.poll(() => vfsRead(page, "/target.txt")).toContain("winner");
  expect(await vfsRead(page, "/source.txt")).toBeUndefined();
});

test(":move onto a directory-shaped key renames to it, not into it", async ({
  page,
}) => {
  await bootEditor(page);

  await page.keyboard.press("i");
  await page.keyboard.type("payload");
  await page.keyboard.press("Escape");
  await saveAs(page, "/dir/file.txt");

  // There are no directories in the store, only keys with slashes in them,
  // so `move_buffer`'s `new_path.is_dir()` test is false and it never
  // appends the original file name the way native helix would. `/dir`
  // becomes an ordinary sibling key holding the contents — this is the
  // docs/limitations.md "There are no directories" entry, pinned.
  await page.keyboard.type(":move /dir");
  await page.keyboard.press("Enter");
  await expect.poll(() => getState(page).then((s) => s.path)).toBe("/dir");

  await expect.poll(() => vfsRead(page, "/dir")).toContain("payload");
  expect(await vfsRead(page, "/dir/file.txt")).toBeUndefined();
});

test(":move to a path the vfs cannot store fails without moving anything", async ({
  page,
}) => {
  await bootEditor(page);

  await page.keyboard.press("i");
  await page.keyboard.type("untouched");
  await page.keyboard.press("Escape");
  await saveAs(page, "/keep.txt");

  // `/` names no file, so the store refuses the key rather than growing an
  // undeletable one. The error has to arrive before anything is dropped.
  await page.keyboard.type(":move /");
  await page.keyboard.press("Enter");
  await expect.poll(() => terminalText(page)).toContain("Could not move file");

  expect(await getState(page).then((s) => s.path)).toBe("/keep.txt");
  expect(await vfsRead(page, "/keep.txt")).toContain("untouched");
});

test(":w refuses to overwrite an embedder's write, and :w! forces it (issue #76)", async ({
  page,
}) => {
  await bootEditor(page);

  await page.keyboard.press("i");
  await page.keyboard.type("editor version");
  await page.keyboard.press("Escape");
  await saveAs(page, "/guard.txt");

  // The embedder writes through the vfs hooks after the buffer last saved.
  // Before the fix the store held no times to compare, so the save below
  // overwrote this with no warning and `:w!` did exactly what `:w` did.
  await page.evaluate(() =>
    window.helixVfs.write("/guard.txt", "embedder version"),
  );

  await page.keyboard.press("A");
  await page.keyboard.type(" plus");
  await page.keyboard.press("Escape");
  await page.keyboard.type(":w");
  await page.keyboard.press("Enter");

  await expect
    .poll(() => terminalText(page))
    .toContain("file modified by an external process");
  // Refused means nothing was written: the embedder's copy is still there.
  expect(await vfsRead(page, "/guard.txt")).toBe("embedder version");

  // `:w!` is the override the message names, and it lands.
  await page.keyboard.type(":w!");
  await page.keyboard.press("Enter");
  await expect.poll(() => vfsRead(page, "/guard.txt")).toContain("plus");
  expect(await vfsRead(page, "/guard.txt")).not.toContain("embedder version");
});

test(":w straight after opening a seeded file is not an external change (issue #76)", async ({
  page,
}) => {
  await bootEditor(page);

  await page.evaluate(() => window.helixVfs.write("/seeded.txt", "seeded\n"));
  await page.keyboard.type(":o /seeded.txt");
  await page.keyboard.press("Enter");
  await expect
    .poll(() => getState(page).then((s) => s.path))
    .toBe("/seeded.txt");

  // A false "modified by an external process" here would be worse than the
  // bug the guard fixes. It cannot happen because the buffer's last-saved
  // time is that key's stored stamp rather than a reading of the clock taken
  // at open — so this compares a time against itself.
  await page.keyboard.press("i");
  await page.keyboard.type("edited ");
  await page.keyboard.press("Escape");
  await page.keyboard.type(":w");
  await page.keyboard.press("Enter");
  await expect.poll(() => vfsRead(page, "/seeded.txt")).toContain("edited ");
  expect(await terminalText(page)).not.toContain("external process");

  // And again straight after, which only works because that save picked its
  // new last-saved time back up out of the store.
  await page.keyboard.press("A");
  await page.keyboard.type("more");
  await page.keyboard.press("Escape");
  await page.keyboard.type(":w");
  await page.keyboard.press("Enter");
  await expect.poll(() => vfsRead(page, "/seeded.txt")).toContain("more");
  expect(await terminalText(page)).not.toContain("external process");
});

test(":w onto a boot-seeded sample from the boot buffer is allowed (issue #76)", async ({
  page,
}) => {
  await bootEditor(page);

  // Boot order is load-bearing now and nothing else would catch it changing:
  // `session.rs` seeds the themes, the tutor and `samples::seed()` before
  // `Application::new`, so the boot buffer's last-saved time is taken after
  // every seed's stamp and this save is not an external modification. Seed
  // below the boot instead and the demo's first `:w` — the flow the seeded
  // `/welcome.txt` invites — would start refusing.
  await page.keyboard.press("i");
  await page.keyboard.type("scratch content");
  await page.keyboard.press("Escape");
  await page.keyboard.type(":w /example.rs");
  await page.keyboard.press("Enter");

  await expect
    .poll(() => vfsRead(page, "/example.rs"))
    .toContain("scratch content");
  expect(await terminalText(page)).not.toContain("external process");
});

// A directory in the store is a prefix its keys share, not an entry of its
// own — so these seed keys and then ask the editor what it can see under
// them.
async function seedTree(page) {
  await page.evaluate(() => {
    window.helixVfs.write("/proj/alpha.txt", "alpha");
    window.helixVfs.write("/proj/beta.txt", "beta");
    window.helixVfs.write("/proj/deep/gamma.txt", "gamma");
    window.helixVfs.write("/proj/.dotfile.txt", "dot");
    window.helixVfs.write("/top.txt", "top");
  });
}

// Type `line` at the prompt and wait for its completion menu to settle.
// Completions recalculate per keystroke and the terminal redraws on its own
// schedule, so the assertion has to poll the screen rather than read it once.
async function promptWith(page, line) {
  await page.keyboard.type(line);
  return expect.poll(() => terminalText(page));
}

test(":pwd reports the working directory instead of calling it deleted (issue #73)", async ({
  page,
}) => {
  await bootEditor(page);

  await page.keyboard.type(":pwd");
  await page.keyboard.press("Enter");
  await expect
    .poll(() => terminalText(page))
    .toContain("Current working directory is /");
  // The bug: `Path::exists` asks a file system that isn't there, so the
  // check never passed and every `:pwd` was an error about a deleted
  // directory. Nothing in the store can delete a working directory — it has
  // no directories to delete.
  expect(await terminalText(page)).not.toContain("deleted");

  // Still true of a directory no key lives under: `:cd` accepts it (there is
  // no mkdir here, so that is where a first `:w` would land), and `:pwd`
  // reports it rather than declaring it gone.
  await page.keyboard.type(":cd /proj");
  await page.keyboard.press("Enter");
  await page.keyboard.type(":pwd");
  await page.keyboard.press("Enter");
  await expect
    .poll(() => terminalText(page))
    .toContain("Current working directory is /proj");
  expect(await terminalText(page)).not.toContain("deleted");

  // And the rest of that claim: a relative `:w` is what puts the first key
  // under such a directory, which is why `:cd` still accepts one.
  await page.keyboard.press("i");
  await page.keyboard.type("first file");
  await page.keyboard.press("Escape");
  await page.keyboard.type(":w rel.txt");
  await page.keyboard.press("Enter");
  await expect
    .poll(() => vfsRead(page, "/proj/rel.txt"))
    .toContain("first file");
});

test("path completion offers the vfs keys, with prefixes as directories (issue #73)", async ({
  page,
}) => {
  await bootEditor(page);
  await seedTree(page);

  // `filename_impl` walked the real filesystem, so before the fix every one
  // of these offered nothing at all.
  await (await promptWith(page, ":o /proj/")).toContain("alpha.txt");
  const underProj = await terminalText(page);
  expect(underProj).toContain("beta.txt");
  // `deep` holds no key of its own; it exists only because keys continue
  // past it, and the trailing separator says the match can be extended.
  expect(underProj).toContain("deep/");
  // Depth 1, as the native walk is: what is inside `deep` is not offered yet.
  expect(underProj).not.toContain("gamma.txt");
  // Nothing here is on disk to be hidden or gitignored, so a leading dot
  // hides nothing either — the native walk runs with `hidden(false)` too.
  expect(underProj).toContain(".dotfile.txt");

  await page.keyboard.press("Escape");
  await (await promptWith(page, ":o /to")).toContain("top.txt");

  await page.keyboard.press("Escape");
  await (await promptWith(page, ":o /pro")).toContain("proj/");
  await page.keyboard.press("Escape");
});

test("path completion resolves relative input against the working directory", async ({
  page,
}) => {
  await bootEditor(page);
  await seedTree(page);

  await page.keyboard.type(":cd /proj");
  await page.keyboard.press("Enter");

  // No leading slash: the candidates come from `/proj`, the directory `:cd`
  // moved to, and not from the root the session booted at.
  await (await promptWith(page, ":o al")).toContain("alpha.txt");
  const relative = await terminalText(page);
  expect(relative).not.toContain("top.txt");

  // And accepting the completion opens the key it named.
  await page.keyboard.press("Tab");
  await page.keyboard.press("Enter");
  await expect
    .poll(() => getState(page).then((s) => s.path))
    .toBe("/proj/alpha.txt");
  expect(await getText(page)).toContain("alpha");
});

test(":cd completes directories only", async ({ page }) => {
  await bootEditor(page);
  await seedTree(page);

  await (await promptWith(page, ":cd /pro")).toContain("proj");
  // `/top.txt` is a key, not a prefix anything extends, so it is not a
  // directory and `:cd` must not offer it.
  expect(await terminalText(page)).not.toContain("top.txt");
  await page.keyboard.press("Escape");
});

test(":read inserts a vfs file at the selection (issue #96)", async ({
  page,
}) => {
  await bootEditor(page);
  await page.evaluate(() => window.helixVfs.write("/insert.txt", "INSERTED"));

  await page.keyboard.press("i");
  await page.keyboard.type("AB");
  await page.keyboard.press("Escape");
  // `gg` leaves the cursor on `A`, so the selection head sits between the two
  // characters: the contents land there and the text either side of the
  // insertion point has to survive. Insertion is helix's own — only where the
  // bytes come from is what wasm32 changes.
  await page.keyboard.type("gg");

  // The bug: `path.exists() && path.is_file()` asked the real filesystem
  // about a vfs key, so the guard rejected every path — including this one,
  // which the store definitely holds — before the open was even reached.
  await page.keyboard.type(":r /insert.txt");
  await page.keyboard.press("Enter");
  await expect.poll(() => getText(page)).toContain("AINSERTEDB");
  expect(await terminalText(page)).not.toContain("path is not a file");

  // Reading is not a modification of the file it read: the store still holds
  // exactly what it did, and only the buffer grew.
  expect(await vfsRead(page, "/insert.txt")).toBe("INSERTED");
});

test(":read resolves a relative path against the working directory", async ({
  page,
}) => {
  await bootEditor(page);
  await seedTree(page);

  await page.keyboard.type(":cd /proj");
  await page.keyboard.press("Enter");

  await page.keyboard.type(":r beta.txt");
  await page.keyboard.press("Enter");
  await expect.poll(() => getText(page)).toContain("beta");
});

test(":read refuses a key the vfs does not hold", async ({ page }) => {
  await bootEditor(page);

  await page.keyboard.press("i");
  await page.keyboard.type("untouched");
  await page.keyboard.press("Escape");
  const before = await getText(page);

  await page.keyboard.type(":r /nope.txt");
  await page.keyboard.press("Enter");
  await expect
    .poll(() => terminalText(page))
    .toContain('path is not a file: "/nope.txt"');
  expect(await getText(page)).toBe(before);
});

test(":read refuses a directory-shaped path, unless a key sits there too", async ({
  page,
}) => {
  await bootEditor(page);
  await seedTree(page);

  const before = await getText(page);

  // `/proj` holds no key of its own — it is only a prefix the keys under it
  // extend — so there is nothing to read, which is also the answer native
  // helix's `is_file()` gives for a real directory.
  await page.keyboard.type(":r /proj");
  await page.keyboard.press("Enter");
  await expect
    .poll(() => terminalText(page))
    .toContain('path is not a file: "/proj"');
  expect(await getText(page)).toBe(before);

  // A key that is *also* a prefix is a file and does read — the store can
  // produce that pair (`vfs::read_dir` documents it) and a file system
  // cannot, so this is the one place the two differ.
  await page.evaluate(() => window.helixVfs.write("/proj", "prefix and key"));
  await page.keyboard.type(":r /proj");
  await page.keyboard.press("Enter");
  await expect.poll(() => getText(page)).toContain("prefix and key");
});
