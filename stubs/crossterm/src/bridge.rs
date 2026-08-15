//! Host-side integration point for the browser terminal.
//!
//! This module is NOT part of upstream crossterm — it is the seam where the
//! wasm frontend replaces the operating-system terminal. The browser backend
//! (xterm.js glue, Phase 3 — see SPIKE-NOTES.md) pushes input with
//! [`inject_event`] and keeps
//! the dimensions current with [`set_size`]; the vendored crossterm API
//! surface ([`EventStream`](crate::event::EventStream),
//! [`terminal::size`](crate::terminal::size), raw-mode toggles) reads from
//! the state kept here instead of issuing ioctls/syscalls.
//!
//! Single-consumer: one waker slot is kept, so exactly one `EventStream`
//! should be polled at a time (helix-term creates exactly one).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};

use crate::event::Event;

/// Terminal dimensions as `columns << 16 | rows`, defaulting to 80x24 until
/// the frontend reports a real size.
static SIZE: AtomicU32 = AtomicU32::new((80 << 16) | 24);
static RAW_MODE: AtomicBool = AtomicBool::new(false);
static QUEUE: Mutex<VecDeque<Event>> = Mutex::new(VecDeque::new());
static WAKER: Mutex<Option<Waker>> = Mutex::new(None);

/// Records the terminal dimensions reported by the frontend.
///
/// This only updates what [`crate::terminal::size`] returns; the frontend
/// should additionally inject an [`Event::Resize`] so the application
/// re-renders.
pub fn set_size(columns: u16, rows: u16) {
    SIZE.store(((columns as u32) << 16) | rows as u32, Ordering::Relaxed);
}

/// Queues an input event and wakes the [`EventStream`](crate::event::EventStream).
pub fn inject_event(event: Event) {
    QUEUE.lock().unwrap().push_back(event);
    if let Some(waker) = WAKER.lock().unwrap().take() {
        waker.wake();
    }
}

pub(crate) fn size() -> (u16, u16) {
    let packed = SIZE.load(Ordering::Relaxed);
    ((packed >> 16) as u16, packed as u16)
}

pub(crate) fn set_raw_mode(enabled: bool) {
    RAW_MODE.store(enabled, Ordering::Relaxed);
}

pub(crate) fn is_raw_mode() -> bool {
    RAW_MODE.load(Ordering::Relaxed)
}

pub(crate) fn poll_next_event(cx: &mut Context<'_>) -> Poll<Option<std::io::Result<Event>>> {
    let mut queue = QUEUE.lock().unwrap();
    match queue.pop_front() {
        Some(event) => Poll::Ready(Some(Ok(event))),
        None => {
            *WAKER.lock().unwrap() = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}
