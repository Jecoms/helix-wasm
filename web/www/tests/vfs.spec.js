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
  // README's "There are no directories" entry, pinned.
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
