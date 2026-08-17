// How long opening a file can freeze the page. Syntax highlighting runs on
// the main thread, so a buffer that is expensive to highlight is a buffer the
// tab stops responding for — the two cases here are the two ways that went
// wrong, and neither is about the other's mechanism.
//
// Case 1 (issue #77) is the parse, which helix gives a 500 ms budget. The C
// side reads `clock_gettime`, which on wasm32 is a shim over the page's
// `performance.now()` (sysroot/shims.c → the web crate's `clock` module).
// While that shim was frozen at zero, elapsed time never grew, the budget
// never expired, and a file too big to parse in time parsed to completion
// instead of degrading to no highlighting.
//
// Case 2 (issue #92) is everything the budget does not cover: after a parse
// succeeds, tree-house runs the injection and local queries over the finished
// tree with no deadline at all. Its payload is pathological rather than large.
//
// The #77 payload is deliberately the opposite — plainly too large, nothing
// odd about its shape: tree-sitter samples the deadline once per 100 parse
// operations, so what a working timeout bounds is a parse that makes many
// small steps. On the frozen clock this file took 2.9s to open here; with a
// real clock it takes ~0.6s, the budget plus the read. If a future machine
// ever parses 5 MB inside 500 ms this test fails on the warning below — grow
// the file then.
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
      // A fixed interval, unlike the escalating default (100 / 250 / 500 /
      // 1000 ms): the stopwatch below reads whatever poll boundary the open
      // lands past, and on the default a 1.1s open is not seen until 1.85s.
      // That quantization has nothing to do with what is under test.
      intervals: [100],
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

// Issue #92: the other half of that story. This file is 100 kB, parses in
// milliseconds and never comes near the timeout — the freeze was after the
// parse, in the injection and local queries tree-house runs over the finished
// tree. Unbalanced delimiters put every token in one flat ERROR node, and
// tree-sitter's tree cursor answered "does this node have a later sibling"
// by scanning the rest of that node's children, once per child. Quadratic:
// this file took 26s to open, 50k took 7s, 200k took 102s. The vendored
// tree-sitter summarizes the child list once instead (delta 2 in
// stubs/tree-house-bindings/Cargo.toml).
const UNBALANCED_RUST = "(".repeat(100_000);

// Generous against the ~150ms this costs now, and nowhere near the 26s it
// cost before: the point is the shape of the curve, not the constant.
const UNBALANCED_BUDGET_MS = 3_000;

test("a file of unbalanced delimiters opens without freezing the page (issue #92)", async ({
  page,
}) => {
  await bootEditor(page);
  await page.evaluate(
    (contents) => window.helixVfs.write("/unbalanced.rs", contents),
    UNBALANCED_RUST,
  );

  const started = Date.now();
  await page.keyboard.type(":o /unbalanced.rs");
  await page.keyboard.press("Enter");
  await expect
    .poll(() => getState(page).then((state) => state.path), {
      message: "the unbalanced file never finished opening",
      timeout: 180_000,
      intervals: [100],
    })
    .toBe("/unbalanced.rs");
  expect(Date.now() - started).toBeLessThan(UNBALANCED_BUDGET_MS);

  // Unlike the oversized file above this one is small enough to parse well
  // inside the budget, so it keeps its syntax tree — the fix is about the
  // work that runs after the parse, not about giving up earlier.
  await page.keyboard.press("i");
  await expect
    .poll(() => getState(page).then((state) => state.mode))
    .toBe("insert");
});
