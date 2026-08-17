// Alt-chord forwarding (issues #68 and #81). The plain key path is covered
// by the smoke suite's real keystrokes; what needs its own tests is the
// macOS Option handling, which has two halves:
//
//   - the `onKey` fallback that takes the chord's character from xterm.js
//     when macOS composed one of its own, and the narrowness of that
//     fallback, which is what keeps `A-Left` from arriving as `A-b`;
//   - the custom key handler that resolves the dead keys xterm.js drops
//     before `onKey` runs at all — the punctuation ones, `` A-` ``
//     among them (issue #81).
//
// The two must not overlap: a chord xterm resolves has to keep going
// through `onKey` alone, or it lands twice.
//
// Same shape as the other suites, sharing their plumbing (./helpers.js).
import { test, expect } from "@playwright/test";
import { bootEditor, getState, getText } from "./helpers.js";

// Every macOS-specific thing xterm.js does — and the host page's Option
// handling with it — hangs off a single value. `isMac` in @xterm/xterm 5.5
// (common/Platform.ts) is `["Macintosh", "MacIntel", "MacPPC",
// "Mac68K"].includes(navigator.platform)`, and main.js mirrors that test
// exactly so the two cannot disagree. Overriding it before any page script
// runs therefore puts a Linux runner on the real macOS path instead of
// skipping these tests there — including xterm's `shouldIgnoreComposition`
// (Terminal.ts:1010-1021), the branch that otherwise sets
// `_unprocessedDeadKey` and swallows a `Dead` keydown before `onKey` fires.
// It cuts the other way too: pointing it at a non-mac value is the only way
// to assert the gates *hold* from a mac dev box, where `navigator.platform`
// would otherwise say "MacIntel" whatever the test wanted.
//
// The assertion is not decoration: without it a failed override would leave
// a green suite that had silently tested the wrong path.
async function bootAsPlatform(page, platform) {
  await page.addInitScript((value) => {
    Object.defineProperty(Navigator.prototype, "platform", {
      configurable: true,
      get: () => value,
    });
  }, platform);
  await bootEditor(page);
  expect(
    await page.evaluate(() => navigator.platform),
    "navigator.platform override did not take, so this test would be exercising the wrong platform path",
  ).toBe(platform);
}

const bootAsMac = (page) => bootAsPlatform(page, "MacIntel");

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

test("a letter dead-key Alt chord runs its binding exactly once", async ({
  page,
}) => {
  await bootAsMac(page);
  await openFile(page, "word.txt", "alpha\n");

  // Two edits, so the chord under test has two history steps behind it —
  // one to undo, and one more that must survive. A single edit would let a
  // chord delivered twice pass: the second `earlier` would find nothing
  // left to roll back and look identical to one delivery.
  await page.keyboard.press("i");
  await page.keyboard.type("X");
  await page.keyboard.press("Escape");
  await page.keyboard.press("i");
  await page.keyboard.type("Y");
  await page.keyboard.press("Escape");
  await expect.poll(() => getText(page)).toBe("XYalpha\n");

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
  // It is also the chord that pins down where the two halves meet. One
  // keystroke must produce one `key_event`, whichever half resolves it:
  // main.js's custom key handler leaves `code: "Key*"` to xterm, and where
  // it does step in it returns `false` so xterm stops. Forward without
  // that `false` and `earlier` runs twice off this one keystroke.
  await dispatchKey(page, {
    key: "Dead",
    code: "KeyU",
    keyCode: 229,
    altKey: true,
  });

  // A-u is `earlier`: one history step back, so the "Y" insert is rolled
  // back and the "X" one is not. Forwarding "Dead" instead names no key at
  // all and would leave the buffer as edited; delivering the chord twice
  // would strip the "X" as well.
  await expect
    .poll(() => getText(page), { message: "A-u did not reach the editor" })
    .toBe("Xalpha\n");
  // Nothing further lands: the chord was one keystroke, and both deliveries
  // of a doubled one would already have run by the time the first showed up
  // above. Re-reading after a round trip through the editor makes that
  // explicit rather than implied.
  await page.keyboard.press("i");
  await expect.poll(() => getState(page).then((s) => s.mode)).toBe("insert");
  await page.keyboard.press("Escape");
  expect(await getText(page)).toBe("Xalpha\n");
});

test("tutor 10.3: a punctuation dead-key Alt chord reaches the editor (issue #81)", async ({
  page,
}) => {
  await bootAsMac(page);
  await openFile(page, "case.txt", "this sentence should be uppercase\n");

  // The other shape a macOS dead key comes in, and the one xterm.js cannot
  // resolve: `code: "Backquote"` fails its `code.startsWith("Key")` test,
  // no other branch matches keyCode 229, and `_keyDown` returns at
  // `if (!result.key)` before `_onKey.fire` (Terminal.ts:1046-1048). So
  // this event never reaches `onKey` at all — only the custom key handler,
  // which runs first (Terminal.ts:1004-1007), can see it.
  //
  // Verbatim tutor 10.3, steps 7-8: "Type x to select the line. Press
  // Alt-` to change the line to uppercase."
  await page.keyboard.press("x");
  await dispatchKey(page, {
    key: "Dead",
    code: "Backquote",
    keyCode: 229,
    altKey: true,
  });

  // A-` is `switch_to_uppercase`.
  await expect
    .poll(() => getText(page), { message: "A-` did not reach the editor" })
    .toBe("THIS SENTENCE SHOULD BE UPPERCASE\n");
});

test("a shifted punctuation dead key takes the shifted character", async ({
  page,
}) => {
  await bootAsMac(page);
  await openFile(page, "shift.txt", "alpha\nbeta\n");

  // Two selections, one per word, with real keystrokes: select the buffer,
  // then `s` to split it on \w+.
  await page.keyboard.press("%");
  await page.keyboard.press("s");
  await page.keyboard.type("\\w+");
  await page.keyboard.press("Enter");
  await expect
    .poll(() => getState(page).then((s) => s.selections.length))
    .toBe(2);

  // The shifted column of the code table. `Digit0` with shift is `)`, and
  // `A-)` is `rotate_selection_contents_forward`; the unshifted `0` is
  // bound to nothing at all, so a table that dropped its shifted column
  // would leave the buffer untouched here. xterm honors shift for the dead
  // keys it resolves itself (its letter branch uppercases on `ev.shiftKey`)
  // and its keyCode table carries both columns, so main.js's carries both
  // for the same reason.
  await dispatchKey(page, {
    key: "Dead",
    code: "Digit0",
    keyCode: 229,
    altKey: true,
    shiftKey: true,
  });

  // The two selections trade contents.
  await expect
    .poll(() => getText(page), {
      message: "A-) did not reach the editor as the shifted character",
    })
    .toBe("beta\nalpha\n");
});

test("off macOS a punctuation dead key is left alone", async ({ page }) => {
  // The gate that keeps the fix from becoming a US-layout guess everywhere
  // else. `KeyboardEvent.code` names a physical key, so resolving one
  // through a US table off macOS would turn chords that are inert on a
  // non-US layout into live commands — the same objection that made the
  // `onKey` fallback macOS-only.
  await bootAsPlatform(page, "Linux x86_64");
  await openFile(page, "linux.txt", "this stays lowercase\n");

  await page.keyboard.press("x");
  await dispatchKey(page, {
    key: "Dead",
    code: "Backquote",
    keyCode: 229,
    altKey: true,
  });

  // A round trip through the editor, so "nothing happened" is a settled
  // fact rather than a race: `switch_to_uppercase` would have run before
  // this mode change if the chord had been taken over.
  await page.keyboard.press("i");
  await expect.poll(() => getState(page).then((s) => s.mode)).toBe("insert");
  await page.keyboard.press("Escape");
  expect(await getText(page)).toBe("this stays lowercase\n");
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
