// `<space>/` global search against the virtual file system (issue #130).
// The native implementation walks a real filesystem on a threadpool and
// greps it with real file IO; the wasm32 arm greps the store — the same
// candidate set `<space>f` offers — and dispatches the query synchronously
// per keystroke, there being no runtime to debounce on. These assert the
// four things that arm has to get right: store contents match and
// selecting a row opens the file at the matching line, unsaved edits in an
// open buffer match (the rope branch), the boot-seeded runtime files
// stay out of the results the way they stay out of the picker, and a
// half-typed pattern that does not parse leaves the editor alive.
//
// The result rows and the preview are compositor layers with no
// `helixState` surface, so the picker assertions read the rendered
// terminal (see the note on `terminalText` in ./helpers.js); everything
// after a selection reads editor state.
import { test, expect } from "@playwright/test";
import {
  bootEditor,
  getState,
  getText,
  terminalText,
  vfsRead,
} from "./helpers.js";

// Save the current buffer under `path` and wait for the async save queue.
async function saveAs(page, path) {
  await page.keyboard.type(`:w ${path}`);
  await page.keyboard.press("Enter");
  await expect.poll(() => getState(page).then((s) => s.path)).toBe(path);
  await expect.poll(() => vfsRead(page, path)).not.toBeUndefined();
}

// Open `<space>/`, type `query`, and wait for `marker` (a `path:line` row)
// to be on screen — poll, because the results arrive through the job queue
// a beat behind the prompt.
async function search(page, query, marker) {
  await page.keyboard.press(" ");
  await page.keyboard.press("/");
  await page.keyboard.type(query);
  await expect
    .poll(() => terminalText(page), {
      message: `global search never listed ${marker}`,
    })
    .toContain(marker);
}

test("global search greps the store and opens the file at the matching line", async ({
  page,
}) => {
  await bootEditor(page);

  // `/example.rs` is seeded but not open, so this hit comes from the
  // store's bytes (`vfs::reader`), not from any buffer.
  await search(page, "wasm32", "example.rs:8");

  // The preview pane shows the file around the hit — content the prompt
  // never carried.
  expect(await terminalText(page)).toContain("fn greet(name");

  await page.keyboard.press("Enter");
  await expect
    .poll(() => getState(page).then((s) => s.path))
    .toBe("/example.rs");
  expect((await getState(page)).cursor.row).toBe(7);
});

test("global search reads unsaved edits out of the open buffer", async ({
  page,
}) => {
  await bootEditor(page);

  await page.keyboard.press("i");
  await page.keyboard.type("saved line");
  await page.keyboard.press("Escape");
  await saveAs(page, "/draft.txt");

  // One more line the store has not seen.
  await page.keyboard.press("o");
  await page.keyboard.type("rope_only_token");
  await page.keyboard.press("Escape");
  expect(await vfsRead(page, "/draft.txt")).not.toContain("rope_only_token");

  // The hit can only have come from the buffer's rope.
  await search(page, "rope_only_token", "draft.txt:2");
});

test("global search does not read the boot-seeded runtime files", async ({
  page,
}) => {
  await bootEditor(page);

  // The word is in the seeded tutor text — the guard the negative
  // assertion below is worthless without — and in a file of ours, so the
  // search provably ran and returned something.
  const tutor = await vfsRead(page, "/.config/helix/runtime/tutor");
  expect(tutor).toContain("cursor");

  await page.keyboard.press("i");
  await page.keyboard.type("the cursor word");
  await page.keyboard.press("Escape");
  await saveAs(page, "/mine.txt");

  await search(page, "cursor", "mine.txt:1");
  // The seeded keys all live under `/.config/helix/runtime/`, which the
  // path column would render with this substring (truncated from the left
  // on the longest of them — see picker.spec.js).
  expect(await terminalText(page)).not.toContain("runtime/");
});

test("an invalid mid-typing pattern leaves the editor alive", async ({
  page,
}) => {
  await bootEditor(page);

  // Something for the search to find once the pattern parses again.
  await page.keyboard.press("i");
  await page.keyboard.type("bracket_token");
  await page.keyboard.press("Escape");
  await saveAs(page, "/probe.txt");

  // An unterminated `[` is what every character-class query looks like
  // mid-typing, and with the query dispatched synchronously per keystroke
  // it reaches the regex Err branch on this very keystroke — the branch
  // jobs.spec.js pins for the `/` prompt, pinned here for `<space>/`.
  await page.keyboard.press(" ");
  await page.keyboard.press("/");
  await page.keyboard.type("[");

  // Finishing the class makes the pattern valid again; the results
  // arriving proves the Err branch neither trapped nor wedged the picker.
  await page.keyboard.type("b]racket_token");
  await expect
    .poll(() => terminalText(page), {
      message: "global search never recovered from the invalid pattern",
    })
    .toContain("probe.txt:1");
  await page.keyboard.press("Escape");

  // The proof of life jobs.spec.js ends on: the editor takes a keystroke,
  // changes state because of it, and paints the result.
  const before = await terminalText(page);
  await page.keyboard.press("i");
  await expect.poll(() => getState(page).then((s) => s.mode)).toBe("insert");
  await page.keyboard.type("after bad regex");
  await expect.poll(() => getText(page)).toContain("after bad regex");
  await page.keyboard.press("Escape");
  await expect.poll(() => getState(page).then((s) => s.mode)).toBe("normal");
  await expect.poll(() => terminalText(page)).not.toBe(before);
  // The host page's crash gate never fired (web/www/main.js): a trap would
  // have replaced the screen with this notice.
  expect(await terminalText(page)).not.toContain(
    "Helix has stopped responding",
  );
});
