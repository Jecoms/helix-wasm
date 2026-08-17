// Background jobs in the browser (issue #71). `Jobs::add` used to hand every
// non-`wait` job to `tokio::spawn`, and there is no tokio runtime here, so
// queuing one panicked — and a wasm32 panic is a trap, which takes the whole
// instance down with it. These specs drive one trigger per class the issue
// names — an LSP command, a subprocess command, a job that needs neither, and
// one queued from a prompt rather than a command — and assert the editor is
// still alive afterwards: the job's own output reached the screen, and the
// next keystroke still lands.
//
// The liveness half is the point. Asserting only on the message would pass on
// a build that printed it and then wedged, so every case ends by proving the
// editor still accepts input and still redraws.
import { test, expect } from "@playwright/test";
import { bootEditor, getState, getText, terminalText } from "./helpers.js";

// Types a command line and submits it.
async function runCommand(page, command) {
  await page.keyboard.type(command);
  await page.keyboard.press("Enter");
}

// The proof of life: the editor takes a keystroke, changes state because of
// it, and paints the result. A wedged instance traps on entry, so the mode
// never moves and the last frame stays on screen — both halves are checked
// because either one alone can pass on a half-dead editor (a render with no
// state change, or a state change nothing ever drew).
async function expectStillAlive(page, marker) {
  const before = await terminalText(page);

  await page.keyboard.press("i");
  await expect.poll(() => getState(page).then((s) => s.mode)).toBe("insert");
  await page.keyboard.type(marker);
  await expect.poll(() => getText(page)).toContain(marker);
  await page.keyboard.press("Escape");
  await expect.poll(() => getState(page).then((s) => s.mode)).toBe("normal");

  await expect.poll(() => terminalText(page)).not.toBe(before);
  // The host page's crash gate never fired (web/www/main.js): a trap would
  // have replaced the screen with this notice.
  expect(await terminalText(page)).not.toContain(
    "Helix has stopped responding",
  );
}

test("gd reports no definition instead of taking the instance down", async ({
  page,
}) => {
  await bootEditor(page);

  // `goto_definition` queues its job unconditionally — an empty
  // language-server set yields an empty request stream rather than an early
  // return, so this is the bare-`g`-key crash the issue leads with. With no
  // servers configured the job resolves to helix's own "nothing found"
  // error, exactly as native helix does without an LSP.
  await page.keyboard.press("g");
  await page.keyboard.press("d");

  await expect.poll(() => terminalText(page)).toContain("No definition found");
  await expectStillAlive(page, "after gd");
});

test(":sh reports the missing subprocess support instead of wedging", async ({
  page,
}) => {
  await bootEditor(page);

  // The other class: `run_shell_command` wants a subprocess, which the
  // browser can never provide. The wasm32 `shell_impl_async` already bails
  // with this message — it was simply unreachable while queuing the job
  // panicked first.
  await runCommand(page, ":sh echo hello");

  await expect
    .poll(() => terminalText(page))
    .toContain("Shell commands are not supported on this platform");
  await expectStillAlive(page, "after sh");
});

test("a pure job (:tree-sitter-scopes) does its work rather than dying", async ({
  page,
}) => {
  await bootEditor(page);

  // Nothing about this job needs a runtime, an LSP or a subprocess — it
  // formats the indent scopes at the cursor and pops them up. It only ever
  // crashed because it went through `Jobs::add` at all, so a working popup
  // is the clearest evidence the job machinery itself runs now.
  // The popup renders the scope list as JSON; an empty scratch buffer has no
  // syntax tree, so the empty list is what draws. Two characters is a thin
  // thing to assert on, so pin the causation instead of the string: nothing
  // on screen matched before the command, and something does after.
  expect(await terminalText(page)).not.toContain("[]");
  await runCommand(page, ":tree-sitter-scopes");

  await expect.poll(() => terminalText(page)).toContain("[]");
  await expectStillAlive(page, "after scopes");
});

test("an invalid search regex pops up its error instead of wedging", async ({
  page,
}) => {
  await bootEditor(page);

  // A third entry point, and per #71 the one most likely to be hit by
  // accident: this job is queued by the search prompt's own callback
  // (`ui/mod.rs`) rather than by a command, when Enter validates a pattern
  // that does not parse.
  await page.keyboard.press("/");
  await page.keyboard.type("[");
  await page.keyboard.press("Enter");

  await expect.poll(() => terminalText(page)).toContain("error parsing pattern");
  await expectStillAlive(page, "after bad regex");
});
