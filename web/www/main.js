import "@xterm/xterm/css/xterm.css";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
// `init` fetches and instantiates the wasm module; the named exports are the
// unstable JS surface of the `helix-web` crate (web/src/session.rs).
import init, { start, key_event, paste, resize } from "helix-web";

const terminal = new Terminal({
  // helix owns the whole grid; there is no history to scroll back to.
  scrollback: 0,
  fontFamily: "'Fira Code', Menlo, Consolas, monospace",
  fontSize: 18,
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
