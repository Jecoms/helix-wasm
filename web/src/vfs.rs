//! JS hooks into the in-memory virtual file system document IO goes through
//! on wasm32 (`helix_stdx::vfs`): the host page injects files for the editor
//! to `:o`pen and extracts what `:w` saved. Unstable, internal to the host
//! page (see crate docs); live (possibly unsaved) buffer state is the
//! [`crate::inspect`] surface instead.
//!
//! Paths are normalized against the editor's current working directory
//! (initially `/`, but `:cd` moves it), so at startup `"scratch.txt"` and
//! `"/scratch.txt"` name the same file.

use helix_wasm::helix_stdx::vfs;
use wasm_bindgen::prelude::*;

/// Creates or replaces the file at `path`. Throws if `path` names no file
/// (`""`, `"."`, `"/"`, ... — such a key would crash path-shaped UI like
/// the file picker's path column).
#[wasm_bindgen]
pub fn vfs_write(path: &str, contents: &str) -> Result<(), JsError> {
    vfs::write(path, contents.as_bytes()).map_err(|err| JsError::new(&err.to_string()))
}

/// The contents of the file at `path`, or `undefined` if it does not exist.
/// Contents are decoded as UTF-8 (lossily — the editor can save in other
/// encodings via `:encoding`).
#[wasm_bindgen]
pub fn vfs_read(path: &str) -> Option<String> {
    vfs::read(path)
        .ok()
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

/// All file paths in the virtual file system, sorted.
#[wasm_bindgen]
pub fn vfs_list() -> Vec<String> {
    vfs::list()
        .into_iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}
