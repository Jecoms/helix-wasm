//! DOM `KeyboardEvent` → crossterm key event conversion.
//!
//! The native input path parses ANSI from the tty; in the browser the DOM
//! already delivers decoded keys, so they are mapped straight onto the
//! crossterm event types helix-term consumes.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

/// Converts a `KeyboardEvent.key`-shaped name and its modifier flags. The
/// name is whatever the host page decided the keystroke was — normally the
/// DOM's own `key`, but a host may resolve it itself where the DOM's name is
/// unusable (see `key_event` in `session.rs`). Returns `None` for events
/// that don't map to a terminal key press (lone modifiers, dead keys, ...).
pub fn convert(key: &str, ctrl: bool, alt: bool, shift: bool, meta: bool) -> Option<KeyEvent> {
    let mut modifiers = KeyModifiers::NONE;
    if shift {
        modifiers.insert(KeyModifiers::SHIFT);
    }
    if ctrl {
        modifiers.insert(KeyModifiers::CONTROL);
    }
    if alt {
        modifiers.insert(KeyModifiers::ALT);
    }
    if meta {
        modifiers.insert(KeyModifiers::SUPER);
    }

    let code = match key {
        "Enter" => KeyCode::Enter,
        "Backspace" => KeyCode::Backspace,
        // Native terminals report shift-tab as its own key.
        "Tab" if shift => KeyCode::BackTab,
        "Tab" => KeyCode::Tab,
        "Escape" => KeyCode::Esc,
        "ArrowUp" => KeyCode::Up,
        "ArrowDown" => KeyCode::Down,
        "ArrowLeft" => KeyCode::Left,
        "ArrowRight" => KeyCode::Right,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        "PageUp" => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        "Delete" => KeyCode::Delete,
        "Insert" => KeyCode::Insert,
        _ => {
            let mut chars = key.chars();
            match (chars.next(), chars.next()) {
                // A single-character `key` is the produced character itself
                // (already uppercased/shifted by the browser).
                (Some(ch), None) => KeyCode::Char(ch),
                // Multi-character names that aren't handled above: accept
                // function keys, drop the rest ("Shift", "Dead", ...).
                _ => KeyCode::F(key.strip_prefix('F')?.parse::<u8>().ok()?),
            }
        }
    };

    Some(KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    })
}
