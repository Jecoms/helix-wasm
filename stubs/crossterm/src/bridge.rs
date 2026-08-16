//! Host-side integration point for the browser terminal.
//!
//! This module is NOT part of upstream crossterm — it is the seam where the
//! wasm frontend replaces the operating-system terminal. The browser backend
//! (the xterm.js glue in `web/`) pushes input with
//! [`inject_event`] and keeps
//! the dimensions current with [`set_size`]; the vendored crossterm API
//! surface ([`EventStream`](crate::event::EventStream),
//! [`terminal::size`](crate::terminal::size), raw-mode toggles) reads from
//! the state kept here instead of issuing ioctls/syscalls.
//!
//! Single-consumer: one waker slot is kept, so exactly one `EventStream`
//! should be polled at a time (helix-term creates exactly one).
//!
//! Output flows the other way through [`Output`], an `io::Write` whose
//! `flush` hands the buffered ANSI bytes to the sink registered with
//! [`set_output`] — the frontend forwards them to its terminal emulator.

use std::collections::VecDeque;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};

use crate::event::Event;

/// Terminal dimensions as `columns << 16 | rows`, defaulting to 80x24 until
/// the frontend reports a real size.
static SIZE: AtomicU32 = AtomicU32::new((80 << 16) | 24);
static RAW_MODE: AtomicBool = AtomicBool::new(false);
static QUEUE: Mutex<VecDeque<Event>> = Mutex::new(VecDeque::new());
static OUTPUT_SINK: Mutex<Option<fn(&[u8])>> = Mutex::new(None);
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

/// Registers the sink that receives everything written to [`Output`].
///
/// A plain `fn` keeps the bridge free of frontend types; a wasm frontend
/// typically registers a function that reads its JS callback out of a
/// thread-local (wasm32 is single-threaded, so that is sound).
pub fn set_output(sink: fn(&[u8])) {
    *OUTPUT_SINK.lock().unwrap() = Some(sink);
}

/// The browser stand-in for the process stdout that terminal rendering
/// writes to: buffers writes and forwards them to the [`set_output`] sink on
/// flush, preserving the batching the renderer's queue!/flush pattern
/// expects. Writes made before a sink is registered are discarded on flush
/// (there is nowhere to show them, and buffering indefinitely would leak).
#[derive(Debug, Default)]
pub struct Output {
    buffer: Vec<u8>,
}

impl Output {
    pub fn new() -> Self {
        Self::default()
    }
}

impl io::Write for Output {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.buffer.is_empty() {
            // Copy the `fn` out so the OUTPUT_SINK lock is released before
            // calling it: the sink crosses into frontend code (JS on wasm32),
            // and `std::sync::Mutex` is non-reentrant — a callback that
            // re-entered the bridge while the lock was held would deadlock
            // the single wasm thread.
            let sink = *OUTPUT_SINK.lock().unwrap();
            if let Some(sink) = sink {
                sink(&self.buffer);
            }
            self.buffer.clear();
        }
        Ok(())
    }
}
