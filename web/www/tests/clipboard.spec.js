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

// Replace `navigator.clipboard.readText` with one the test scripts: each
// call takes the next entry of `answers` — a string resolves with it, `null`
// never resolves — and the calls are counted on `window.__reads`. The bridge
// reaches the method through the instance, so an own property shadows it.
async function scriptReads(page, answers) {
  await page.evaluate((answers) => {
    window.__reads = 0;
    navigator.clipboard.readText = () => {
      const answer = answers[window.__reads];
      window.__reads += 1;
      return answer === null ? new Promise(() => {}) : Promise.resolve(answer);
    };
  }, answers);
}

const readCount = (page) => page.evaluate(() => window.__reads);

test.describe("which keystrokes read", () => {
  test.use({ permissions: ["clipboard-read", "clipboard-write"] });

  test("p typed into a prompt is not a paste, so it does not read", async ({
    page,
  }) => {
    await bootEditor(page);
    await scriptReads(page, ["never pasted"]);
    // A space then `p`/`P`/`R` in a search and a command line, where the
    // editor's mode still says normal; then `"+p` out of the prompt, which
    // is the one that has to read.
    await page.keyboard.type("/a p");
    await page.keyboard.press("Escape");
    await page.keyboard.type(":open some Path");
    await page.keyboard.press("Escape");
    expect(await readCount(page)).toBe(0);
    await page.keyboard.type('"+p');
    await expect.poll(() => getText(page)).toBe("never pasted");
    expect(await readCount(page)).toBe(1);
  });

  test("a second paste typed during a read makes its own read", async ({
    page,
  }) => {
    await bootEditor(page);
    await typeWord(page, "x");
    // The first read is answered only when the test says so; the second
    // `"+p` is typed while it is still open, and must not go through on
    // the first one's answer.
    await page.evaluate(() => {
      window.__reads = 0;
      window.__answer = [];
      navigator.clipboard.readText = () => {
        const index = window.__reads;
        window.__reads += 1;
        return new Promise((resolve) => {
          window.__answer[index] = resolve;
        });
      };
    });
    await page.keyboard.type('"+p"+p');
    await expect.poll(() => readCount(page)).toBe(1);
    await page.evaluate(() => window.__answer[0]("one"));
    await expect.poll(() => getText(page)).toBe("xone");
    await expect.poll(() => readCount(page)).toBe(2);
    await page.evaluate(() => window.__answer[1]("two"));
    await expect.poll(() => getText(page)).toBe("xonetwo");
  });

  test("a settled read's timeout does not cut a later read short", async ({
    page,
  }) => {
    await bootEditor(page);
    await typeWord(page, "x");
    // The keystrokes above armed helix's debounced hooks (diagnostics,
    // signature help — 350 ms at most) on real timers. Let those elapse
    // before the fake clock takes over: a real timer can only be cleared by
    // the real `clearTimeout`, and once installed the fake one answers that
    // call instead, so a hook re-arming its debounce under the fake clock
    // would leave the real timer to fire into a cancelled callback.
    await page.waitForTimeout(500);
    await page.clock.install();
    // First read answered at once; the second is never answered, so only
    // its own 5 s timeout may end it — not the first read's, which is
    // still pending when the second starts 4.5 s later.
    await scriptReads(page, ["one", null]);
    await page.keyboard.type('"+p');
    await expect.poll(() => getText(page)).toBe("xone");
    await page.clock.runFor(4_500);
    await page.keyboard.type('"+p');
    await expect.poll(() => readCount(page)).toBe(2);
    await page.clock.runFor(1_000);
    // 5.5 s after the first read, 1 s into the second: still waiting.
    expect(await getText(page)).toBe("xone");
    await page.clock.runFor(4_500);
    await expect.poll(() => getText(page)).toBe("xoneone");
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
