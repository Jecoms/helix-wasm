// Alt-chord forwarding (issues #68, #81 and #137). The plain key path is
// covered by the smoke suite's real keystrokes; what needs its own tests is
// the Alt handling, which has two halves:
//
//   - the `onKey` fallback that takes the chord's character from xterm.js
//     when macOS composed one of its own, and the narrowness of that
//     fallback, which is what keeps `A-Left` from arriving as `A-b`;
//   - the custom key handler that resolves the punctuation- and digit-row
//     chords xterm.js drops before `onKey` runs at all — the macOS dead
//     keys, `` A-` `` among them (issue #81), and the keys whose legacy
//     `keyCode` Firefox numbers differently, `A-;` among them (issue #137).
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
  //
  // Unlike the `Backquote` and `KeyU` cases, this shape is constructed
  // rather than observed: no US-layout dead key sits on a shifted
  // punctuation key, so nobody can type this. It tests that the table is a
  // faithful transposition of xterm's, which is the only claim main.js
  // makes about the shifted column — layouts that do put a dead key there
  // are exactly the ones this cannot be checked against by hand.
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

test("the dead-key handler holds xterm's own conditions on an Alt chord", async ({
  page,
}) => {
  // The handler is the missing case of xterm's Alt branch, not a second
  // policy about Alt chords, so it restates that branch's condition:
  // `(!isMac || macOptionIsMeta) && ev.altKey && !ev.metaKey`
  // (Keyboard.ts:349). Both of the clauses that are not covered elsewhere
  // are asserted here, against the same chord the tutor 10.3 test uses.
  await bootAsMac(page);
  await openFile(page, "conditions.txt", "this stays lowercase\n");
  await page.keyboard.press("x");

  // Cmd-Option chords belong to the browser and the OS; xterm declines them
  // and so does this.
  await dispatchKey(page, {
    key: "Dead",
    code: "Backquote",
    keyCode: 229,
    altKey: true,
    metaKey: true,
  });

  // With Option no longer claimed as Meta it goes back to being a compose
  // key, and xterm drops every Alt chord in `_isThirdLevelShift`. The two
  // have to move together, or this one chord would keep firing while the
  // rest of the `A-` space went dead.
  await page.evaluate(() => {
    window.__helixTerminal.options.macOptionIsMeta = false;
  });
  await dispatchKey(page, {
    key: "Dead",
    code: "Backquote",
    keyCode: 229,
    altKey: true,
  });

  await page.keyboard.press("i");
  await expect.poll(() => getState(page).then((s) => s.mode)).toBe("insert");
  await page.keyboard.press("Escape");
  expect(await getText(page)).toBe("this stays lowercase\n");

  // The positive control: with the option back on and no `metaKey`, the
  // very same event does land — so the two assertions above are the gates
  // holding, not a setup that could never have fired.
  await page.evaluate(() => {
    window.__helixTerminal.options.macOptionIsMeta = true;
  });
  await page.keyboard.press("x"); // the round trip above collapsed the selection
  await dispatchKey(page, {
    key: "Dead",
    code: "Backquote",
    keyCode: 229,
    altKey: true,
  });
  await expect
    .poll(() => getText(page), { message: "the positive control never fired" })
    .toBe("THIS STAYS LOWERCASE\n");
});

test("a Firefox-numbered punctuation Alt chord reaches the editor off macOS (issue #137)", async ({
  page,
}) => {
  await bootAsPlatform(page, "Linux x86_64");
  await openFile(page, "flip.txt", "alpha beta\n");

  // One selection, anchor before head.
  await page.keyboard.press("w");
  await expect
    .poll(() => getState(page).then((s) => s.selections.length))
    .toBe(1);
  const before = await getState(page);

  // What a real Alt-; delivers in Firefox: `key` is the layout's own
  // character, `keyCode` is Gecko's `DOM_VK_SEMICOLON` (59) rather than the
  // 186 xterm's table knows. Chrome honors `keyCode` from the init dict and
  // its table misses 59 just as Firefox's does, so this shape is faithful
  // where it runs: xterm emits nothing for it, and only the custom key
  // handler — reading `key`, not `keyCode` — can land the chord.
  await dispatchKey(page, {
    key: ";",
    code: "Semicolon",
    keyCode: 59,
    altKey: true,
  });

  // A-; is `flip_selections`: anchor and head trade places.
  await expect
    .poll(() => getState(page).then((s) => s.selections[0].head), {
      message: "A-; did not reach the editor",
    })
    .toBe(before.selections[0].anchor);
  expect((await getState(page)).selections[0].anchor).toBe(
    before.selections[0].head,
  );
});

test("a Windows AltGr character on a handled row is left to xterm (issue #137)", async ({
  page,
}) => {
  // AltGr reaches the browser as `altKey && ctrlKey` on Windows (Chrome
  // and Firefox alike), and as `getModifierState("AltGraph")`; `key` is
  // already the third-level character — `{` on a German `AltGr-7`. xterm
  // drops the keydown in `_isThirdLevelShift` so the keypress can insert
  // it, but that check runs after the custom handler: the handler has to
  // bail on its own, or the character is forwarded as `C-A-{` and
  // cancelled. Asserted in insert mode, where a forwarded chord would be
  // the most visible as a lost keystroke — and where `preventDefault()`
  // would otherwise stop the keypress from ever being seen.
  await bootAsPlatform(page, "Win32");
  await openFile(page, "altgr.txt", "alpha\n");

  await page.keyboard.press("i");
  await expect.poll(() => getState(page).then((s) => s.mode)).toBe("insert");

  for (const shape of [{ ctrlKey: true }, { modifierAltGraph: true }]) {
    const cancelled = await page.evaluate(
      (event) =>
        !window.__helixTerminal.textarea.dispatchEvent(
          new KeyboardEvent("keydown", {
            ...event,
            bubbles: true,
            cancelable: true,
          }),
        ),
      { key: "{", code: "Digit7", keyCode: 55, altKey: true, ...shape },
    );
    expect(
      cancelled,
      `AltGr shape ${JSON.stringify(shape)} was cancelled`,
    ).toBe(false);
  }

  // A real keypress would follow each keydown and insert the character;
  // synthetic events carry none. The positive control that the page is
  // still listening: a plain keystroke after the two lands, and nothing
  // from the AltGr events — a forwarded `C-A-{` would have left insert
  // mode or eaten the stroke.
  await page.keyboard.type("Z");
  await expect.poll(() => getText(page)).toBe("Zalpha\n");
  expect((await getState(page)).mode).toBe("insert");
});

test("a macOS-composed punctuation Alt chord resolves off the physical key (issue #137)", async ({
  page,
}) => {
  await bootAsMac(page);
  await openFile(page, "ellipsis.txt", "alpha beta\n");

  await page.keyboard.press("w");
  await expect
    .poll(() => getState(page).then((s) => s.selections.length))
    .toBe(1);
  const before = await getState(page);

  // What a real Option-; delivers on macOS (US layout): the composed
  // ellipsis in `key`, the physical key in `code`. Here `key` names no
  // binding, so the handler falls back to the `code`-keyed US table — the
  // same table the dead-key cases above use — and forwards `A-;`.
  await dispatchKey(page, {
    key: "…",
    code: "Semicolon",
    keyCode: 59,
    altKey: true,
  });

  await expect
    .poll(() => getState(page).then((s) => s.selections[0].head), {
      message: "A-; did not reach the editor as the US-position character",
    })
    .toBe(before.selections[0].anchor);
});

test("a letter Alt chord still lands exactly once off macOS", async ({
  page,
}) => {
  // The handler now owns the punctuation and digit rows on every platform,
  // and the letters are still xterm's. This is the ownership boundary from
  // the other side: a plain `code: "KeyU"` chord must keep going through
  // xterm and `onKey` alone — a handler that overreached into `Key*` would
  // run the binding twice off one keystroke.
  await bootAsPlatform(page, "Linux x86_64");
  await openFile(page, "once.txt", "alpha\n");

  await page.keyboard.press("i");
  await page.keyboard.type("X");
  await page.keyboard.press("Escape");
  await page.keyboard.press("i");
  await page.keyboard.type("Y");
  await page.keyboard.press("Escape");
  await expect.poll(() => getText(page)).toBe("XYalpha\n");

  await dispatchKey(page, {
    key: "u",
    code: "KeyU",
    keyCode: 85,
    altKey: true,
  });

  // A-u is `earlier`: one step back strips the "Y" and not the "X".
  await expect
    .poll(() => getText(page), { message: "A-u did not reach the editor" })
    .toBe("Xalpha\n");
  await page.keyboard.press("i");
  await expect.poll(() => getState(page).then((s) => s.mode)).toBe("insert");
  await page.keyboard.press("Escape");
  expect(await getText(page)).toBe("Xalpha\n");
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

// The composition a macOS dead key starts, and the Firefox half of it
// (issue #142). Firefox on macOS begins the dead key's IME composition
// even after xterm cancelled the keydown, and xterm's `CompositionHelper`
// would show the accent at the cursor and paste it on `compositionend`;
// main.js swallows an Option-started composition before it gets there.
// The events are synthetic for the same reason `dispatchKey`'s are: the
// renderer never goes through the OS input method (issue #139), and
// Chromium does not start this composition at all, so the sequence below
// is the one Gecko fires, read off xterm's own `CompositionHelper`
// handling rather than captured from a run.
//
// Resolves to whether xterm's `.composition-view` — the overlay that draws
// the in-progress text at the cursor cell — was active mid-composition,
// which is the only moment it can be: `compositionend` deactivates it.
const dispatchComposition = (page, data) =>
  page.evaluate((text) => {
    const textarea = window.__helixTerminal.textarea;
    const fire = (type) =>
      textarea.dispatchEvent(
        new CompositionEvent(type, { data: text, bubbles: true }),
      );
    fire("compositionstart");
    fire("compositionupdate");
    const shown = document
      .querySelector(".xterm .composition-view")
      .classList.contains("active");
    // Gecko commits the composed text into the textarea before
    // `compositionend`; that value is what xterm's helper diffs out.
    textarea.value += text;
    fire("compositionend");
    return shown;
  }, data);

const textareaValue = (page) =>
  page.evaluate(() => window.__helixTerminal.textarea.value);

test("an Option dead key's composition is kept away from xterm (issue #142)", async ({
  page,
}) => {
  await bootAsMac(page);
  await openFile(page, "word.txt", "alpha\n");
  await page.keyboard.press("i");
  await page.keyboard.type("X");
  await page.keyboard.press("Escape");
  await page.keyboard.press("i");
  await page.keyboard.type("Y");
  await page.keyboard.press("Escape");
  await expect.poll(() => getText(page)).toBe("XYalpha\n");

  // The same `A-u` as above, followed by what Firefox does next: the
  // composition the dead key opened, ending on the accent it composes.
  await dispatchKey(page, {
    key: "Dead",
    code: "KeyU",
    keyCode: 229,
    altKey: true,
  });
  // xterm's helper never activated — no `¨` drawn at the cursor.
  expect(
    await dispatchComposition(page, "¨"),
    "xterm drew the Option composition at the cursor",
  ).toBe(false);

  // The chord still lands, once.
  await expect
    .poll(() => getText(page), { message: "A-u did not reach the editor" })
    .toBe("Xalpha\n");
  // The accent Gecko committed is gone from the textarea — back to what
  // the keystrokes before it had left there, which xterm never clears —
  // so nothing is there for a later diff to send as input.
  expect(await textareaValue(page)).toBe("XY");
  // xterm's `compositionend` send is a `setTimeout(0)`; a round trip
  // through the editor in insert mode is past it, and what arrives is the
  // typed character only — no pasted `¨` before or after it.
  await page.keyboard.press("i");
  await expect.poll(() => getState(page).then((s) => s.mode)).toBe("insert");
  await page.keyboard.type("Z");
  await expect.poll(() => getText(page)).toBe("XZalpha\n");

  // The swallow ends with the composition: a real IME composition right
  // after — no Option keydown in front of it — still reaches the editor as
  // a paste and inserts.
  await dispatchComposition(page, "é");
  await expect
    .poll(() => getText(page), {
      message: "a real composition after an Option one was swallowed too",
    })
    .toBe("XZéalpha\n");
});

test("a composition with no Option keydown in front of it is still pasted", async ({
  page,
}) => {
  await bootAsMac(page);
  await openFile(page, "word.txt", "alpha\n");
  await page.keyboard.press("i");
  await expect.poll(() => getState(page).then((s) => s.mode)).toBe("insert");

  // The path docs/limitations.md's "IME and other composed input arrive as a paste"
  // entry describes, pinned so the swallow above cannot widen into it.
  await dispatchComposition(page, "日本");
  await expect
    .poll(() => getText(page), {
      message: "an IME composition no longer reaches the editor",
    })
    .toBe("日本alpha\n");
});

test("an Option dead key that never composed does not swallow the next composition", async ({
  page,
}) => {
  await bootAsMac(page);
  await openFile(page, "word.txt", "alpha\n");

  // Chromium and Safari: the cancelled keydown starts no composition at
  // all, so the flag the keydown set has nothing to clear it — until the
  // next keydown that is not itself inside a composition. Here that is
  // `i`, which also puts the editor in insert mode for the composition
  // after it.
  await dispatchKey(page, {
    key: "Dead",
    code: "KeyU",
    keyCode: 229,
    altKey: true,
  });
  await page.keyboard.press("i");
  await expect.poll(() => getState(page).then((s) => s.mode)).toBe("insert");
  await dispatchComposition(page, "é");
  await expect
    .poll(() => getText(page), {
      message: "a stale Option flag swallowed a real composition",
    })
    .toBe("éalpha\n");
});

test("a keydown inside an Option dead key's composition does not leave a stray DEL", async ({
  page,
}) => {
  await bootAsMac(page);
  await openFile(page, "word.txt", "alpha\n");
  await page.keyboard.press("i");
  await page.keyboard.type("X");
  await page.keyboard.press("Escape");
  await page.keyboard.press("i");
  await page.keyboard.type("Y");
  await page.keyboard.press("Escape");
  await expect.poll(() => getText(page)).toBe("XYalpha\n");

  // `A-u` then `i` before the composition closes — Gecko feeds the `i`
  // into the dead key's composition (`keyCode` 229, `isComposing`) and
  // delivers that keydown *before* the `compositionend` it triggers. With
  // the helper never told a composition started, xterm takes the
  // non-composing 229 branch (CompositionHelper.ts:113-117): it snapshots
  // the textarea's `¨` and diffs it in a `setTimeout(0)`. If the swallowed
  // `compositionend` had emptied the textarea by then, the diff would read
  // as a shrink and xterm would emit `C0.DEL`, pasted as `\x7f`; the
  // host page puts the textarea back to what that keydown saw instead.
  // Same caveat as `dispatchComposition`: this is Gecko's sequence as
  // read off xterm's source, not captured from a run.
  await dispatchKey(page, {
    key: "Dead",
    code: "KeyU",
    keyCode: 229,
    altKey: true,
  });
  await page.evaluate(() => {
    const textarea = window.__helixTerminal.textarea;
    const fire = (type, data) =>
      textarea.dispatchEvent(
        new CompositionEvent(type, { data, bubbles: true }),
      );
    fire("compositionstart", "");
    fire("compositionupdate", "¨");
    textarea.value += "¨";
    textarea.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "i",
        code: "KeyI",
        keyCode: 229,
        isComposing: true,
        bubbles: true,
        cancelable: true,
      }),
    );
    fire("compositionupdate", "ï");
    textarea.value = textarea.value.slice(0, -1) + "ï";
    fire("compositionend", "ï");
  });

  // The chord landed once; the `i` was eaten by the composition, as in
  // Firefox; and nothing else arrived — a pasted DEL would have deleted a
  // second character.
  await expect
    .poll(() => getText(page), { message: "A-u did not reach the editor" })
    .toBe("Xalpha\n");
  // The textarea holds what the inner keydown saw — the `¨` included, so
  // xterm's diff of it comes out empty — not the `ï` Gecko committed.
  expect(await textareaValue(page)).toBe("XY¨");
  // Past xterm's deferred diff: a round trip through insert mode, with the
  // buffer still holding the `X` a stray DEL would have taken.
  await page.keyboard.press("i");
  await expect.poll(() => getState(page).then((s) => s.mode)).toBe("insert");
  await page.keyboard.type("Z");
  await expect
    .poll(() => getText(page), { message: "a stray DEL reached the editor" })
    .toBe("XZalpha\n");
});
