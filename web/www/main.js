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
  vfs_delete,
  editor_state,
  editor_text,
  on_download,
  on_remove,
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
// `config.toml` is unreachable from a browser (issue #75), so a page that
// wants a non-default keymap, `[editor]` setting or theme hands the text to
// `start`, which seeds it where helix reads its user config from. Set
// `window.helixConfig` before this module runs — an inline script above the
// bundle, or Playwright's `addInitScript`. Left unset, helix boots on its
// defaults, which is what the demo itself does.
// A config is likely fetched, so the near-miss worth catching is a Promise
// (or an already-parsed object) left where the text belongs: helix would boot
// on its defaults with nothing anywhere to say why. `null` and `undefined`
// are how a page says "no config", so they pass quietly.
const configured = window.helixConfig ?? undefined;
if (configured !== undefined && typeof configured !== "string") {
  console.warn(
    "window.helixConfig must be the text of a config.toml; ignoring",
    configured,
  );
}
const bootConfig = typeof configured === "string" ? configured : undefined;
callEditor(() =>
  start(
    (bytes) => terminal.write(bytes),
    terminal.cols,
    terminal.rows,
    bootConfig,
  ),
);

// For a keystroke xterm.js fires onKey and then, synchronously, onData with
// the sequence that key produced. Pastes (and IME-composed text) arrive
// through onData alone — a length heuristic can't tell a one-character paste
// from a keystroke, so track "the next onData is this keystroke's own
// sequence" explicitly instead.
let dataIsFromKey = false;
// xterm.js's own platform test, mirrored (@xterm/xterm 5.5
// common/Platform.ts). Both Option paths below hang off it — one consumes
// what xterm resolved on its macOS path, the other takes over where xterm's
// macOS path gives up — so the checks have to agree on what "mac" means.
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
// This handles only the chords xterm resolves. The ones it cannot are taken
// over before `onKey` by the custom key handler below.
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

// xterm.js's `KEYCODE_KEY_MAPPINGS` (@xterm/xterm 5.5
// common/input/Keyboard.ts:11-35) re-keyed by `KeyboardEvent.code`: the same
// US layout, the same `[unshifted, shifted]` pairs — looked up by the field
// composition leaves alone. Only the punctuation and digit rows are here;
// the letters are xterm's to resolve (see the handler below).
const CODE_KEY_MAPPINGS = {
  Digit0: ["0", ")"],
  Digit1: ["1", "!"],
  Digit2: ["2", "@"],
  Digit3: ["3", "#"],
  Digit4: ["4", "$"],
  Digit5: ["5", "%"],
  Digit6: ["6", "^"],
  Digit7: ["7", "&"],
  Digit8: ["8", "*"],
  Digit9: ["9", "("],
  Semicolon: [";", ":"],
  Equal: ["=", "+"],
  Comma: [",", "<"],
  Minus: ["-", "_"],
  Period: [".", ">"],
  Slash: ["/", "?"],
  Backquote: ["`", "~"],
  BracketLeft: ["[", "{"],
  Backslash: ["\\", "|"],
  BracketRight: ["]", "}"],
  Quote: ["'", '"'],
};

// The half of macOS composition the fallback above cannot reach (issue #81).
// A dead key is not merely composed, it *starts* a composition, and the
// keydown announcing that carries `key: "Dead"`, `code` naming the physical
// key, and `keyCode: 229` — the composition sentinel, not the key's own
// legacy code. xterm's Alt branch is keyed on `keyCode` throughout, so 229
// misses its US-layout table; the one branch that reads `code` instead only
// fires for `code.startsWith("Key")` (Keyboard.ts:373-385, added for
// xtermjs/xterm.js#3725). That covers the letter accent starters
// (Option-e/i/n/u) and nothing else: for `` Option-` `` (`code:
// "Backquote"`) no branch matches, `result.key` stays undefined, and
// `_keyDown` returns at `if (!result.key)` (Terminal.ts:1046-1048) *before*
// `_onKey.fire` — so `onKey` never runs and no forwarding logic downstream
// of it can recover the chord. `` A-` `` is `switch_to_uppercase`, the chord
// `:tutor` 10.3 asks for.
//
// `attachCustomKeyEventHandler` is the supported hook that runs earlier than
// that early return — it is the first thing `_keyDown` does
// (Terminal.ts:1004-1007) — so the chord is resolved and forwarded here, and
// returning `false` stops xterm from processing the event a second time.
// What keeps it narrow: xterm's own Alt-branch condition, restated below,
// plus two gates of this page's own. macOS only, for the same reason
// `composed()` is — nothing else composes Option, and a `code`-driven
// US-layout guess would turn chords that are inert on a non-US layout into
// live commands. And `code: "Key*"` is left alone: those are the ones xterm
// resolves for itself, and one owner per shape is what keeps a chord from
// being decoded twice by two US-layout tables that could drift apart. Where
// this does step in, returning `false` is what keeps the chord from *also*
// going out through `onKey` — forwarding without it would run the binding
// twice off one keystroke.
terminal.attachCustomKeyEventHandler((event) => {
  // The handler is consulted for keypress and keyup too (Terminal.ts:1102,
  // 1129); a composition only ever announces itself on keydown. The rest
  // restates xterm's own Alt-branch condition — `(!isMac || macOptionIsMeta)
  // && ev.altKey && !ev.metaKey` (Keyboard.ts:349) — because this is that
  // branch's missing case, not a second policy about Alt chords. Reading
  // `macOptionIsMeta` off the terminal rather than assuming it means the two
  // cannot disagree if the option is ever turned off: Option would go back
  // to composing characters, and every Alt chord (this one included) back to
  // being dropped, together.
  if (
    event.type !== "keydown" ||
    !IS_MAC ||
    !terminal.options.macOptionIsMeta ||
    !event.altKey ||
    event.metaKey ||
    event.key !== "Dead" ||
    event.code.startsWith("Key")
  ) {
    return true;
  }
  const chord = CODE_KEY_MAPPINGS[event.code];
  if (!chord) {
    return true;
  }
  // xterm cancels the dead keys it resolves for itself (`result.cancel` in
  // its own `Dead` branch), and this has to as well: without it the
  // keystroke still begins its composition in xterm's helper textarea, and
  // the `compositionend` that eventually lands would arrive as a paste.
  event.preventDefault();
  callEditor(() =>
    key_event(
      chord[event.shiftKey ? 1 : 0],
      event.ctrlKey,
      true,
      event.shiftKey,
      false,
    ),
  );
  return false;
});
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
window.helixVfs = {
  write: vfs_write,
  read: vfs_read,
  list: vfs_list,
  delete: vfs_delete,
};

// Read-only editor state inspection (issue #18): `state()` returns
// { mode, theme, path, cursor: { row, col }, selections: [{ anchor, head }] },
// `text()` the focused buffer's live text (unsaved edits included — unlike
// `helixVfs.read`, which sees what was last saved). Both return `undefined`
// when helix is not running, and throw if called from inside the editor's
// own output callback (defer to a microtask there). The intended assertion
// surface for embedders and browser-automation harnesses.
window.helixState = { state: editor_state, text: editor_text };

// The page's half of `:download` (issue #67): the wasm module hands over a
// file name and its bytes, and saving them to the reader's machine is a
// thing only a page can do. On `window` like the two surfaces above so it
// can be swapped from the devtools console (or by a harness) without
// re-registering with the module — `on_download` is the embedder's seam,
// this is the demo's. Registering it down here, after `start()`, is safe:
// the editor's event loop first polls on a microtask, which cannot run
// until this module body has finished.
//
// The anchor has to be in the document for Firefox to honor the click, and
// the object URL outlives it — revoking in the same task cancels the save in
// some browsers, so it goes out on a later one. Throwing from here refuses
// the download and puts the message on the editor's statusline.
window.helixDownload = (name, bytes) => {
  const url = URL.createObjectURL(
    new Blob([bytes], { type: "application/octet-stream" }),
  );
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  link.style.display = "none";
  document.body.append(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 60_000);
};
on_download((name, bytes) => window.helixDownload(name, bytes));

// The page's half of `:remove` (issue #132): the wasm module says which
// store key is about to go, before it goes. Registering is what enables the
// command — a page that must not offer deletion registers nothing — and
// the handler is where a page that mirrors the store prunes its mirror.
// The demo mirrors nothing, so this only has to exist; it is on `window`
// for the same reason `helixDownload` is, so a harness can swap it for one
// that records the path or throws. Throwing refuses the removal and puts
// the message on the editor's statusline.
window.helixRemove = (path) => {};
on_remove((path) => window.helixRemove(path));
