// The `+`/`*` register bridge to `navigator.clipboard` (issue #140). Chromium
// is the one browser Playwright can grant the clipboard permissions in, so
// these cover the bridge's mechanics — the write on a yank, the read
// prefetched ahead of a paste, the held-back input flowing on in order, and
// the editor-local fallback when the read is refused. Safari's and
// Firefox's per-paste "Paste" affordance is a manual check (see the README's
// limitations section).
//
// Same shape as the other suites, sharing their plumbing (./helpers.js).
import { test, expect } from "@playwright/test";
import { bootEditor, getState, getText } from "./helpers.js";

const readClipboard = (page) =>
  page.evaluate(() => navigator.clipboard.readText());

const writeClipboard = (page, text) =>
  page.evaluate((t) => navigator.clipboard.writeText(t), text);

// A buffer holding one known word, selected whole (`%`): the scratch buffer
// has no trailing newline, so the selection is exactly the word.
async function typeWord(page, word) {
  await page.keyboard.press("i");
  await page.keyboard.type(word);
  await page.keyboard.press("Escape");
  await expect.poll(() => getText(page)).toBe(word);
  await page.keyboard.press("%");
}

test.describe("with the clipboard permissions granted", () => {
  test.use({ permissions: ["clipboard-read", "clipboard-write"] });

  test('"+y puts the selection on the browser clipboard', async ({ page }) => {
    await bootEditor(page);
    await typeWord(page, "hello");
    await page.keyboard.type('"+y');
    await expect.poll(() => readClipboard(page)).toBe("hello");
  });

  test('"*y writes the same clipboard: a browser has only one', async ({
    page,
  }) => {
    await bootEditor(page);
    await typeWord(page, "star");
    await page.keyboard.type('"*y');
    await expect.poll(() => readClipboard(page)).toBe("star");
  });

  test('"+p pastes what the page did not put there', async ({ page }) => {
    await bootEditor(page);
    await typeWord(page, "x");
    await writeClipboard(page, "from outside");
    // `p` pastes after the selection, which is the whole of `x`.
    await page.keyboard.type('"+p');
    await expect.poll(() => getText(page)).toBe("xfrom outside");
  });

  test("space-p pastes the clipboard too, and later keys wait their turn", async ({
    page,
  }) => {
    await bootEditor(page);
    await typeWord(page, "x");
    await writeClipboard(page, "outside");
    // Everything after the `p` is typed without waiting for the read to
    // settle: it has to land after the paste, not before it.
    await page.keyboard.type(" pA!");
    await page.keyboard.press("Escape");
    await expect.poll(() => getText(page)).toBe("xoutside!");
    expect((await getState(page)).mode).toBe("normal");
  });

  test("C-r + in insert mode inserts the clipboard", async ({ page }) => {
    await bootEditor(page);
    await writeClipboard(page, "inserted");
    await page.keyboard.press("i");
    await page.keyboard.press("Control+r");
    await page.keyboard.type("+");
    await page.keyboard.press("Escape");
    await expect.poll(() => getText(page)).toBe("inserted");
  });
});

test.describe("with the clipboard read refused", () => {
  // Chromium auto-denies a permission no one granted, so `readText()`
  // rejects at once and the register falls back to what it holds.
  test('"+y then "+p still round-trips inside the page, and input flows on', async ({
    page,
  }) => {
    await bootEditor(page);
    await typeWord(page, "local");
    await page.keyboard.type('"+y"+p');
    await expect.poll(() => getText(page)).toBe("locallocal");
    // The refused read must not have wedged the queue: a plain keystroke
    // after it still reaches the editor.
    await page.keyboard.press("i");
    await expect.poll(() => getState(page).then((s) => s.mode)).toBe("insert");
  });
});
