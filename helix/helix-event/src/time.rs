//! One import path for the timer primitives the async hooks and the LSP
//! client are built on. On native targets these are tokio's own, re-exported
//! unchanged. On wasm32-unknown-unknown tokio's time driver cannot run (no
//! runtime thread to park) and `tokio::time::Instant` is `std::time::Instant`
//! underneath, which traps there — so this module supplies lookalikes backed
//! by browser timeouts (`gloo-timers`) and `performance.now()` (`web-time`),
//! the same substitution helix-view's editor timers already make privately
//! (see `wasm_timer` in helix-view's `editor.rs`).

#[cfg(not(target_arch = "wasm32"))]
pub use tokio::time::{error::Elapsed, sleep, timeout, timeout_at, Instant};

pub use std::time::Duration;

#[cfg(target_arch = "wasm32")]
pub use wasm::{sleep, timeout, timeout_at, Elapsed, Instant};

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::fmt;
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use std::time::Duration;

    use gloo_timers::future::TimeoutFuture;
    pub use web_time::Instant;

    /// The deadline elapsed before the future completed — the wasm stand-in
    /// for `tokio::time::error::Elapsed` (which has no public constructor).
    #[derive(Debug, PartialEq, Eq)]
    pub struct Elapsed(());

    impl fmt::Display for Elapsed {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            "deadline has elapsed".fmt(f)
        }
    }

    impl std::error::Error for Elapsed {}

    /// A timer future with the API subset of `tokio::time::Sleep` used here.
    pub struct Sleep {
        timeout: TimeoutFuture,
        deadline: Instant,
    }

    // SAFETY: `TimeoutFuture` holds a JS closure handle, which is `!Send`
    // because a JS value belongs to the thread that made it. On
    // wasm32-unknown-unknown (no `atomics`, no `wasm-bindgen-rayon`) there
    // is exactly one thread, so there is no other thread for the value to
    // reach: every future built on this one is created, polled and dropped
    // on the main thread. The impl exists because helix boxes its LSP
    // request futures as `BoxFuture` (`Send`), and a request's timeout is
    // part of that future — the same trade `send_wrapper` makes.
    unsafe impl Send for Sleep {}

    pub fn sleep(duration: Duration) -> Sleep {
        sleep_until(
            Instant::now()
                .checked_add(duration)
                .unwrap_or_else(far_future),
        )
    }

    fn sleep_until(deadline: Instant) -> Sleep {
        Sleep {
            timeout: timeout_until(deadline),
            deadline,
        }
    }

    // Far enough out to mean "never" (30 years), mirroring what
    // tokio::time::Instant::far_future is documented to guarantee.
    fn far_future() -> Instant {
        Instant::now() + Duration::from_secs(86400 * 365 * 30)
    }

    fn timeout_until(deadline: Instant) -> TimeoutFuture {
        // Browsers cap setTimeout delays at i32::MAX ms (~24.8 days) and fire
        // larger values immediately, so clamp; `Sleep::poll` re-arms until
        // the logical deadline is actually reached.
        let ms = deadline
            .saturating_duration_since(Instant::now())
            .as_millis()
            .min(i32::MAX as u128) as u32;
        TimeoutFuture::new(ms)
    }

    impl Future for Sleep {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            let this = self.get_mut();
            loop {
                match Pin::new(&mut this.timeout).poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(()) => {
                        if Instant::now() >= this.deadline {
                            return Poll::Ready(());
                        }
                        this.timeout = timeout_until(this.deadline);
                    }
                }
            }
        }
    }

    pin_project_lite::pin_project! {
        /// A future racing its inner future against a deadline, with the
        /// shape of `tokio::time::Timeout`.
        pub struct Timeout<F> {
            #[pin]
            future: F,
            delay: Sleep,
        }
    }

    impl<F: Future> Future for Timeout<F> {
        type Output = Result<F::Output, Elapsed>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let this = self.project();
            if let Poll::Ready(value) = this.future.poll(cx) {
                return Poll::Ready(Ok(value));
            }
            match Pin::new(this.delay).poll(cx) {
                Poll::Ready(()) => Poll::Ready(Err(Elapsed(()))),
                Poll::Pending => Poll::Pending,
            }
        }
    }

    pub fn timeout<F: Future>(duration: Duration, future: F) -> Timeout<F> {
        Timeout {
            future,
            delay: sleep(duration),
        }
    }

    pub fn timeout_at<F: Future>(deadline: Instant, future: F) -> Timeout<F> {
        Timeout {
            future,
            delay: sleep_until(deadline),
        }
    }
}
