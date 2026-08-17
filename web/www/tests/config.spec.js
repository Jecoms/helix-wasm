// `config.toml` in the browser (issue #75). Before this, the two config
// reads went through `std::fs`, which is unconditionally an error on wasm32:
// the default keymap was the only keymap a page could ever have, and
// `:config-reload` answered "operation not supported on this platform".
// Both files are read out of the virtual file system now, so what these
// cover is the whole channel — seeded at boot, merged the way native helix
// merges them, and re-read on `:config-reload` — with a `[keys]` remap as
// the load-bearing case, since that is the one nothing else could reach.
import { test, expect } from "@playwright/test";
import {
  bootEditor,
  bootWithConfig,
  getState,
  getText,
  terminalText,
  topLeftBg,
  vfsRead,
} from "./helpers.js";

// Where helix reads the user config from on wasm32 (`helix_loader::config_dir()`
// plus `config.toml`), and where `start`'s config argument lands.
const CONFIG = "/.config/helix/config.toml";
// The workspace half, relative to the boot working directory (`/`): nothing
// on wasm32 can hold a `.git`/`.helix` marker for `find_workspace` to find,
// so the workspace is always the working directory.
const WORKSPACE_CONFIG = "/.helix/config.toml";

// `y` is remapped to `insert_mode` throughout: the mode is the one editor
// state a keymap can change that `helixState` reports directly, and yank —
// its default binding — leaves both the mode and the buffer alone, so the
// unconfigured case is a clean negative.
const REMAP = '[keys.normal]\ny = "insert_mode"\n';

const modeAfter = async (page, key) => {
  await page.keyboard.press(key);
  return (await getState(page)).mode;
};

const reloadConfig = async (page) => {
  await page.keyboard.type(":config-reload");
  await page.keyboard.press("Enter");
  await expect.poll(() => terminalText(page)).toContain("Config refreshed");
};

test("without a config the default keymap is what runs", async ({ page }) => {
  await bootEditor(page);

  // The negative half of the test below — `y` yanks, and nothing seeded a
  // config file for it to have come from.
  expect(await modeAfter(page, "y")).toBe("normal");
  expect(await vfsRead(page, CONFIG)).toBeUndefined();
});

test("a [keys] remap from the boot config takes effect (issue #75)", async ({
  page,
}) => {
  await bootWithConfig(page, REMAP);

  expect(await modeAfter(page, "y")).toBe("insert");
  // Seeded, not parsed-and-discarded: the file is a real key, which is what
  // makes `:config-open` and `:config-reload` work on it below.
  expect(await vfsRead(page, CONFIG)).toBe(REMAP);
});

test("[editor] settings and a theme from the boot config apply", async ({
  page,
}) => {
  await bootWithConfig(
    page,
    'theme = "gruvbox"\n[editor.statusline]\nmode.normal = "CONFIGURED"\n',
  );

  // The mode indicator is the statusline's leftmost field, so a rename of it
  // can only come from `[editor]` having been read. (`:set` cannot reach the
  // statusline table at all — it takes scalar options.)
  await expect.poll(() => terminalText(page)).toContain("CONFIGURED");
  // gruvbox paints `ui.background` with bg0, the same proof the `:theme`
  // tests in smoke.spec.js use. It doubles as the true-color check: an RGB
  // theme is refused outright unless helix believes the terminal can render
  // 24-bit color, which on wasm32 nothing but `helix_term::true_color` says.
  await expect.poll(() => topLeftBg(page)).toBe(0x282828);
});

test("a malformed config is reported and the editor boots on defaults", async ({
  page,
}) => {
  const errors = [];
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(message.text());
  });
  // Booted unfocused: the statusline below is helix's own status message,
  // which the first event handled clears — and the focusing click is one.
  await bootWithConfig(page, "[keys.normal\ny = ", { focus: false });

  // Native helix prints the parse error and waits for a keypress before
  // falling back to the defaults; there is no stdin here, so it goes to the
  // statusline and (for an embedder watching, and for the reader who has
  // already typed past it) the console.
  expect(await terminalText(page)).toContain("Bad config");
  expect(errors.join("\n")).toContain("Bad config");

  // Booted, and on the defaults rather than on half a config.
  await page.locator("#terminal").click();
  expect(await getState(page).then((s) => s.mode)).toBe("normal");
  expect(await modeAfter(page, "y")).toBe("normal");
});

test(":config-reload picks up a config written after boot", async ({
  page,
}) => {
  await bootWithConfig(page, 'theme = "gruvbox"\n');
  expect(await modeAfter(page, "y")).toBe("normal");

  await page.evaluate(
    ([path, text]) => window.helixVfs.write(path, text),
    [CONFIG, `theme = "gruvbox"\n${REMAP}`],
  );
  await reloadConfig(page);

  expect(await modeAfter(page, "y")).toBe("insert");
  await page.keyboard.press("Escape");
  // The reload replaces the whole config, and on wasm32 there is no
  // COLORTERM or terminfo to re-derive true color from — so this is where an
  // override applied once at boot would have been dropped, taking the RGB
  // theme with it.
  expect(await topLeftBg(page)).toBe(0x282828);
});

test("a workspace .helix/config.toml is read and merged over the global one", async ({
  page,
}) => {
  await bootWithConfig(page, REMAP);

  await page.evaluate(
    ([path, text]) => window.helixVfs.write(path, text),
    [WORKSPACE_CONFIG, '[keys.normal]\nu = "insert_mode"\n'],
  );
  await reloadConfig(page);

  // Both files applied: the workspace one merges onto the global one rather
  // than replacing it.
  expect(await modeAfter(page, "u")).toBe("insert");
  await page.keyboard.press("Escape");
  expect(await modeAfter(page, "y")).toBe("insert");
});

test("the workspace config follows the working directory, marker or not", async ({
  page,
}) => {
  await bootWithConfig(page, REMAP);

  // `find_workspace` looks for a `.git`/`.jj`/`.helix` marker on the real
  // filesystem, and nothing on wasm32 answers — so the workspace is always
  // the working directory, and `:cd` moves which `.helix/config.toml` counts.
  // The decoy at the boot directory is what separates "followed the cwd" from
  // "found a marker at `/`": it stays behind, and its remap must not apply.
  await page.evaluate(
    ([path, text]) => window.helixVfs.write(path, text),
    [WORKSPACE_CONFIG, '[keys.normal]\n";" = "insert_mode"\n'],
  );
  await page.keyboard.type(":cd /project");
  await page.keyboard.press("Enter");
  await page.evaluate(
    ([path, text]) => window.helixVfs.write(path, text),
    ["/project/.helix/config.toml", '[keys.normal]\nu = "insert_mode"\n'],
  );
  await reloadConfig(page);

  expect(await modeAfter(page, "u")).toBe("insert");
  await page.keyboard.press("Escape");
  expect(await modeAfter(page, ";")).toBe("normal");
  // The global config is not directory-scoped and still applies.
  expect(await modeAfter(page, "y")).toBe("insert");
});

test(":config-open edits the live config, and :w + :config-reload apply it", async ({
  page,
}) => {
  // No trailing newline: `ge` has to land on the last remap, not on the
  // empty line one would leave behind.
  await bootWithConfig(
    page,
    '[keys.normal]\nz = "insert_mode"\ny = "insert_mode"',
  );

  // The issue's other half: `:config-open` opened a permanently empty buffer,
  // because nothing ever wrote that key.
  await page.keyboard.type(":config-open");
  await page.keyboard.press("Enter");
  await expect.poll(() => getState(page).then((s) => s.path)).toBe(CONFIG);
  expect(await getText(page)).toContain('y = "insert_mode"');

  // Drop the last line (the `y` remap) and save. Deleting rather than typing
  // keeps the edit clear of auto-pairs mangling quotes and brackets.
  await page.keyboard.press("g");
  await page.keyboard.press("e");
  await page.keyboard.press("x");
  await page.keyboard.press("d");
  await page.keyboard.type(":w");
  await page.keyboard.press("Enter");
  await expect.poll(() => vfsRead(page, CONFIG)).not.toContain("y =");

  await reloadConfig(page);
  // The removed remap is gone and the rest of the file still applies.
  expect(await modeAfter(page, "y")).toBe("normal");
  expect(await modeAfter(page, "z")).toBe("insert");
});
