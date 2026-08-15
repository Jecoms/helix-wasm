/* Freestanding assert shim for wasm32-unknown-unknown: assertions are
 * compiled out, as with NDEBUG. */
#ifndef _WASM_SYSROOT_ASSERT_H
#define _WASM_SYSROOT_ASSERT_H

#define assert(ignore) ((void)0)

#endif
