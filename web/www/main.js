import "@xterm/xterm/css/xterm.css";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
// `init` fetches and instantiates the wasm module; the named exports are the
// unstable JS surface of the `helix-web` crate (web/src/session.rs).
import init, {
  start,
  on_exit,
  key_event,
  mouse_event,
  focus_event,
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
  // On macOS, Option is a compose key unless the terminal claims it as Meta,
  // and xterm.js drops the keydown outright while it doesn't — which left
  // every `A-` binding dead there (issue #68). This is the same trade
  // iTerm's "Option as Meta" makes: the chords work, and Option-composed
  // character entry (é, ß, ...) does not. Ignored off macOS.
  macOptionIsMeta: true,
  fontFamily: "'Fira Code', Menlo, Consolas, monospace",
  fontSize: 18,
  theme: { background: BACKGROUND },
});
const fitAddon = new FitAddon();
terminal.loadAddon(fitAddon);
terminal.open(document.getElementById("terminal"));
fitAddon.fit();
terminal.focus();

// A dead editor must never look like a frozen page. Whether helix exited
// cleanly or the wasm instance died, the last frame stays on screen and
// every later keystroke vanishes into nothing — identical symptoms, and
// nothing on the page to distinguish either from a hang. So input
// forwarding runs through a liveness gate: once helix is gone, the page
// stops feeding it and (for the unclean deaths, which have no notice of
// their own) says so in the terminal.
const CRASH_NOTICE =
  "Helix has stopped responding. Refresh the page to start a new session.";
let editorAlive = true;

// Undo the terminal claim before writing: an unclean death leaves the
// terminal mid-render with everything helix enabled at boot still on, so
// the notice would otherwise land on top of the frozen frame, in a terminal
// that still swallows the mouse. Mirrors the wasm side's `restore_terminal`
// (web/src/session.rs), which the clean `:q` path runs for itself — mouse
// capture (?1006l ?1015l ?1002l ?1000l, crossterm's DisableMouseCapture
// set), bracketed paste (?2004l), focus reporting (?1004l), the alternate
// screen (?1049l), and the hidden cursor (?25h). Mouse capture is the one
// that matters most here: with `editor.mouse` on by default xterm.js keeps
// reporting drags as SGR sequences rather than selecting text, so a reader
// could not copy the line they are being asked to act on.
const RESTORE_TERMINAL =
  "\x1b[?1006l\x1b[?1015l\x1b[?1002l\x1b[?1000l\x1b[?2004l\x1b[?1004l\x1b[?1049l\x1b[?25h";

function stopEditor(notice) {
  editorAlive = false;
  if (notice) {
    terminal.write(`${RESTORE_TERMINAL}\r\n${notice}\r\n`);
  }
}

function reportCrash(error) {
  if (!editorAlive) {
    return;
  }
  console.error("helix stopped responding", error);
  stopEditor(CRASH_NOTICE);
}

// Every call on the input path goes through here. After a panic the wasm
// instance is poisoned and traps on entry, so a throw is the signal that
// helix is gone — there is no other notification. (The console hooks below
// are left ungated: a devtools caller wants the real error.)
function callEditor(call) {
  if (!editorAlive) {
    return;
  }
  try {
    call();
  } catch (error) {
    reportCrash(error);
  }
}

// A panic inside the editor's own event loop surfaces as an uncaught error
// (a wasm `unreachable` trap) instead of through one of the calls above, so
// the page doesn't have to wait for the next keystroke to notice.
window.addEventListener("error", (event) => reportCrash(event.error));

await init();
// `:q` really does quit, and nothing can restart the editor in this page —
// so record the exit and announce it, both for anything scripting the page
// and for a host that wants to swap in its own "refresh to start again" UI.
// No notice from this side: the wasm module has already painted its own,
// on the restored main screen, by the time this runs.
on_exit((code) => {
  stopEditor();
  window.helixExit = { code };
  window.dispatchEvent(new CustomEvent("helix-exit", { detail: { code } }));
});
callEditor(() =>
  start((bytes) => terminal.write(bytes), terminal.cols, terminal.rows),
);

// For a keystroke xterm.js fires onKey and then, synchronously, onData with
// the sequence that key produced. Pastes (and IME-composed text) arrive
// through onData alone — a length heuristic can't tell a one-character paste
// from a keystroke, so track "the next onData is this keystroke's own
// sequence" explicitly instead.
let dataIsFromKey = false;
// xterm.js's own platform test, mirrored (@xterm/xterm 5.5
// common/Platform.ts): the fallback below only makes sense for events xterm
// resolved on its macOS path, so the two checks have to agree on what "mac"
// means.
const IS_MAC = ["Macintosh", "MacIntel", "MacPPC", "Mac68K"].includes(
  navigator.platform,
);
// For an Alt chord, macOS composes before dispatch — Option-s arrives as
// `key: "ß"`, and the letter accent starters (Option-e/i/n/u) as
// `key: "Dead"` — so `domEvent.key` names the wrong binding, or none.
// xterm.js has already resolved the chord's character itself, off the
// event's keyCode and a US layout, and hands it over as the sequence `ESC` +
// character; take it from there when the DOM's own name is unusable.
//
// The guard is narrow on two axes, both load-bearing:
//   - macOS only. Nothing else composes Option, and Chrome reports
//     US-position keyCodes for non-Latin layouts, so running this on Linux
//     would resolve a Russian `A-ф` through keyCode 65 into `A-a`
//     (`select_all_siblings`) — a live command where the chord had been
//     inert. (Windows AltGr never gets this far: xterm's
//     `_isThirdLevelShift` drops it before `onKey`.)
//   - named keys keep `domEvent.key`, because xterm encodes `A-Left` as
//     `ESC b` on macOS and a looser rule would forward that as `A-b`.
//
// Not everything is recoverable here: xterm's dead-key branch only fires for
// `code: "Key*"`, so a punctuation dead key (`` Option-` ``, i.e. `` A-` ``)
// never reaches `onKey` at all — issue #81.
//
// This decode stays in the host page rather than moving to Rust the way #56
// moved the SGR mouse decoding: its input is xterm.js's own `onKey` payload,
// which exists only in the browser, and `key_event()` takes an
// already-resolved key name rather than terminal bytes. A Rust version would
// mean exporting a second, escape-sequence-shaped entry point — the tty
// input path this port deliberately does not have.
const ALT_CHAR = /^\x1b(.)$/;
const composed = (key) =>
  IS_MAC && (key === "Dead" || /^[^\x00-\x7f]$/.test(key));
terminal.onKey(({ key, domEvent }) => {
  dataIsFromKey = true;
  let name = domEvent.key;
  if (domEvent.altKey && composed(name)) {
    name = key.match(ALT_CHAR)?.[1] ?? name;
  }
  callEditor(() =>
    key_event(
      name,
      domEvent.ctrlKey,
      domEvent.altKey,
      domEvent.shiftKey,
      domEvent.metaKey,
    ),
  );
});
// Browser-native paste (ctrl/cmd-v reaches xterm.js as a paste, not a key).
// helix enables bracketed paste mode at boot, so xterm.js delivers pasted
// text wrapped in the \x1b[200~ ... \x1b[201~ markers — unwrap those and
// forward the payload. Other ESC-prefixed payloads are scanned for the
// mouse and focus reports helix turned on at boot (mouse capture makes
// xterm.js report mouse activity as SGR \x1b[<code;col;row M/m sequences,
// focus reporting as \x1b[I / \x1b[O); the rest of an ESC payload is
// dropped — xterm.js answers terminal queries (cursor position, device
// attributes) through onData with ESC sequences too. Bare non-ESC data
// that no keystroke produced (IME-composed text, paste with bracketed
// mode off) is forwarded as-is.
const BRACKETED_START = "\x1b[200~";
const BRACKETED_END = "\x1b[201~";
// Global: xterm.js batches several reports into one onData chunk during a
// drag or a wheel flick, so every match gets forwarded, not just the first.
// Mouse (SGR \x1b[<code;col;row M/m) and focus (\x1b[I / \x1b[O) reports
// share one alternation so a single scan forwards them in stream order —
// the bridge queues events in call order, and two separate passes would
// reorder a mixed chunk to all-mouse-then-focus.
const INPUT_REPORT = /\x1b\[(?:<(\d+);(\d+);(\d+)([Mm])|([IO]))/g;
terminal.onData((data) => {
  const fromKey = dataIsFromKey;
  dataIsFromKey = false;
  if (fromKey) {
    return;
  }
  if (data.startsWith(BRACKETED_START) && data.endsWith(BRACKETED_END)) {
    callEditor(() =>
      paste(data.slice(BRACKETED_START.length, -BRACKETED_END.length)),
    );
  } else if (data.startsWith("\x1b")) {
    for (const [, code, col, row, press, inOut] of data.matchAll(
      INPUT_REPORT,
    )) {
      callEditor(() =>
        inOut
          ? focus_event(inOut === "I")
          : mouse_event(Number(code), Number(col), Number(row), press === "M"),
      );
    }
  } else {
    callEditor(() => paste(data));
  }
});
terminal.onResize(({ cols, rows }) => callEditor(() => resize(cols, rows)));
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
