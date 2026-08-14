/* Freestanding stdlib shim for wasm32-unknown-unknown.
 *
 * The allocator is implemented in Rust on top of `std::alloc`
 * (vendor/tree-house-bindings/src/wasm_alloc.rs); `abort` traps
 * (see shims.c). */
#ifndef _WASM_SYSROOT_STDLIB_H
#define _WASM_SYSROOT_STDLIB_H

#include <stddef.h>

void *malloc(size_t size);
void *calloc(size_t count, size_t size);
void *realloc(void *ptr, size_t size);
void free(void *ptr);

_Noreturn void abort(void);

#endif
