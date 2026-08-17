# The bundled tutor text

No runtime files live in this directory. `session.rs` embeds helix's own
tutorial text with `include_str!`, straight out of the in-tree port
(`../../helix/runtime/tutor`), and seeds it into the virtual file system at
boot, at the path `helix_loader::runtime_file("tutor")` resolves to on
wasm32, so the built-in `:tutor` command finds it.

The text ships verbatim: it is upstream's file, read where it lies, so
nothing here can annotate it for the browser. What that costs is catalogued
below.

The tutor text is helix's own (MPL-2.0, the same license as this repo), so
it needs no entry in `../NOTICE.md`'s license table — that table covers the
differently licensed grammar C sources. `../NOTICE.md` names it alongside
the other helix runtime content the bundle embeds (`queries/`, `themes/`)
via a pointer paragraph.

## Known gaps in the browser

The tutorial was written for helix in a terminal, and it ships unannotated
(see above), so these gaps are documented rather than patched. A walk of all 60
sections ([#65](https://github.com/Jecoms/helix-wasm/issues/65)) found the
following steps that do not work as written. Everything else — every
motion, selection, register, macro, match-mode and multi-cursor exercise in
chapters 2 through 12 — behaves as the text says.

- **1.2 / 1.5, chapter 1 recap — `:q`, `:q!`, `:wq`.** They quit for real,
  and a browser tab has no shell to come back to: `start()` runs once per
  page load, so the editor cannot be restarted in place. Quitting restores
  the main screen and prints
  `Helix has exited. Refresh the page to start a new session. (exit code N)`,
  then calls the `on_exit` handler an embedding page registered (the demo
  page mirrors it as `window.helixExit` and a `helix-exit` DOM event).
  Refresh to carry on with the tutorial.
- **1.5 — "open a new terminal … run `hx FILENAME`".** There is no CLI and
  no second instance per page. `:w /notes.txt` is the working equivalent:
  it saves into the in-memory file system, and `:o /notes.txt` reopens it.
- **13.1-13.3, 13.5-13.7, chapter 13 recap — the `Ctrl-w` window menu.**
  Chrome and Firefox keep Ctrl-w for closing the tab on Windows and Linux,
  and a page cannot take it back (it does reach the editor on macOS, where
  the browsers use Cmd-w instead). Upstream binds the same menu under
  `space w`, so read every `Ctrl-w` in chapter 13 as `space w`:
  `space w n v` splits, `space w hjkl` moves, `space w q` closes,
  `space w o` closes the others, `space w HJKL` swaps, `space w t`
  transposes. 13.4's `:vs` / `:hs` need no chord at all. The chord itself
  stays out of reach whatever anyone configures, but the menu no longer has
  to be read under upstream's alias: `config.toml` reaches the browser as of
  [#75](https://github.com/Jecoms/helix-wasm/issues/75), so a host page that
  wants chapter 13's prefix somewhere else can bind it in `[keys.normal]`.
- **4.2 note, 4.3 recap — `space y` / `space p` "on the system's
  clipboard".** The `+` and `*` registers are editor-local here: the wasm
  clipboard provider silently drops writes and refuses reads, so the
  registers fall back to their in-editor copies. Yanking and pasting inside
  the editor works; nothing crosses into the OS clipboard. Browser-native
  copy/paste (Ctrl/Cmd-V into the terminal) still works — it arrives as a
  bracketed paste.

The rest of chapter 13 works once the prefix is swapped: 13.7's file picker
opens on the sample files seeded at boot (`../src/samples.rs`), and its
`Ctrl-v` / `Ctrl-s` split shortcuts do reach the editor — of the chords the
tutorial teaches, only `Ctrl-w` is one the browser refuses to hand over.

The #65 walk found one more gap that has since been closed, and it was the
largest: every `Alt-` chord the tutorial teaches (3.8's `Alt-;`, 4.2's
`Alt-d` / `Alt-c`, 5.1's `Alt-C`, 5.5's `Alt-s`, 6.3's `Alt-.`, 10.1-10.3's
`Alt-,`, `Alt-)`, `Alt-(` and `` Alt-` ``) did nothing at all on macOS,
because xterm.js treats Option as a compose key unless the terminal claims
it as Meta. The host page now claims it and resolves what macOS composed
anyway, so those steps run as written
([#68](https://github.com/Jecoms/helix-wasm/issues/68),
[#81](https://github.com/Jecoms/helix-wasm/issues/81)). The trade is that
Option no longer types composed characters (`é`, `ß`, `…`) in insert mode
on macOS — nothing the tutorial asks for.

Unlike everything else on this page, that is not a claim from a hand walk,
and it is not chord-by-chord either. Browser automation drives the renderer
directly and never invokes the OS input method, so it cannot compose a real
Option keystroke; `../www/tests/keys.spec.js` stands in with synthetic
events, and it covers the three *shapes* macOS delivers rather than the nine
chords above — an Option-composed character (`Alt-s`), a letter dead key
(`Alt-u`) and a punctuation dead key (`` Alt-` ``). Every chord on that list
arrives as one of the three, which is why the list follows, but only the
shapes are checked.
