// Mouse and focus forwarding (issue #50): clicks, drags, and wheel scrolls
// go through Playwright's real pointer events, so they exercise the true
// xterm → SGR report → `mouse_event()` path; focus reports are injected
// synthetically (see that test's comment). Assertions read editor state
// (`window.helixState`), except the wheel and focus tests, which must
// watch the rendered terminal cells.
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

// The rendered style of one terminal grid row, one entry per cell
// (inverse flag plus fg/bg color). helix draws its block cursor manually
// as a cell style — and only while the terminal is focused — so the
// cursor's row renders differently focused vs. unfocused.
const rowStyles = (page, gridRow) =>
  page.evaluate((row) => {
    const line = window.__helixTerminal.buffer.active.getLine(row);
    const cells = [];
    for (let i = 0; i < line.length; i += 1) {
      const cell = line.getCell(i);
      cells.push(
        [
          cell.isInverse(),
          cell.getFgColorMode(),
          cell.getFgColor(),
          cell.getBgColorMode(),
          cell.getBgColor(),
        ].join(":"),
      );
    }
    return cells;
  }, gridRow);

// Focus forwarding (also issue #50): real OS focus/blur is unreliable
// under headless automation, so push synthetic \x1b[O / \x1b[I reports
// through xterm.js's input() — the same onData path the emulator's own
// focus reports take (the fromKey guard only skips keystroke-produced
// data). This exercises the report regex and the `focus_event()` export;
// the assertion watches helix drop and redraw its block cursor highlight.
test("synthetic focus reports toggle the block cursor highlight", async ({
  page,
}) => {
  await bootEditor(page);

  // Scratch buffer, cursor at (0, 0) — the cursor highlight is on grid
  // row 0.
  const focused = await rowStyles(page, 0);

  await page.evaluate(() => window.__helixTerminal.input("\x1b[O", false));
  await expect
    .poll(() => rowStyles(page, 0), {
      message: "focus-out report did not reach the editor",
    })
    .not.toEqual(focused);

  await page.evaluate(() => window.__helixTerminal.input("\x1b[I", false));
  await expect
    .poll(() => rowStyles(page, 0), {
      message: "focus-in report did not reach the editor",
    })
    .toEqual(focused);
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
