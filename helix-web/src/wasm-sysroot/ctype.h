/* Freestanding ctype shim for wasm32-unknown-unknown (ASCII only). */
#ifndef _WASM_SYSROOT_CTYPE_H
#define _WASM_SYSROOT_CTYPE_H

static inline int isprint(int c) {
  return c >= 0x20 && c < 0x7f;
}

static inline int isdigit(int c) {
  return c >= '0' && c <= '9';
}

static inline int isalpha(int c) {
  return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z');
}

static inline int isalnum(int c) {
  return isalpha(c) || isdigit(c);
}

static inline int isspace(int c) {
  return c == ' ' || (c >= '\t' && c <= '\r');
}

#endif
