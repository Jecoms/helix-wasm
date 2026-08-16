//! This module provides platform related functions.
//!
//! Shim: cursor position reporting requires a tty round-trip; the browser
//! bridge does not track the cursor, so the origin is reported.

/// Returns the cursor position (column, row).
///
/// Shim: always `Ok((0, 0))` — never `Err`. Upstream does an `ESC[6n`
/// round-trip that yields real coordinates or errors on timeout; here a
/// future caller silently gets the origin. Currently nothing in helix
/// reaches this (only helix-tui's `get_cursor()` would); if a real caller
/// appears, the position must be tracked browser-side via the bridge.
#[cfg(feature = "events")]
pub fn position() -> std::io::Result<(u16, u16)> {
    Ok((0, 0))
}
