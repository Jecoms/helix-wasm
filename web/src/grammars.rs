//! The static grammar set: registration glue between the grammars the build
//! script compiled into this binary and `helix_loader`'s wasm32 grammar
//! registry. The grammar list lives in build.rs (`GRAMMARS`), which
//! generates the `register()` function included here; the query files it
//! embeds are vendored under `queries/`.

include!(concat!(env!("OUT_DIR"), "/grammar_registration.rs"));
