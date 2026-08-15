//! Provides interface for controlling the terminal

use std::io;
use std::io::Write;

use crate::{buffer::Cell, terminal::Config};

use helix_view::graphics::{CursorKind, Rect};

#[cfg(all(feature = "crossterm", not(target_arch = "wasm32")))]
mod crossterm;
#[cfg(all(feature = "crossterm", not(target_arch = "wasm32")))]
pub use self::crossterm::CrosstermBackend;

mod test;
pub use self::test::TestBackend;

/// The writer a backend renders into.
///
/// On native targets any [`Write`] implementor qualifies. On wasm32 the writer
/// must also report the terminal geometry and cursor position, since there is
/// no OS terminal to query for them.
#[cfg(not(target_arch = "wasm32"))]
pub trait Buffer: Write {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Write> Buffer for T {}

#[cfg(target_arch = "wasm32")]
pub trait Buffer: Write {
    fn size(&self) -> std::io::Result<Rect>;
    fn cursor_x(&self) -> u16;
    fn cursor_y(&self) -> u16;
}

/// Representation of a terminal backend.
pub trait Backend {
    /// Claims the terminal for TUI use.
    fn claim(&mut self, config: Config) -> Result<(), io::Error>;
    /// Update terminal configuration.
    fn reconfigure(&mut self, config: Config) -> Result<(), io::Error>;
    /// Restores the terminal to a normal state, undoes `claim`
    fn restore(&mut self, config: Config) -> Result<(), io::Error>;
    /// Forcibly resets the terminal, ignoring errors and configuration
    fn force_restore() -> Result<(), io::Error>;
    /// Draws styled text to the terminal
    fn draw<'a, I>(&mut self, content: I) -> Result<(), io::Error>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>;
    /// Hides the cursor
    fn hide_cursor(&mut self) -> Result<(), io::Error>;
    /// Sets the cursor to the given shape
    fn show_cursor(&mut self, kind: CursorKind) -> Result<(), io::Error>;
    /// Gets the current position of the cursor
    fn get_cursor(&mut self) -> Result<(u16, u16), io::Error>;
    /// Sets the cursor to the given position
    fn set_cursor(&mut self, x: u16, y: u16) -> Result<(), io::Error>;
    /// Clears the terminal
    fn clear(&mut self) -> Result<(), io::Error>;
    /// Gets the size of the terminal in cells
    fn size(&self) -> Result<Rect, io::Error>;
    /// Flushes the terminal buffer
    fn flush(&mut self) -> Result<(), io::Error>;
}
