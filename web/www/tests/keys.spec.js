// Alt-chord forwarding (issue #68). The plain key path is covered by the
// smoke suite's real keystrokes; what needs its own tests is the macOS
// Option handling — `macOptionIsMeta` plus the fallback that takes the
// chord's character from xterm.js when the DOM composed one of its own.
//
// Only half of that is reachable from CI. The composed-character case is
// platform-independent inside xterm (its Alt branch runs off `keyCode`, and
// only the *entry* condition looks at `isMac`), so a synthetic KeyboardEvent
// on the helper textarea reproduces it on the Linux runner. The dead-key
// case is macOS-only and skips itself elsewhere — see its comment.
import { test, expect } from "@playwright/test";

const getState = (page) => page.evaluate(() => window.helixState.state());

const getText = (page) => page.evaluate(() => window.helixState.text());

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

// One keydown straight onto xterm's helper textarea — the element its own
// key handling listens on, so the event takes the full
// `evaluateKeyboardEvent` → `onKey` → `key_event()` path. Playwright's
// keyboard cannot stand in here: it synthesizes from a US layout and never
// composes, so it can't produce the `key`/`code` divergence macOS does.
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

test("Option is claimed as Meta so Alt chords are not composed away", async ({
  page,
}) => {
  await bootEditor(page);

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
  await bootEditor(page);

  await page.evaluate(() => window.helixVfs.write("two.txt", "alpha\nbeta\n"));
  await page.keyboard.type(":o two.txt");
  await page.keyboard.press("Enter");
  await expect.poll(() => getState(page).then((s) => s.path)).toBe("/two.txt");

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
  // Only macOS composes an accent starter into a `Dead` keydown and lets it
  // through: everywhere else xterm.js swallows the event before `onKey`, so
  // there is nothing for the host page to forward and no way to fake one.
  // The browser runs on this machine, so the host platform decides.
  test.skip(
    process.platform !== "darwin",
    "dead keys only reach onKey on macOS",
  );
  await bootEditor(page);

  await page.evaluate(() => window.helixVfs.write("word.txt", "alpha\n"));
  await page.keyboard.type(":o word.txt");
  await page.keyboard.press("Enter");
  await expect.poll(() => getState(page).then((s) => s.path)).toBe("/word.txt");
  await page.keyboard.press("%");

  // Option-` on macOS: the grave accent starts a composition, so `key` is
  // "Dead" and the character only exists in `code`/`keyCode`.
  await dispatchKey(page, {
    key: "Dead",
    code: "Backquote",
    keyCode: 192,
    altKey: true,
  });

  // A-` is `switch_to_uppercase` over the selection.
  await expect
    .poll(() => getText(page), { message: "A-` did not reach the editor" })
    .toBe("ALPHA\n");
});
