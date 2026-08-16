//! SGR mouse report → crossterm mouse event conversion.
//!
//! helix enables mouse capture at boot, so the terminal emulator reports
//! mouse activity as SGR (1006) sequences `ESC [ < code ; col ; row M|m`.
//! The host page parses out the three numeric fields plus the final byte
//! and this module decodes them onto the crossterm event types helix-term
//! consumes, mirroring upstream crossterm's `parse_csi_sgr_mouse`.

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

/// Converts one SGR mouse report. `code` is the button/modifier field,
/// `column`/`row` are the report's 1-based coordinates, and `pressed` is
/// whether the final byte was `M` (press) rather than `m` (release).
/// Returns `None` for codes that name no event (values no terminal emits).
// The only non-test caller (`session`) is wasm-gated; on native this module
// exists purely so its unit tests run under plain `cargo test`.
#[cfg_attr(not(target_family = "wasm"), allow(dead_code))]
pub fn convert(code: u16, column: u16, row: u16, pressed: bool) -> Option<MouseEvent> {
    let mut modifiers = KeyModifiers::NONE;
    if code & 4 != 0 {
        modifiers.insert(KeyModifiers::SHIFT);
    }
    if code & 8 != 0 {
        modifiers.insert(KeyModifiers::ALT);
    }
    if code & 16 != 0 {
        modifiers.insert(KeyModifiers::CONTROL);
    }

    // Button number: the low two bits, extended by bits 6-7 (the +64/+128
    // ranges: scroll wheel, and the 8-11 buttons this doesn't map).
    let button = (code & 0b0000_0011) | ((code & 0b1100_0000) >> 4);
    let dragging = code & 0b0010_0000 != 0;
    let kind = match (button, dragging) {
        (0, false) => MouseEventKind::Down(MouseButton::Left),
        (1, false) => MouseEventKind::Down(MouseButton::Middle),
        (2, false) => MouseEventKind::Down(MouseButton::Right),
        (0, true) => MouseEventKind::Drag(MouseButton::Left),
        (1, true) => MouseEventKind::Drag(MouseButton::Middle),
        (2, true) => MouseEventKind::Drag(MouseButton::Right),
        // Button 3 is "no button". Without motion that's a release — the
        // legacy encoding's only release form; upstream also accepts it in
        // SGR and calls the unknown button Left. Conforming SGR terminals
        // report releases as the pressed code with a final `m` instead.
        (3, false) => MouseEventKind::Up(MouseButton::Left),
        // With motion: the mouse moved with no button held.
        (3, true) | (4, true) | (5, true) => MouseEventKind::Moved,
        (4, false) => MouseEventKind::ScrollUp,
        (5, false) => MouseEventKind::ScrollDown,
        (6, false) => MouseEventKind::ScrollLeft,
        (7, false) => MouseEventKind::ScrollRight,
        _ => return None,
    };
    // SGR reports the release of a button as the same code with a final `m`.
    let kind = match kind {
        MouseEventKind::Down(button) if !pressed => MouseEventKind::Up(button),
        kind => kind,
    };

    Some(MouseEvent {
        kind,
        // SGR coordinates are 1-based; crossterm's are 0-based.
        column: column.saturating_sub(1),
        row: row.saturating_sub(1),
        modifiers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(code: u16, pressed: bool) -> Option<MouseEventKind> {
        convert(code, 1, 1, pressed).map(|event| event.kind)
    }

    #[test]
    fn buttons_press_and_release() {
        assert_eq!(kind(0, true), Some(MouseEventKind::Down(MouseButton::Left)));
        assert_eq!(kind(0, false), Some(MouseEventKind::Up(MouseButton::Left)));
        assert_eq!(
            kind(1, true),
            Some(MouseEventKind::Down(MouseButton::Middle))
        );
        assert_eq!(
            kind(2, false),
            Some(MouseEventKind::Up(MouseButton::Right))
        );
        // Legacy-style release: button 3 without motion, whatever the
        // final byte; the button is unknown, so it maps to Left.
        assert_eq!(kind(3, true), Some(MouseEventKind::Up(MouseButton::Left)));
        assert_eq!(kind(3, false), Some(MouseEventKind::Up(MouseButton::Left)));
    }

    #[test]
    fn drag_and_motion() {
        assert_eq!(
            kind(32, true),
            Some(MouseEventKind::Drag(MouseButton::Left))
        );
        assert_eq!(
            kind(34, true),
            Some(MouseEventKind::Drag(MouseButton::Right))
        );
        // Motion with no button held (32 + 3).
        assert_eq!(kind(35, true), Some(MouseEventKind::Moved));
    }

    #[test]
    fn scroll() {
        assert_eq!(kind(64, true), Some(MouseEventKind::ScrollUp));
        assert_eq!(kind(65, true), Some(MouseEventKind::ScrollDown));
        assert_eq!(kind(66, true), Some(MouseEventKind::ScrollLeft));
        assert_eq!(kind(67, true), Some(MouseEventKind::ScrollRight));
    }

    #[test]
    fn modifiers() {
        // ctrl+shift+left-click: 0 + 4 + 16.
        let event = convert(20, 1, 1, true).unwrap();
        assert_eq!(event.kind, MouseEventKind::Down(MouseButton::Left));
        assert_eq!(event.modifiers, KeyModifiers::SHIFT | KeyModifiers::CONTROL);
        // alt+scroll-down: 65 + 8.
        let event = convert(73, 1, 1, true).unwrap();
        assert_eq!(event.kind, MouseEventKind::ScrollDown);
        assert_eq!(event.modifiers, KeyModifiers::ALT);
    }

    #[test]
    fn coordinates_become_zero_based() {
        let event = convert(0, 3, 7, true).unwrap();
        assert_eq!((event.column, event.row), (2, 6));
        // A (non-conforming) zero coordinate must not wrap.
        let event = convert(0, 0, 0, true).unwrap();
        assert_eq!((event.column, event.row), (0, 0));
    }

    #[test]
    fn unmapped_codes() {
        // Buttons 8-11 (128 + 0..=3) have no crossterm representation.
        assert_eq!(kind(128, true), None);
        assert_eq!(kind(131, true), None);
    }
}
