# License notes

The port keeps upstream's MPL-2.0, and the text is at the root in
[`LICENSE`](../LICENSE) — the unmodified license, byte-identical to the copy
helix ships as `helix/LICENSE`. It covers helix's files here (modified
MPL-2.0 source, which stays MPL-2.0) and this port's own code alike.

It is not a claim over everything in the tree, though: MPL-2.0 is file-level
copyleft, so each vendored dependency under `stubs/` keeps the license it
arrived with, in its own license file beside the code — `crossterm` MIT, `url`
MIT OR Apache-2.0, `nucleo` and `tree-house-bindings` MPL-2.0, the last of
those also carrying MIT tree-sitter C and Unicode-licensed ICU headers.
Copyright stays with the respective authors: helix's files with the helix
contributors, this port's with its own. The crates.io dependency tree the wasm
links — mostly MIT or Apache-2.0, and far larger than this repository's own
code — carries its own terms too. The Rust crates, the grammars, the
tree-sitter runtime and the helix runtime files the wasm bundle ships all have
their notices in `web/NOTICE.md`, whose crate table is generated from the
dependency graph by `web/notice-crates.py` and re-checked by the `wasm32 check`
workflow. That file travels with what is distributed, and this one goes
with it: into the release tarball as `LICENSE`, and onto the deployed demo as
`LICENSE.txt` beside `NOTICE.txt`. The notice opens by naming this license and
the repository the corresponding source form lives in.
