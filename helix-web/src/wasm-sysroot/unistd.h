/* Freestanding unistd shim for wasm32-unknown-unknown. Only referenced by
 * tree-sitter's dot-graph output; `dup` always fails (see shims.c). */
#ifndef _WASM_SYSROOT_UNISTD_H
#define _WASM_SYSROOT_UNISTD_H

int dup(int fd);

#endif
