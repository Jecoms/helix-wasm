//! Read-only inspection of live editor state (issue #18): embedders poll
//! mode, cursor, selections, and buffer text from JS instead of scraping the
//! rendered terminal.
//!
//! Everything reads the focused view's document. Coordinate semantics:
//! `cursor.row`/`cursor.col` are 0-based document coordinates (row = line
//! index, col = **grapheme-cluster** count from the start of the line — a
//! combining sequence or ZWJ emoji counts once, so it is *not* a char
//! offset), and selection `anchor`/`head` are rope **char** indices — not
//! bytes, not UTF-16 code units, not grapheme clusters. `head` is the
//! moving end; `cursor` is derived from the primary range (1-width
//! semantics), so it can sit one left of `head`.
//!
//! Both exports return `undefined` while helix is not running (before
//! `start`, or after the editor exits) and throw when called from inside the
//! editor's own event loop — e.g. from the `output` callback during a
//! render, where the editor is mid-borrow; such callers must defer to a
//! microtask.

use helix_wasm::helix_core::coords_at_pos;
use helix_wasm::helix_view::current_ref;
use js_sys::{Array, Object, Reflect};
use wasm_bindgen::prelude::*;

use crate::session::{with_app, AppBusy};

impl From<AppBusy> for JsError {
    fn from(_: AppBusy) -> Self {
        JsError::new(
            "editor state is unavailable from inside the editor's event loop \
             (e.g. the output callback); defer to a microtask",
        )
    }
}

fn set(obj: &Object, key: &str, value: &JsValue) {
    // Reflect::set only fails on non-objects; `obj` is always an Object here.
    Reflect::set(obj, &JsValue::from_str(key), value).unwrap();
}

/// A snapshot of the focused view's state:
/// `{ mode, path, cursor: { row, col }, selections: [{ anchor, head }] }`.
///
/// `mode` is `"normal"` / `"select"` / `"insert"`; `path` is the document's
/// path, or `undefined` for a scratch buffer. See the module docs for
/// coordinate semantics.
#[wasm_bindgen]
pub fn editor_state() -> Result<JsValue, JsError> {
    let state = with_app(|app| {
        let editor = &app.editor;
        let (view, doc) = current_ref!(editor);
        let text = doc.text().slice(..);
        let selection = doc.selection(view.id);
        let cursor = coords_at_pos(text, selection.primary().cursor(text));

        let cursor_obj = Object::new();
        set(&cursor_obj, "row", &(cursor.row as f64).into());
        set(&cursor_obj, "col", &(cursor.col as f64).into());

        let selections = Array::new();
        for range in selection.ranges() {
            let range_obj = Object::new();
            set(&range_obj, "anchor", &(range.anchor as f64).into());
            set(&range_obj, "head", &(range.head as f64).into());
            selections.push(&range_obj);
        }

        let obj = Object::new();
        set(&obj, "mode", &editor.mode().to_string().into());
        let path = doc
            .path()
            .map_or(JsValue::UNDEFINED, |path| path.to_string_lossy().as_ref().into());
        set(&obj, "path", &path);
        set(&obj, "cursor", &cursor_obj);
        set(&obj, "selections", &selections);
        obj.into()
    })?;
    Ok(state.unwrap_or(JsValue::UNDEFINED))
}

/// The full text of the focused document: the live buffer, unsaved edits
/// included — distinct from `vfs_read`, which sees what was last saved.
///
/// Split out from [`editor_state`] because it copies the whole rope (O(n));
/// state polling stays cheap without it.
#[wasm_bindgen]
pub fn editor_text() -> Result<Option<String>, JsError> {
    Ok(with_app(|app| {
        let (_view, doc) = current_ref!(&app.editor);
        doc.text().to_string()
    })?)
}
