//! One import path for spawning detached background futures. On native
//! targets this is `tokio::spawn` (and tokio's `JoinSet`), unchanged. On
//! wasm32-unknown-unknown there is no tokio runtime, ever — `tokio::spawn`
//! panics with "there is no reactor running", and a wasm32 panic is a trap
//! that takes the whole module with it — so the browser's own executor, the
//! JS microtask queue, stands in via `wasm_bindgen_futures::spawn_local`:
//! the same trade helix-term's `job::spawn_detached` already makes.
//!
//! Everything spawned through here communicates through `tokio::sync`
//! channels, which never touch a reactor, so the futures themselves run the
//! same on either executor.

use std::future::Future;

/// Runs `future` to completion in the background, detached: nothing joins
/// it and nothing waits on it. The output is discarded.
#[cfg(not(target_arch = "wasm32"))]
pub fn spawn<F>(future: F)
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    tokio::spawn(future);
}

/// Runs `future` to completion in the background, detached: nothing joins
/// it and nothing waits on it. The output is discarded.
///
/// The microtask queue appends rather than re-entering, so spawning from
/// inside a task it is already polling (the editor's event loop) is fine.
#[cfg(target_arch = "wasm32")]
pub fn spawn<F>(future: F)
where
    F: Future + 'static,
{
    wasm_bindgen_futures::spawn_local(async move {
        let _ = future.await;
    });
}

#[cfg(not(target_arch = "wasm32"))]
pub use tokio::task::JoinSet;

#[cfg(target_arch = "wasm32")]
pub use wasm::{JoinError, JoinSet};

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::future::Future;

    use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

    /// Uninhabited: a `spawn_local` task cannot be cancelled or panic its
    /// way out (a wasm32 panic traps the module), so `join_next` never
    /// yields an `Err`. Exists so callers can keep the `.unwrap()` they
    /// need against tokio's `JoinSet`.
    #[derive(Debug)]
    pub enum JoinError {}

    /// The `tokio::task::JoinSet` API subset the completion handler uses,
    /// over `spawn_local` tasks funneling their outputs through a channel.
    /// Completion order is the order the tasks finish in, same as tokio's.
    pub struct JoinSet<T> {
        tx: UnboundedSender<T>,
        rx: UnboundedReceiver<T>,
        pending: usize,
    }

    impl<T: 'static> JoinSet<T> {
        pub fn new() -> Self {
            let (tx, rx) = unbounded_channel();
            Self { tx, rx, pending: 0 }
        }

        pub fn spawn<F>(&mut self, task: F)
        where
            F: Future<Output = T> + 'static,
        {
            self.pending += 1;
            let tx = self.tx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let _ = tx.send(task.await);
            });
        }

        /// There is no blocking pool (or any second thread) in the browser;
        /// the closure runs as an immediately-ready task on the microtask
        /// queue instead. Anything genuinely blocking inside it blocks the
        /// main thread — which is where it would have run anyway here.
        pub fn spawn_blocking<F>(&mut self, task: F)
        where
            F: FnOnce() -> T + 'static,
        {
            self.spawn(async move { task() });
        }

        pub async fn join_next(&mut self) -> Option<Result<T, JoinError>> {
            if self.pending == 0 {
                return None;
            }
            // Never `None`: `self.tx` keeps the channel open for as long as
            // this set exists.
            let value = self.rx.recv().await?;
            self.pending -= 1;
            Some(Ok(value))
        }

        pub fn is_empty(&self) -> bool {
            self.pending == 0
        }
    }

    impl<T: 'static> Default for JoinSet<T> {
        fn default() -> Self {
            Self::new()
        }
    }
}
