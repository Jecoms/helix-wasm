//! Shim: upstream's `EventStream` pumps the OS event source (mio/winapi) on
//! a helper thread. This one is fed by [`crate::bridge::inject_event`].

use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use futures_core::stream::Stream;

use crate::event::Event;

/// A stream of `Result<Event>` fed by [`crate::bridge::inject_event`].
///
/// It implements the [Stream](futures_core::stream::Stream) trait and never
/// ends. Single-consumer: only one instance should be polled at a time (see
/// [`crate::bridge`]).
#[derive(Debug, Default)]
pub struct EventStream {
    _private: (),
}

impl EventStream {
    pub fn new() -> EventStream {
        EventStream::default()
    }
}

impl Stream for EventStream {
    type Item = io::Result<Event>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        crate::bridge::poll_next_event(cx)
    }
}
