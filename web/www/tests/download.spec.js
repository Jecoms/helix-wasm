// `:download` — the one way a reader (as opposed to a script) gets work out
// of the page (issue #67). Everything else here lives in a store that dies
// with the tab, so what these assert on is the real thing: a browser
// download event, its suggested file name, and the bytes it carries.
import { test, expect } from "@playwright/test";
import {
  bootEditor,
  getState,
  getText,
  terminalText,
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

// Run `line` and return the download it produces, as
// `{ name, contents }`. Fails the test if none arrives.
async function downloadFrom(page, line) {
  const [download] = await Promise.all([
    page.waitForEvent("download"),
    run(page, line),
  ]);
  const stream = await download.createReadStream();
  const chunks = [];
  for await (const chunk of stream) {
    chunks.push(chunk);
  }
  return {
    name: download.suggestedFilename(),
    contents: Buffer.concat(chunks).toString("utf8"),
  };
}

test(":download saves the current file under its own name", async ({
  page,
}) => {
  await bootEditor(page);

  await typeText(page, "keep this");
  await run(page, ":w /notes.txt");
  await expect.poll(() => getState(page).then((s) => s.path)).toBe("/notes.txt");

  // The store key is a path; the download is a file name. There are no
  // directories to save into, so only the last component survives.
  const saved = await downloadFrom(page, ":download");
  expect(saved.name).toBe("notes.txt");
  expect(saved.contents).toBe("keep this\n");
});

test(":download exports the live buffer, not the last-saved copy", async ({
  page,
}) => {
  await bootEditor(page);

  await typeText(page, "saved");
  await run(page, ":w /draft.txt");
  await expect.poll(() => vfsRead(page, "/draft.txt")).toContain("saved");

  // Edit past the save. `:download` is "save a copy of what I am looking
  // at", so it must carry the edit the store has never seen — the same
  // split `editor_text()` draws against `vfs_read()`.
  await page.keyboard.press("A");
  await page.keyboard.type(" and then some");
  await page.keyboard.press("Escape");

  const saved = await downloadFrom(page, ":download");
  expect(saved.contents).toContain("and then some");
  expect(await vfsRead(page, "/draft.txt")).not.toContain("and then some");

  // And it exported rather than saved: the store is where it was, and the
  // buffer is still modified.
  expect(await vfsRead(page, "/draft.txt")).toBe("saved\n");
  expect(await getText(page)).toContain("and then some");
});

test(":download <name> names the download without touching anything", async ({
  page,
}) => {
  await bootEditor(page);

  await typeText(page, "from a scratch buffer");
  // The issue's "require name if not set": a scratch buffer has no name, so
  // the argument is the only one there is.
  expect(await getState(page).then((s) => s.path)).toBeUndefined();

  const saved = await downloadFrom(page, ":download hello.txt");
  expect(saved.name).toBe("hello.txt");
  // Byte for byte what the buffer holds — no trailing newline, because
  // `insert-final-newline` is something `:w` does *to the document* on its
  // way to the store, and `:download` edits nothing.
  expect(saved.contents).toBe("from a scratch buffer");

  // The argument named the download and nothing else: the buffer is still
  // nameless and the store never heard about it.
  expect(await getState(page).then((s) => s.path)).toBeUndefined();
  expect(await vfsRead(page, "hello.txt")).toBeUndefined();
});

test(":download refuses a nameless buffer instead of inventing a name", async ({
  page,
}) => {
  await bootEditor(page);
  await typeText(page, "unnamed");

  let downloads = 0;
  page.on("download", () => {
    downloads += 1;
  });

  await run(page, ":download");
  await expect
    .poll(() => terminalText(page))
    .toContain("This buffer has no name");

  // Naming it is what the message asks for, and it works.
  const saved = await downloadFrom(page, ":download named.txt");
  expect(saved.name).toBe("named.txt");
  expect(downloads).toBe(1);
});

test("a host page that refuses the save says so on the statusline", async ({
  page,
}) => {
  await bootEditor(page);
  await typeText(page, "nowhere to go");

  // The demo's implementation is replaceable — that is the point of hanging
  // it on `window` — and a handler that throws must surface as an editor
  // error rather than a download that silently never happens.
  await page.evaluate(() => {
    window.helixDownload = () => {
      throw new Error("the vault is closed");
    };
  });

  await run(page, ":download refused.txt");
  await expect.poll(() => terminalText(page)).toContain("the vault is closed");
});
