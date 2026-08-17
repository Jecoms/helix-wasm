//! Utilities for declaring an async (usually debounced) hook

#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
use futures_executor::block_on;
use tokio::sync::mpsc::{self, error::TrySendError, Sender};
use tokio::time::Instant;

/// Async hooks provide a convenient framework for implementing (debounced)
/// async event handlers. Most synchronous event hooks will likely need to
/// debounce their events, coordinate multiple different hooks and potentially
/// track some state. `AsyncHooks` facilitate these use cases by running as
/// a background tokio task that waits for events (usually an enum) to be
/// sent through a channel.
pub trait AsyncHook: Sync + Send + 'static + Sized {
    type Event: Sync + Send + 'static;
    /// Called immediately whenever an event is received, this function can
    /// consume the event immediately or debounce it. In case of debouncing,
    /// it can either define a new debounce timeout or continue the current one
    fn handle_event(&mut self, event: Self::Event, timeout: Option<Instant>) -> Option<Instant>;

    /// Called whenever the debounce timeline is reached
    fn finish_debounce(&mut self);

    fn spawn(self) -> mpsc::Sender<Self::Event> {
        // the capacity doesn't matter too much here, unless the cpu is totally overwhelmed
        // the cap will never be reached since we always immediately drain the channel
        // so it should only be reached in case of total CPU overload.
        // However, a bounded channel is much more efficient so it's nice to use here
        let (tx, rx) = mpsc::channel(128);
        // only spawn worker if we are inside runtime to avoid having to spawn a runtime for unrelated unit tests
        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::spawn(run(self, rx));
        }
        tx
    }
}

async fn run<Hook: AsyncHook>(mut hook: Hook, mut rx: mpsc::Receiver<Hook::Event>) {
    let mut deadline = None;
    loop {
        let event = match deadline {
            Some(deadline_) => {
                let res = tokio::time::timeout_at(deadline_, rx.recv()).await;
                match res {
                    Ok(event) => event,
                    Err(_) => {
                        hook.finish_debounce();
                        deadline = None;
                        continue;
                    }
                }
            }
            None => rx.recv().await,
        };
        let Some(event) = event else {
            break;
        };
        deadline = hook.handle_event(event, deadline);
    }
}

pub fn send_blocking<T>(tx: &Sender<T>, data: T) {
    // block_on has some overhead and in practice the channel should basically
    // never be full anyway so first try sending without blocking
    if let Err(TrySendError::Full(data)) = tx.try_send(data) {
        send_full(tx, data);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn send_full<T>(tx: &Sender<T>, data: T) {
    // set a timeout so that we just drop a message instead of freezing the editor in the worst case
    let _ = block_on(tx.send_timeout(data, Duration::from_millis(10)));
}

/// wasm32 has neither half of that fallback: `send_timeout` builds a
/// `tokio::time` sleep, and building one calls `Instant::now()`, which traps
/// on wasm32-unknown-unknown — and there is no thread for `block_on` to park
/// even if it didn't. Dropping is where the timeout was headed anyway, so go
/// straight there rather than trapping the whole module on the way.
#[cfg(target_arch = "wasm32")]
fn send_full<T>(_tx: &Sender<T>, data: T) {
    drop(data);
}
