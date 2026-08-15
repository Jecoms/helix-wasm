//! Feeds xterm.js input into the helix event loop.
//!
//! On wasm32 `helix_term::application::InputEvent` is
//! [`helix_view::input::Event`] rather than a crossterm event, so DOM
//! keyboard events are converted directly — there is no ANSI input parsing.

use std::io;

use futures_channel::mpsc::{unbounded, UnboundedReceiver};
use helix_view::input::{Event, KeyEvent};
use helix_view::keyboard::{KeyCode, KeyModifiers};
use rs_xterm_js::{OnKeyEvent, ResizeEventData, Terminal};
use wasm_bindgen::prelude::*;
use web_sys::KeyboardEvent;

pub type EventStream = UnboundedReceiver<io::Result<Event>>;

/// Subscribes to the terminal's key and resize events, yielding them as helix
/// input events. The JS callbacks are leaked (`Closure::forget`); they live
/// for the lifetime of the page, like the terminal itself.
pub fn event_stream(terminal: &Terminal) -> EventStream {
    let (tx, rx) = unbounded();

    let key_tx = tx.clone();
    let on_key = Closure::<dyn FnMut(OnKeyEvent)>::new(move |event: OnKeyEvent| {
        if let Some(key) = convert_key_event(&event.dom_event()) {
            let _ = key_tx.unbounded_send(Ok(Event::Key(key)));
        }
    });
    terminal.on_key(on_key.as_ref().unchecked_ref());
    on_key.forget();

    let resize_tx = tx;
    let on_resize = Closure::<dyn FnMut(ResizeEventData)>::new(move |data: ResizeEventData| {
        let _ = resize_tx.unbounded_send(Ok(Event::Resize(data.cols(), data.rows())));
    });
    terminal.on_resize(&on_resize);
    on_resize.forget();

    rx
}

fn convert_key_event(event: &KeyboardEvent) -> Option<KeyEvent> {
    let mut modifiers = KeyModifiers::NONE;
    if event.shift_key() {
        modifiers.insert(KeyModifiers::SHIFT);
    }
    if event.ctrl_key() {
        modifiers.insert(KeyModifiers::CONTROL);
    }
    if event.alt_key() {
        modifiers.insert(KeyModifiers::ALT);
    }
    if event.meta_key() {
        modifiers.insert(KeyModifiers::SUPER);
    }

    let key = event.key();
    let code = match key.as_str() {
        "Enter" => KeyCode::Enter,
        "Backspace" => KeyCode::Backspace,
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
                _ => {
                    let function = key.strip_prefix('F')?.parse::<u8>().ok()?;
                    KeyCode::F(function)
                }
            }
        }
    };

    Some(KeyEvent { code, modifiers })
}
