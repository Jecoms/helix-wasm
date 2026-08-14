/* Freestanding time shim for wasm32-unknown-unknown.
 *
 * `clock` always returns 0 (see shims.c). tree-sitter only uses it for parse
 * timeouts, which helix does not set, so a frozen clock is harmless.
 * CLOCK_MONOTONIC is deliberately not defined so that tree-sitter's clock.h
 * picks its plain `clock()` fallback. */
#ifndef _WASM_SYSROOT_TIME_H
#define _WASM_SYSROOT_TIME_H

typedef long long clock_t;

#define CLOCKS_PER_SEC 1000000

clock_t clock(void);

#endif
