// Alt-chord forwarding (issue #68). The plain key path is covered by the
// smoke suite's real keystrokes; what needs its own tests is the macOS
// Option handling — `macOptionIsMeta` plus the fallback that takes the
// chord's character from xterm.js when macOS composed one of its own, and
// the narrowness of that fallback, which is what keeps `A-Left` from
// arriving as `A-b`.
//
// Same shape as the other suites, sharing their plumbing (./helpers.js).
import { test, expect } from "@playwright/test";
import { bootEditor, getState, getText } from "./helpers.js";

// Every macOS-specific thing xterm.js does — and the host page's fallback
// with it — hangs off a single value. `isMac` in @xterm/xterm 5.5
// (common/Platform.ts) is `["Macintosh", "MacIntel", "MacPPC",
// "Mac68K"].includes(navigator.platform)`, and main.js mirrors that test
// exactly so the two cannot disagree. Overriding it before any page script
// runs therefore puts a Linux runner on the real macOS path instead of
// skipping these tests there — including xterm's `shouldIgnoreComposition`
// (Terminal.ts:1010-1021), the branch that otherwise sets
// `_unprocessedDeadKey` and swallows a `Dead` keydown before `onKey` fires.
// The assertion below is not decoration: without it a failed override would
// leave a green suite that had silently tested the non-mac path.
async function bootAsMac(page) {
  await page.addInitScript(() => {
    Object.defineProperty(Navigator.prototype, "platform", {
      configurable: true,
      get: () => "MacIntel",
    });
  });
  await bootEditor(page);
  expect(
    await page.evaluate(() => navigator.platform),
    "navigator.platform override did not take, so these tests would be exercising the non-mac path",
  ).toBe("MacIntel");
}

// One keydown straight onto xterm's helper textarea — the element its own
// key handling listens on, so the event takes the full
// `evaluateKeyboardEvent` → `onKey` → `key_event()` path. Playwright's
// keyboard cannot stand in where composition is the point: it synthesizes
// from a US layout and drives the renderer through CDP, which never invokes
// the OS input method, so it cannot produce the `key`/`code` divergence a
// composed Option keystroke has. The events below are the shape macOS
// produces as read off xterm's source, not as captured from a physical Mac
// keyboard — issue #81 tracks closing that gap by hand.
const dispatchKey = (page, init) =>
  page.evaluate(
    (event) =>
      window.__helixTerminal.textarea.dispatchEvent(
        new KeyboardEvent("keydown", {
          ...event,
          bubbles: true,
          cancelable: true,
        }),
      ),
    init,
  );

async function openFile(page, name, contents) {
  await page.evaluate(
    ([path, text]) => window.helixVfs.write(path, text),
    [name, contents],
  );
  await page.keyboard.type(`:o ${name}`);
  await page.keyboard.press("Enter");
  await expect.poll(() => getState(page).then((s) => s.path)).toBe(`/${name}`);
}

test("Option is claimed as Meta so Alt chords are not composed away", async ({
  page,
}) => {
  await bootAsMac(page);

  // Without this xterm.js drops every Alt keydown on macOS before `onKey`
  // (its `_isThirdLevelShift` guard), and no amount of host-page handling
  // can recover the chord.
  expect(
    await page.evaluate(() => window.__helixTerminal.options.macOptionIsMeta),
  ).toBe(true);
});

test("an Option-composed Alt chord runs the binding, not the composed character", async ({
  page,
}) => {
  await bootAsMac(page);
  await openFile(page, "two.txt", "alpha\nbeta\n");

  // Select the whole buffer: one selection spanning both lines.
  await page.keyboard.press("%");
  await expect
    .poll(() => getState(page).then((s) => s.selections.length))
    .toBe(1);

  // What a real Option-s delivers on macOS: the composed character in `key`,
  // the physical key in `code`/`keyCode`. Chrome honors `keyCode` from the
  // init dict, which is what xterm dispatches off.
  await dispatchKey(page, {
    key: "ß",
    code: "KeyS",
    keyCode: 83,
    altKey: true,
  });

  // A-s is `split_selection_on_newline`, so the chord landing splits the
  // selection per line; forwarding "ß" instead would leave it whole.
  await expect
    .poll(() => getState(page).then((s) => s.selections.length), {
      message: "A-s did not reach the editor",
    })
    .toBeGreaterThan(1);
  expect(await getText(page)).toBe("alpha\nbeta\n");
});

test("a dead-key Alt chord runs its binding", async ({ page }) => {
  await bootAsMac(page);
  await openFile(page, "word.txt", "alpha\n");

  // One edit, so the chord under test has something to undo.
  await page.keyboard.press("i");
  await page.keyboard.type("X");
  await page.keyboard.press("Escape");
  await expect.poll(() => getText(page)).toBe("Xalpha\n");

  // What a real Option-u delivers on macOS: the accent starter begins a
  // composition, so `key` is "Dead" and `keyCode` is 229 — the composition
  // sentinel, not the letter's own legacy code. That pair reaches exactly
  // one branch of xterm's `evaluateKeyboardEvent` (`ev.key === 'Dead' &&
  // ev.code.startsWith('Key')`, Keyboard.ts:373-385), which resolves the
  // chord off `code` and hands over `ESC u`. Faking a letter's own keyCode
  // here instead would be caught by the `keyCode >= 65 && <= 90` branch
  // above it — a shape macOS never sends, and one that would pass whether
  // or not the dead-key path worked at all.
  //
  // Only `code: "Key*"` gets this far. A punctuation dead key
  // (`` Option-` ``, i.e. `` A-` ``) matches no branch, so `result.key`
  // stays undefined and `_keyDown` returns before `onKey` — that chord is
  // still broken, tracked in issue #81.
  await dispatchKey(page, {
    key: "Dead",
    code: "KeyU",
    keyCode: 229,
    altKey: true,
  });

  // A-u is `earlier`: the insert is rolled back. Forwarding "Dead" instead
  // names no key at all and would leave the buffer as edited.
  await expect
    .poll(() => getText(page), { message: "A-u did not reach the editor" })
    .toBe("alpha\n");
});

test("a named key whose sequence is ESC + a character keeps its DOM name", async ({
  page,
}) => {
  await bootAsMac(page);
  await openFile(page, "words.txt", "alpha beta\n");

  // The other half of the fix. xterm encodes several named keys as `ESC` +
  // one character — `A-Backspace` as `ESC DEL` (Keyboard.ts case 8, on every
  // platform), `A-Left` as `ESC b` on macOS — so a looser rule ("if altKey,
  // prefer the payload") would forward those as `A-\x7f` and `A-b`
  // (`move_parent_node_start`). `A-Backspace` is the one that can be
  // asserted here: `A-Left` is `select_prev_sibling`, which needs a syntax
  // tree a .txt buffer does not have.
  await page.keyboard.press("A"); // append: end of line, insert mode
  await expect.poll(() => getState(page).then((s) => s.mode)).toBe("insert");
  await page.keyboard.press("Alt+Backspace");

  // A-backspace is `delete_word_backward` in insert mode.
  await expect
    .poll(() => getText(page), {
      message: "A-Backspace did not reach the editor as a named key",
    })
    .toBe("alpha \n");
});
