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

// The rendered terminal, as text. Only the boot test and the theme tests
// assert on rendered output (this helper or raw xterm cells) — there the
// pixels are the point: boot proves a statusline drew at all, and the theme
// tests prove the theme actually painted the screen, which no editor-state
// read can show. Everything else reads editor state, not pixels (the
// state-over-scraping rule from the issue #18 inspection API).
const terminalText = (page) =>
  page.evaluate(() => {
    const buffer = window.__helixTerminal.buffer.active;
    const lines = [];
    for (let i = 0; i < buffer.length; i += 1) {
      lines.push(buffer.getLine(i).translateToString(true));
    }
    return lines.join("\n");
  });

// The top-left cell's background as an RGB number, or -1 while it still has
// the terminal's (non-RGB) default background. Theme tests only — see the
// note on terminalText.
const topLeftBg = (page) =>
  page.evaluate(() => {
    const cell = window.__helixTerminal.buffer.active.getLine(0).getCell(0);
    return cell.isBgRGB() ? cell.getBgColor() : -1;
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

test(":tutor opens the tutorial in a pathless buffer", async ({ page }) => {
  await bootEditor(page);

  await page.keyboard.type(":tutor");
  await page.keyboard.press("Enter");

  await expect
    .poll(() => getText(page))
    .toContain("Welcome to the Helix tutorial!");
  // The command unsets the document path (`set_path(None)`), so a plain `:w`
  // cannot clobber the seeded tutor file.
  expect((await getState(page)).path).toBeUndefined();
});

test(":tutor still opens after a :cd away from the boot cwd (issue #60)", async ({
  page,
}) => {
  await bootEditor(page);

  // Move the virtual cwd off the boot cwd `/`. The relative `:w` proves the
  // cwd really changed — the document takes its path under the new
  // directory — so the `:tutor` below can't pass vacuously.
  await page.keyboard.type(":cd /elsewhere");
  await page.keyboard.press("Enter");
  await page.keyboard.type(":w proof.txt");
  await page.keyboard.press("Enter");
  await expect
    .poll(() => getState(page).then((s) => s.path))
    .toBe("/elsewhere/proof.txt");

  // The wasm32 runtime dir is absolute, so `runtime_file("tutor")` resolves
  // to the same vfs key the boot seeding wrote, regardless of the cwd.
  await page.keyboard.type(":tutor");
  await page.keyboard.press("Enter");
  await expect
    .poll(() => getText(page))
    .toContain("Welcome to the Helix tutorial!");
  expect((await getState(page)).path).toBeUndefined();
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

test(":theme lists a vendored theme and applying it recolors the screen", async ({
  page,
}) => {
  await bootEditor(page);

  // The prompt's completion menu lists the runtime themes directory, which
  // startup seeds with the vendored set (web/themes/).
  await page.keyboard.type(":theme ");
  await expect.poll(() => terminalText(page)).toContain("gruvbox");

  await page.keyboard.type("gruvbox");
  await page.keyboard.press("Enter");

  // gruvbox paints `ui.background` with bg0 (#282828) while the built-in
  // default theme leaves the terminal's default background, so the top-left
  // cell flipping to that RGB value proves the theme really applied.
  await expect.poll(() => topLeftBg(page)).toBe(0x282828);
});

test("a theme using inherits resolves through its parent", async ({
  page,
}) => {
  await bootEditor(page);

  // catppuccin_latte only sets a palette and inherits everything else from
  // catppuccin_mocha, so its `ui.background` (mocha's key, latte's `base`
  // color #eff1f5) can only render if the loader resolved the parent theme
  // through the vfs at runtime. A failed resolution refuses the theme
  // silently, leaving the default background — and this poll red.
  await page.keyboard.type(":theme catppuccin_latte");
  await page.keyboard.press("Enter");
  await expect.poll(() => topLeftBg(page)).toBe(0xeff1f5);
});

test(":theme still applies after a :cd away from the boot cwd (issue #60)", async ({
  page,
}) => {
  await bootEditor(page);

  // Same mechanism as the `:tutor` regression above: the themes are seeded
  // under `runtime_dirs()[0]/themes` at boot (cwd `/`), so before the r3
  // absolute-path fix a relative runtime dir made the loader search under the
  // *current* cwd and miss them. The relative `:w` proves the cwd really
  // moved, so the `:theme` below can't pass vacuously.
  await page.keyboard.type(":cd /elsewhere");
  await page.keyboard.press("Enter");
  await page.keyboard.type(":w proof.txt");
  await page.keyboard.press("Enter");
  await expect
    .poll(() => getState(page).then((s) => s.path))
    .toBe("/elsewhere/proof.txt");

  // A theme the loader can't find is refused silently, leaving the default
  // background — so gruvbox's bg0 landing in the top-left cell is the proof
  // it resolved the seeded file from the new cwd.
  await page.keyboard.type(":theme gruvbox");
  await page.keyboard.press("Enter");
  await expect.poll(() => topLeftBg(page)).toBe(0x282828);
});
