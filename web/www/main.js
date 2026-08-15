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

terminal.onKey(({ domEvent }) => {
  key_event(
    domEvent.key,
    domEvent.ctrlKey,
    domEvent.altKey,
    domEvent.shiftKey,
    domEvent.metaKey,
  );
});
// Browser-native paste (ctrl/cmd-v reaches xterm.js as a paste, not a key).
// Single characters and ESC-prefixed sequences are ordinary keystrokes,
// already delivered through onKey above.
terminal.onData((data) => {
  if (data.length > 1 && !data.startsWith("\x1b")) {
    paste(data);
  }
});
terminal.onResize(({ cols, rows }) => resize(cols, rows));
window.addEventListener("resize", () => fitAddon.fit());
