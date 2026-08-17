// The exit teardown (issue #69). `:q` runs `Application::close` — wait out
// the jobs that asked to be waited on, flush pending writes, shut the
// language servers down — and that last step built a `tokio::time::timeout`,
// whose timer calls `Instant::now()`, unimplemented on
// wasm32-unknown-unknown. The trap took the instance with it: no exit code,
// no notice, and a wasm-bindgen-futures queue left borrowed, so every later
// poll panicked too.
//
// So these specs assert the teardown *runs to the end* rather than merely
// that something got printed. `tutor.spec.js` covers the reader-facing half
// (the notice and the exit callback, as tutor chapter 1.2 lands on it); what
// is here is the work `close()` does on the way there, and the state the
// module is left in afterwards.
import { test, expect } from "@playwright/test";
import { bootEditor, getState, terminalText, vfsRead } from "./helpers.js";

// Collects everything that would mean the instance died: a wasm trap reaches
// the page as an uncaught error, and the panic itself reaches the console
// through console_error_panic_hook first.
function watchForPanics(page) {
  const panics = [];
  page.on("pageerror", (error) => panics.push(String(error.message)));
  page.on("console", (message) => {
    if (message.text().includes("panicked")) panics.push(message.text());
  });
  return panics;
}

test(":wq runs the whole teardown: the write lands and the exit completes", async ({
  page,
}) => {
  const panics = watchForPanics(page);
  await bootEditor(page);

  await page.keyboard.press("i");
  await page.keyboard.type("teardown");
  await page.keyboard.press("Escape");
  await page.keyboard.type(":wq /close-spec.txt");
  await page.keyboard.press("Enter");

  // Step two of `close()` is `flush_writes`, which drains the save queue —
  // so a `:wq` that reaches its exit code has been through it. Assert the
  // file, not just the exit: a teardown that skipped the flush could still
  // announce itself.
  await expect.poll(() => page.evaluate(() => window.helixExit)).toEqual({
    code: 0,
  });
  expect(await vfsRead(page, "/close-spec.txt")).toBe("teardown\n");
  expect(panics).toEqual([]);
});

test("the module is still callable after the exit, not poisoned", async ({
  page,
}) => {
  const panics = watchForPanics(page);
  await bootEditor(page);

  // How long a keystroke really takes to land here, so the "nothing was
  // drawn after the exit" check below can outwait one instead of guessing a
  // window (smoke.spec.js makes the same measurement for the crash path).
  const startedAt = Date.now();
  await page.keyboard.press("i");
  await expect.poll(() => getState(page).then((s) => s.mode)).toBe("insert");
  const settle = Math.max(500, (Date.now() - startedAt) * 10);
  await page.keyboard.press("Escape");
  await expect.poll(() => getState(page).then((s) => s.mode)).toBe("normal");

  await page.keyboard.type(":q");
  await page.keyboard.press("Enter");
  await expect.poll(() => page.evaluate(() => window.helixExit)).toEqual({
    code: 0,
  });

  // The distinction this test exists for. A trapped instance and a cleanly
  // exited one look identical on screen — the difference is only visible
  // from JS, where a poisoned module throws `RuntimeError: unreachable` on
  // entry. So call across the boundary and require an answer: inspection
  // reports not-running (helix really is gone) and the vfs exports still
  // work (the module really is alive). `evaluate` surfaces a throw as a
  // rejected promise, so a trap fails these outright rather than returning
  // something falsy.
  expect(await getState(page)).toBeUndefined();
  expect(await page.evaluate(() => window.helixState.text())).toBeUndefined();
  await page.evaluate(() => window.helixVfs.write("/after-exit.txt", "still here"));
  expect(await vfsRead(page, "/after-exit.txt")).toBe("still here");

  // Keystrokes after the exit are inert by design (web/src/session.rs drops
  // them; the host page has stopped forwarding as well), so the notice is
  // still the only thing on the restored main screen — no frame from a
  // half-dead editor painted over it. Compare the written lines rather than
  // the whole buffer: xterm.js grows its blank tail asynchronously after the
  // notice scrolls the viewport, which is bookkeeping, not output.
  const written = async () =>
    (await terminalText(page)).split("\n").filter((line) => line.trim());
  await expect.poll(written).toEqual([
    "Helix has exited. Refresh the page to start a new session. (exit code 0)",
  ]);

  await page.keyboard.press("i");
  await page.keyboard.type("ignored");
  await page.waitForTimeout(settle);
  expect(await written()).toEqual([
    "Helix has exited. Refresh the page to start a new session. (exit code 0)",
  ]);
  expect(panics).toEqual([]);
});
