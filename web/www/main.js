import "@xterm/xterm/css/xterm.css";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
// `init` fetches and instantiates the wasm module; the named exports are the
// unstable JS surface of the `helix-web` crate (web/src/session.rs).
import init, {
  start,
  key_event,
  paste,
  resize,
  vfs_write,
  vfs_read,
  vfs_list,
  editor_state,
  editor_text,
} from "helix-web";

// The one color every background surface derives from: xterm's default
// background (which is also what helix's default cells render with — its
// base16 fallback theme sets no ui.background, so cells stay on the
// terminal default) and the partial-cell strips the integer cell grid
// leaves around the editor. The page background in index.html mirrors this
// value so those strips blend into the editor instead of framing it.
const BACKGROUND = "#000000";

const terminal = new Terminal({
  // helix owns the whole grid; there is no history to scroll back to.
  scrollback: 0,
  fontFamily: "'Fira Code', Menlo, Consolas, monospace",
  fontSize: 18,
  theme: { background: BACKGROUND },
});
const fitAddon = new FitAddon();
terminal.loadAddon(fitAddon);
terminal.open(document.getElementById("terminal"));
fitAddon.fit();
terminal.focus();

await init();
start((bytes) => terminal.write(bytes), terminal.cols, terminal.rows);

// For a keystroke xterm.js fires onKey and then, synchronously, onData with
// the sequence that key produced. Pastes (and IME-composed text) arrive
// through onData alone — a length heuristic can't tell a one-character paste
// from a keystroke, so track "the next onData is this keystroke's own
// sequence" explicitly instead.
let dataIsFromKey = false;
terminal.onKey(({ domEvent }) => {
  dataIsFromKey = true;
  key_event(
    domEvent.key,
    domEvent.ctrlKey,
    domEvent.altKey,
    domEvent.shiftKey,
    domEvent.metaKey,
  );
});
// Browser-native paste (ctrl/cmd-v reaches xterm.js as a paste, not a key).
// helix enables bracketed paste mode at boot, so xterm.js delivers pasted
// text wrapped in the \x1b[200~ ... \x1b[201~ markers — unwrap those and
// forward the payload. Other ESC-prefixed payloads are skipped: xterm.js
// answers terminal queries (cursor position, device attributes) through
// onData with ESC sequences. Bare non-ESC data that no keystroke produced
// (IME-composed text, paste with bracketed mode off) is forwarded as-is.
const BRACKETED_START = "\x1b[200~";
const BRACKETED_END = "\x1b[201~";
terminal.onData((data) => {
  const fromKey = dataIsFromKey;
  dataIsFromKey = false;
  if (fromKey) {
    return;
  }
  if (data.startsWith(BRACKETED_START) && data.endsWith(BRACKETED_END)) {
    paste(data.slice(BRACKETED_START.length, -BRACKETED_END.length));
  } else if (!data.startsWith("\x1b")) {
    paste(data);
  }
});
terminal.onResize(({ cols, rows }) => resize(cols, rows));
window.addEventListener("resize", () => fitAddon.fit());

// Smoke-test hook: lets a browser-automation harness read the terminal
// buffer (text and colors) to assert on rendered output. Not part of the
// page's own behavior.
window.__helixTerminal = terminal;

// The virtual file system the editor's documents live in — usable from the
// devtools console (inject a file, then `:o` it; `:w` saves land here).
// Like `__helixTerminal` above, also a natural assertion surface for a
// browser-automation harness. Note `write` throws on paths that name no
// file (`""`, `"."`, `"/"`, ...).
window.helixVfs = { write: vfs_write, read: vfs_read, list: vfs_list };

// Read-only editor state inspection (issue #18): `state()` returns
// { mode, path, cursor: { row, col }, selections: [{ anchor, head }] },
// `text()` the focused buffer's live text (unsaved edits included — unlike
// `helixVfs.read`, which sees what was last saved). Both return `undefined`
// when helix is not running, and throw if called from inside the editor's
// own output callback (defer to a microtask there). The intended assertion
// surface for embedders and browser-automation harnesses.
window.helixState = { state: editor_state, text: editor_text };
