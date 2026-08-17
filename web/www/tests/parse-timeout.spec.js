// Tree-sitter's parse timeout in the browser (issue #77): the C side reads
// `clock_gettime`, which on wasm32 is a shim over the page's
// `performance.now()` (sysroot/shims.c → the web crate's `clock` module).
// While that shim was frozen at zero, elapsed time never grew, helix's
// 500 ms budget never expired, and a file too big to parse in time parsed to
// completion on the main thread instead of degrading to no highlighting.
//
// The payload is a plainly-too-large file rather than a pathological one:
// tree-sitter samples the deadline once per 100 parse operations, so what a
// working timeout bounds is a parse that makes many small steps. On the
// frozen clock this file took 2.9s to open here; with a real clock it takes
// ~0.6s, the budget plus the read. If a future machine ever parses 5 MB
// inside 500 ms this test fails on the warning below — grow the file then.
import { test, expect } from "@playwright/test";
import { bootEditor, getState } from "./helpers.js";

const OVERSIZED_RUST = "fn main() {}\n".repeat(400_000); // ~5 MB

// Comfortably under the frozen clock's 2.9s and over the ~0.6s a timed-out
// parse costs. The post-fix cost is bounded by the budget rather than by the
// file, so a slow runner does not close the gap.
const OPEN_BUDGET_MS = 2_000;

test("an oversized file gives up on highlighting instead of parsing to completion", async ({
  page,
}) => {
  const console_messages = [];
  page.on("console", (message) => console_messages.push(message.text()));

  await bootEditor(page);
  await page.evaluate(
    (contents) => window.helixVfs.write("/oversized.rs", contents),
    OVERSIZED_RUST,
  );

  const started = Date.now();
  await page.keyboard.type(":o /oversized.rs");
  await page.keyboard.press("Enter");
  await expect
    .poll(() => getState(page).then((state) => state.path), {
      message: "the oversized file never finished opening",
      timeout: 30_000,
    })
    .toBe("/oversized.rs");
  expect(Date.now() - started).toBeLessThan(OPEN_BUDGET_MS);

  // The parse was abandoned at the deadline, so helix drops the syntax tree
  // for the buffer and logs why. This is what distinguishes a timeout that
  // fired from a machine that was merely fast.
  expect(console_messages.join("\n")).toMatch(/timeout was exceeded/i);

  // And the editor is still taking input rather than sitting in a parse.
  await page.keyboard.press("i");
  await expect
    .poll(() => getState(page).then((state) => state.mode))
    .toBe("insert");
});
