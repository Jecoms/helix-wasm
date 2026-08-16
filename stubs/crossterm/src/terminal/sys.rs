//! This module provides platform related functions.
//!
//! Shim: the OS implementations (ioctl/termios on unix, the console API on
//! windows) are replaced by the state kept in [`crate::bridge`].

use std::io;

use super::WindowSize;

pub(crate) fn is_raw_mode_enabled() -> bool {
    crate::bridge::is_raw_mode()
}

pub(crate) fn enable_raw_mode() -> io::Result<()> {
    crate::bridge::set_raw_mode(true);
    Ok(())
}

pub(crate) fn disable_raw_mode() -> io::Result<()> {
    crate::bridge::set_raw_mode(false);
    Ok(())
}

pub(crate) fn size() -> io::Result<(u16, u16)> {
    Ok(crate::bridge::size())
}

pub(crate) fn window_size() -> io::Result<WindowSize> {
    let (columns, rows) = crate::bridge::size();
    Ok(WindowSize {
        rows,
        columns,
        width: 0,
        height: 0,
    })
}

/// The keyboard enhancement protocol requires querying the terminal over the
/// tty, which the browser bridge does not implement.
#[cfg(feature = "events")]
pub fn supports_keyboard_enhancement() -> io::Result<bool> {
    Ok(false)
}
