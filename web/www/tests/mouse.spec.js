// Mouse forwarding (issue #50): clicks, drags, and wheel scrolls go through
// Playwright's real pointer events, so they exercise the true
// xterm → SGR report → `mouse_event()` path. Assertions read editor state
// (`window.helixState`), except the wheel test, which must watch the
// rendered viewport move.
import { test, expect } from "@playwright/test";

const getState = (page) =>
  page.evaluate(() => window.helixState.state());

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

// Open a 200-line file so there is room to click, drag, and scroll.
async function openTallFile(page) {
  await page.evaluate(() => {
    const lines = [];
    for (let i = 1; i <= 200; i += 1) {
      lines.push(`line ${String(i).padStart(3, "0")} abcdefghijklmnop`);
    }
    window.helixVfs.write("tall.txt", `${lines.join("\n")}\n`);
  });
  await page.keyboard.type(":o tall.txt");
  await page.keyboard.press("Enter");
  await expect
    .poll(() => getState(page).then((s) => s.path))
    .toBe("/tall.txt");
}

// Center-of-cell page coordinates for a terminal grid position, from the
// rendered grid's own geometry.
const cellToPage = (page, col, row) =>
  page.evaluate(
    ([c, r]) => {
      const terminal = window.__helixTerminal;
      const rect = terminal.element
        .querySelector(".xterm-screen")
        .getBoundingClientRect();
      return {
        x: rect.left + ((c + 0.5) * rect.width) / terminal.cols,
        y: rect.top + ((r + 0.5) * rect.height) / terminal.rows,
      };
    },
    [col, row],
  );

// The file line number ("001"...) shown on the terminal's first grid row —
// the viewport's scroll position, read from rendered text.
const firstVisibleLine = (page) =>
  page.evaluate(() => {
    const buffer = window.__helixTerminal.buffer.active;
    for (let i = 0; i < buffer.length; i += 1) {
      const match = buffer
        .getLine(i)
        .translateToString(true)
        .match(/line (\d+)/);
      if (match) return match[1];
    }
    return null;
  });

test("click places the cursor on the clicked row", async ({ page }) => {
  await bootEditor(page);
  await openTallFile(page);

  expect((await getState(page)).cursor).toEqual({ row: 0, col: 0 });

  // The view starts at the top, so grid row 5 shows buffer row 5 (the
  // gutter only offsets columns).
  const target = await cellToPage(page, 12, 5);
  await page.mouse.click(target.x, target.y);
  await expect
    .poll(() => getState(page).then((s) => s.cursor.row))
    .toBe(5);
});

test("drag sweeps out a selection", async ({ page }) => {
  await bootEditor(page);
  await openTallFile(page);

  const from = await cellToPage(page, 8, 2);
  const to = await cellToPage(page, 16, 6);
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(to.x, to.y, { steps: 8 });
  await page.mouse.up();

  await expect
    .poll(async () => {
      const state = await getState(page);
      const { anchor, head } = state.selections[0];
      return { spans: head !== anchor, endRow: state.cursor.row };
    })
    .toEqual({ spans: true, endRow: 6 });
});

test("wheel scrolls the viewport and back", async ({ page }) => {
  await bootEditor(page);
  await openTallFile(page);

  await expect.poll(() => firstVisibleLine(page)).toBe("001");

  const over = await cellToPage(page, 20, 10);
  await page.mouse.move(over.x, over.y);
  await page.mouse.wheel(0, 120);
  await expect
    .poll(() => firstVisibleLine(page), {
      message: "wheel-down did not scroll the view",
    })
    .not.toBe("001");

  await page.mouse.wheel(0, -120);
  await expect.poll(() => firstVisibleLine(page)).toBe("001");
});
