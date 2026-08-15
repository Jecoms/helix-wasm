//! This module provides platform related functions.
//!
//! Shim: cursor position reporting requires a tty round-trip; the browser
//! bridge does not track the cursor, so the origin is reported.

/// Returns the cursor position (column, row).
#[cfg(feature = "events")]
pub fn position() -> std::io::Result<(u16, u16)> {
    Ok((0, 0))
}
