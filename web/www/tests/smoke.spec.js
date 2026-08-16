// Browser smoke tests (issue #44): boot the built wasm bundle headlessly and
// assert on editor behavior through the committed inspection surfaces —
// `window.helixState` (issue #18), `window.helixVfs`, and the raw
// `window.__helixTerminal` buffer. Keystrokes go through Playwright's real
// keyboard events, so they exercise the true xterm → `key_event()` path.
//
// Boot and the save queue are asynchronous, so every post-action assertion
// polls (`expect.poll`) instead of reading once.
import { test, expect } from "@playwright/test";

const getState = (page) =>
  page.evaluate(() => window.helixState.state());

const getText = (page) =>
  page.evaluate(() => window.helixState.text());

const vfsRead = (page, path) =>
  page.evaluate((p) => window.helixVfs.read(p), path);

// The rendered terminal, as text. Only the boot test asserts on this —
// everything else reads editor state, not pixels.
const terminalText = (page) =>
  page.evaluate(() => {
    const buffer = window.__helixTerminal.buffer.active;
    const lines = [];
    for (let i = 0; i < buffer.length; i += 1) {
      lines.push(buffer.getLine(i).translateToString(true));
    }
    return lines.join("\n");
  });

// Wait out the wasm fetch + instantiation, then make sure keystrokes land in
// xterm's hidden textarea.
async function bootEditor(page) {
  await page.goto("/");
  await expect
    .poll(async () => page.evaluate(() => window.helixState?.state()?.mode), {
      message: "editor did not reach normal mode after boot",
      timeout: 30_000,
    })
    .toBe("normal");
  await page.locator("#terminal").click();
}

test("boots into a normal-mode scratch buffer with a rendered statusline", async ({
  page,
}) => {
  await bootEditor(page);

  const state = await getState(page);
  expect(state.mode).toBe("normal");
  expect(state.cursor).toEqual({ row: 0, col: 0 });
  await expect(
    page.evaluate(() => window.helixState.state().path === undefined),
  ).resolves.toBe(true);

  // The default statusline names the buffer and shows the mode indicator.
  await expect.poll(() => terminalText(page)).toContain("[scratch]");
  await expect.poll(() => terminalText(page)).toContain("NOR");
});

test("tracks mode, cursor, and selection through i / Esc / v motions", async ({
  page,
}) => {
  await bootEditor(page);

  await page.keyboard.press("i");
  await expect.poll(() => getState(page).then((s) => s.mode)).toBe("insert");

  await page.keyboard.type("hello");
  await expect.poll(() => getText(page)).toContain("hello");
  await expect
    .poll(() => getState(page).then((s) => s.cursor))
    .toEqual({ row: 0, col: 5 });

  await page.keyboard.press("Escape");
  await expect.poll(() => getState(page).then((s) => s.mode)).toBe("normal");

  // `v b` from the last character sweeps the selection back over the word:
  // anchor and head diverge and the span covers all five chars.
  await page.keyboard.press("v");
  await expect.poll(() => getState(page).then((s) => s.mode)).toBe("select");
  await page.keyboard.press("b");
  await expect
    .poll(async () => {
      const { anchor, head } = (await getState(page)).selections[0];
      return { moved: head !== anchor, span: Math.abs(head - anchor) };
    })
    .toEqual({ moved: true, span: 5 });

  await page.keyboard.press("Escape");
  await expect.poll(() => getState(page).then((s) => s.mode)).toBe("normal");
});

test("page background matches the terminal's, with no phantom scrollbar (issue #49)", async ({
  page,
}) => {
  await bootEditor(page);

  // Background unification: the page body mirrors the BACKGROUND constant
  // that drives xterm's theme.background, so the partial-cell strips the
  // integer cell grid leaves around the editor blend in instead of framing
  // it in a different color.
  expect(
    await page.evaluate(() => getComputedStyle(document.body).backgroundColor),
  ).toBe("rgb(0, 0, 0)");
  expect(
    await page.evaluate(() => window.__helixTerminal.options.theme.background),
  ).toBe("#000000");

  // Phantom scrollbar: xterm.css ships the viewport with overflow-y: scroll,
  // which keeps a track visible on classic-scrollbar systems even though
  // scrollback: 0 means it can never overflow. The index.html override must
  // win the cascade.
  expect(
    await page.evaluate(
      () =>
        getComputedStyle(document.querySelector("#terminal .xterm-viewport"))
          .overflowY,
    ),
  ).toBe("hidden");

  // Regression check on the #37 min-size floor: the override is scoped to
  // the inner viewport, so below 600x400 the page itself must still scroll
  // on both axes. Poll — the refit after resize is async.
  await page.setViewportSize({ width: 500, height: 300 });
  await expect
    .poll(() =>
      page.evaluate(() => ({
        w: document.documentElement.scrollWidth,
        h: document.documentElement.scrollHeight,
      })),
    )
    .toEqual({ w: 600, h: 400 });
});

test(":w names the buffer in the vfs; live text diverges until re-saved", async ({
  page,
}) => {
  await bootEditor(page);

  await page.keyboard.press("i");
  await page.keyboard.type("saved text");
  await page.keyboard.press("Escape");

  await page.keyboard.type(":w scratch.txt");
  await page.keyboard.press("Enter");

  // The save queue is async: poll until the document takes its path, then
  // the vfs copy must match the live buffer exactly.
  await expect
    .poll(() => getState(page).then((s) => s.path))
    .toBe("/scratch.txt");
  const savedText = await getText(page);
  expect(savedText).toContain("saved text");
  await expect.poll(() => vfsRead(page, "scratch.txt")).toBe(savedText);

  // An unsaved edit shows up in the live text but not in the saved copy.
  await page.keyboard.press("A");
  await page.keyboard.type(" plus more");
  await page.keyboard.press("Escape");
  await expect.poll(() => getText(page)).toContain("plus more");
  expect(await vfsRead(page, "scratch.txt")).toBe(savedText);

  // Re-saving converges the two again.
  await page.keyboard.type(":w");
  await page.keyboard.press("Enter");
  await expect
    .poll(async () => vfsRead(page, "scratch.txt"))
    .toBe(await getText(page));
});
