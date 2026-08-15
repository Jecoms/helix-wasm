//! C allocator entry points for wasm32-unknown-unknown, backed by Rust's
//! global allocator.
//!
//! The vendored tree-sitter runtime (and grammar scanners) call `malloc`,
//! `calloc`, `realloc` and `free`. There is no libc on
//! wasm32-unknown-unknown, so these definitions are the only ones in the
//! final link. Rust's allocator needs the `Layout` back at deallocation
//! time, which C callers don't provide, so each allocation stores its size
//! in a header directly before the pointer handed to C.

use std::alloc::{self, Layout};
use std::ptr;

/// Large enough for the header (a `usize`) and at least as strict as C's
/// `max_align_t` on wasm32 (16 bytes).
const ALIGN: usize = 16;

#[inline]
fn layout(size: usize) -> Option<Layout> {
    let total = size.checked_add(ALIGN)?;
    // `ALIGN` is a non-zero power of two and `total` can't overflow `isize`
    // on wasm32 without the allocation itself failing first.
    Layout::from_size_align(total, ALIGN).ok()
}

/// Writes the size header and returns the pointer to hand out to C.
///
/// # Safety
/// `base` must point to an allocation of `layout(size)`.
unsafe fn finish(base: *mut u8, size: usize) -> *mut u8 {
    if base.is_null() {
        return ptr::null_mut();
    }
    (base as *mut usize).write(size);
    base.add(ALIGN)
}

#[no_mangle]
pub unsafe extern "C" fn malloc(size: usize) -> *mut u8 {
    match layout(size) {
        Some(layout) => finish(alloc::alloc(layout), size),
        None => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn calloc(count: usize, size: usize) -> *mut u8 {
    let Some(bytes) = count.checked_mul(size) else {
        return ptr::null_mut();
    };
    match layout(bytes) {
        Some(layout) => finish(alloc::alloc_zeroed(layout), bytes),
        None => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn realloc(ptr: *mut u8, new_size: usize) -> *mut u8 {
    if ptr.is_null() {
        return malloc(new_size);
    }
    let base = ptr.sub(ALIGN);
    let old_size = (base as *const usize).read();
    let old_layout = layout(old_size).unwrap();
    let Some(new_total) = new_size.checked_add(ALIGN) else {
        return ptr::null_mut();
    };
    finish(alloc::realloc(base, old_layout, new_total), new_size)
}

#[no_mangle]
pub unsafe extern "C" fn free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    let base = ptr.sub(ALIGN);
    let size = (base as *const usize).read();
    alloc::dealloc(base, layout(size).unwrap());
}
