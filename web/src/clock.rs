//! The monotonic clock the statically linked C reads, backed by the host
//! page (issue #77).
//!
//! wasm32-unknown-unknown has no clock, so `sysroot/shims.c` used to freeze
//! `clock_gettime` at zero. tree-sitter measures helix's 500 ms parse budget
//! with that clock, so elapsed time was always zero, the deadline never
//! arrived, and a file too big to parse inside the budget parsed to
//! completion anyway — on the main thread, which means a frozen page
//! instead of a buffer that quietly drops its highlighting. The shim now
//! calls in here, the same split the C
//! allocator already uses: the browser's clock is reachable only through JS,
//! and JS only from the Rust side (see [`crate::c_alloc`]).
//!
//! The reading comes from `web-time`, which the helix crates already use for
//! `Instant` on wasm32, so the C clock and the Rust one run on one
//! `performance.now()` timeline rather than two unrelated ones.

use std::sync::OnceLock;

use web_time::Instant;

/// Nanoseconds elapsed since this function was first called — the value
/// `clock_gettime(CLOCK_MONOTONIC, ...)` hands back, split into a
/// `struct timespec` on the C side.
///
/// Monotonic, and only as fine-grained as the browser lets it be:
/// `performance.now()` is deliberately coarsened (0.1 ms in Chromium, ~1 ms
/// in Firefox by default) as a side-channel defense. That is three orders of
/// magnitude below the 500 ms budget this exists to enforce, and tree-sitter
/// only samples the clock once per 100 parser operations, so neither the
/// resolution nor the cost of crossing into JS matters here.
#[no_mangle]
pub extern "C" fn helix_web_monotonic_nanos() -> u64 {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    let origin = ORIGIN.get_or_init(Instant::now);
    // `performance.now()` is specified never to run backwards; saturate
    // rather than panic in case a host ever disagrees.
    Instant::now().saturating_duration_since(*origin).as_nanos() as u64
}
