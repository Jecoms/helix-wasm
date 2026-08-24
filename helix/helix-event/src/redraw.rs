//! Signals that control when/if the editor redraws

use std::future::Future;
use std::task::Waker;

use parking_lot::{Mutex, RwLock, RwLockReadGuard};
use tokio::sync::Notify;

use crate::runtime_local;

runtime_local! {
    /// A `Notify` instance that can be used to (asynchronously) request
    /// the editor to render a new frame.
    static REDRAW_NOTIFY: Notify = Notify::const_new();

    /// A `RwLock` that prevents the next frame from being
    /// drawn until an exclusive (write) lock can be acquired.
    /// This allows asynchronous tasks to acquire `non-exclusive`
    /// locks (read) to prevent the next frame from being drawn
    /// until a certain computation has finished.
    static RENDER_LOCK: RwLock<()> = RwLock::new(());

    /// The waker of a driver that polls the editor's event loop as a
    /// freshly built future each time (the browser port's `drive`, see
    /// [`register_loop_waker`]). Unset — and so a no-op in
    /// [`request_redraw`] — everywhere the loop is one long-lived future.
    static LOOP_WAKER: Mutex<Option<Waker>> = Mutex::new(None);
}

pub type RenderLockGuard = RwLockReadGuard<'static, ()>;

/// Requests that the editor is redrawn. The redraws are debounced (currently to
/// 30FPS) so this can be called many times without causing a ton of frames to
/// be rendered.
pub fn request_redraw() {
    REDRAW_NOTIFY.notify_one();
    if let Some(waker) = LOOP_WAKER.lock().as_ref() {
        waker.wake_by_ref();
    }
}

/// Registers the waker of the task driving the event loop, for a driver
/// that rebuilds the loop's future on every poll rather than holding one
/// across polls. Dropping that future drops the [`redraw_requested`]
/// waiter inside it, so a later [`request_redraw`] finds no waiter to wake:
/// `notify_one` stores its permit (the next poll's waiter takes it) but
/// wakes nobody, and the loop stays asleep until something else — a
/// keystroke, a channel — polls it. Registering here is what supplies that
/// wake. Call it on every poll, before the loop's future is built; a waker
/// that would wake the same task as the registered one is not replaced.
pub fn register_loop_waker(waker: &Waker) {
    let mut slot = LOOP_WAKER.lock();
    if !slot
        .as_ref()
        .is_some_and(|current| current.will_wake(waker))
    {
        *slot = Some(waker.clone());
    }
}

/// Returns a future that will yield once a redraw has been asynchronously
/// requested using [`request_redraw`].
pub fn redraw_requested() -> impl Future<Output = ()> {
    REDRAW_NOTIFY.notified()
}

/// Wait until all locks acquired with [`lock_frame`] have been released.
/// This function is called before rendering and is intended to allow the frame
/// to wait for async computations that should be included in the current frame.
pub fn start_frame() {
    drop(RENDER_LOCK.write());
    // exhaust any leftover redraw notifications
    let notify = REDRAW_NOTIFY.notified();
    tokio::pin!(notify);
    notify.enable();
}

/// Acquires the render lock which will prevent the next frame from being drawn
/// until the returned guard is dropped.
pub fn lock_frame() -> RenderLockGuard {
    RENDER_LOCK.read()
}

/// A zero sized type that requests a redraw via [request_redraw] when the type [Drop]s.
pub struct RequestRedrawOnDrop;

impl Drop for RequestRedrawOnDrop {
    fn drop(&mut self) {
        request_redraw();
    }
}
