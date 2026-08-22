// `:remove` / `:rm` — the one way a file leaves the virtual file system
// (issue #132). The store has no shell to `:sh rm` from, so this is the
// counterpart of `:download`: a key goes out of the store, a buffer closes
// with it, and the host page is asked first. The unregistered-host arm is
// not reachable from here — the demo page registers its handler at boot —
// and is covered by `helix_stdx::remove`'s unit test instead, as for
// `:download`.
import { test, expect } from "@playwright/test";
import {
  bootEditor,
  getState,
  getText,
  terminalText,
  vfsList,
  vfsRead,
} from "./helpers.js";

// Type a command line and run it.
async function run(page, line) {
  await page.keyboard.type(line);
  await page.keyboard.press("Enter");
}

// Put `text` in the buffer, from normal mode, and come back to normal mode.
async function typeText(page, text) {
  await page.keyboard.press("i");
  await page.keyboard.type(text);
  await page.keyboard.press("Escape");
  await expect.poll(() => getText(page)).toContain(text);
}

// Swap the demo's handler for one that records every path it is asked
// about; `removed()` reads the record back.
async function recordRemovals(page) {
  await page.evaluate(() => {
    window.__removed = [];
    window.helixRemove = (path) => {
      window.__removed.push(path);
    };
  });
}
const removed = (page) => page.evaluate(() => window.__removed);

const currentPath = (page) => getState(page).then((s) => s.path);

test(":rm deletes the current file, closes its buffer and tells the host", async ({
  page,
}) => {
  await bootEditor(page);
  await recordRemovals(page);

  await typeText(page, "going away");
  await run(page, ":w /a.txt");
  await expect.poll(() => currentPath(page)).toBe("/a.txt");
  expect(await vfsList(page)).toContain("/a.txt");

  await run(page, ":rm");
  await expect.poll(() => vfsList(page)).not.toContain("/a.txt");
  // The buffer went with the key — the view is back on a scratch buffer,
  // not left open over a file that no longer exists.
  expect(await currentPath(page)).toBeUndefined();
  expect(await getText(page)).not.toContain("going away");
  // And the host heard about it, by store key.
  expect(await removed(page)).toEqual(["/a.txt"]);
  await expect.poll(() => terminalText(page)).toContain("Removed /a.txt");
});

test(":remove refuses unsaved changes until :remove!", async ({ page }) => {
  await bootEditor(page);
  await recordRemovals(page);

  await typeText(page, "saved");
  await run(page, ":w /b.txt");
  await expect.poll(() => vfsRead(page, "/b.txt")).toBe("saved\n");

  await page.keyboard.press("A");
  await page.keyboard.type(" and unsaved");
  await page.keyboard.press("Escape");

  await run(page, ":remove");
  await expect.poll(() => terminalText(page)).toContain("unsaved changes");
  // Refused means refused: the key, the buffer and the host are untouched.
  expect(await vfsRead(page, "/b.txt")).toBe("saved\n");
  expect(await getText(page)).toContain("and unsaved");
  expect(await removed(page)).toEqual([]);

  await run(page, ":remove!");
  await expect.poll(() => vfsList(page)).not.toContain("/b.txt");
  expect(await currentPath(page)).toBeUndefined();
  expect(await removed(page)).toEqual(["/b.txt"]);
});

test("a never-saved buffer just closes; the host is not told", async ({
  page,
}) => {
  await bootEditor(page);
  await recordRemovals(page);

  // A path without a `:w` behind it: the buffer has a name, the store has
  // no key. `:o` on a missing path is how helix makes one.
  await run(page, ":o /never.txt");
  await expect.poll(() => currentPath(page)).toBe("/never.txt");
  expect(await vfsList(page)).not.toContain("/never.txt");

  await run(page, ":rm");
  await expect.poll(() => currentPath(page)).toBeUndefined();
  // Nothing left the store, so the host — whose handler means "this key is
  // leaving" — was not asked about a key it never saw.
  expect(await removed(page)).toEqual([]);
  await expect
    .poll(() => terminalText(page))
    .toContain("Closed /never.txt (never saved; nothing to remove)");
});

test("a host page that refuses the removal keeps the file", async ({
  page,
}) => {
  await bootEditor(page);

  await typeText(page, "precious");
  await run(page, ":w /keep.txt");
  await expect.poll(() => vfsRead(page, "/keep.txt")).toBe("precious\n");

  // Throwing from the handler is the host's veto, and it has to land
  // before anything happens, not after.
  await page.evaluate(() => {
    window.helixRemove = () => {
      throw new Error("this page keeps everything");
    };
  });

  await run(page, ":rm");
  await expect
    .poll(() => terminalText(page))
    .toContain("this page keeps everything");
  expect(await vfsRead(page, "/keep.txt")).toBe("precious\n");
  expect(await currentPath(page)).toBe("/keep.txt");
  expect(await getText(page)).toContain("precious");
});

test(":remove <path> deletes a key that is not open, and only that", async ({
  page,
}) => {
  await bootEditor(page);
  await recordRemovals(page);

  await typeText(page, "looking at this one");
  await run(page, ":w /here.txt");
  await expect.poll(() => currentPath(page)).toBe("/here.txt");
  await page.evaluate(() => window.helixVfs.write("/other.txt", "not open"));

  await run(page, ":remove /other.txt");
  await expect.poll(() => vfsList(page)).not.toContain("/other.txt");
  expect(await removed(page)).toEqual(["/other.txt"]);
  // The argument named the target; the buffer being looked at is not it
  // and stays exactly where it was.
  expect(await currentPath(page)).toBe("/here.txt");
  expect(await vfsRead(page, "/here.txt")).toBe("looking at this one\n");

  // A name that is neither a key nor an open buffer is a refusal, not a
  // no-op that reads as success.
  await run(page, ":remove /missing.txt");
  await expect.poll(() => terminalText(page)).toContain("no such file");
  expect(await removed(page)).toEqual(["/other.txt"]);
});

test("helixVfs.delete drops a key without consulting the handler", async ({
  page,
}) => {
  await bootEditor(page);
  await recordRemovals(page);

  await page.evaluate(() => window.helixVfs.write("/mine.txt", "page's own"));
  expect(await vfsList(page)).toContain("/mine.txt");

  await page.evaluate(() => window.helixVfs.delete("/mine.txt"));
  expect(await vfsList(page)).not.toContain("/mine.txt");
  // The page did the deleting; the handler is for deletions the editor
  // makes, so it has nothing to say here.
  expect(await removed(page)).toEqual([]);

  // And a missing key throws rather than passing for a deletion.
  await expect(
    page.evaluate(() => window.helixVfs.delete("/mine.txt")),
  ).rejects.toThrow(/no such virtual file/);
});
