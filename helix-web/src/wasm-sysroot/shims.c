/* libc shims for wasm32-unknown-unknown, linked with the statically compiled
 * tree-sitter runtime and grammars.
 *
 * The stdio family only backs tree-sitter's debug logging and dot-graph
 * output, which are never enabled in the wasm build, so those are no-ops.
 * The str* functions are real implementations (tree-sitter's query parser
 * relies on them); the mem* functions come from Rust's compiler-builtins at
 * link time and the allocator from vendor/tree-house-bindings/src/wasm_alloc.rs.
 */
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

int fprintf(FILE *stream, const char *format, ...) {
  (void)stream;
  (void)format;
  return 0;
}

int vsnprintf(char *buf, size_t size, const char *format, va_list ap) {
  (void)format;
  (void)ap;
  if (buf && size > 0) {
    buf[0] = '\0';
  }
  return 0;
}

int snprintf(char *buf, size_t size, const char *format, ...) {
  (void)format;
  if (buf && size > 0) {
    buf[0] = '\0';
  }
  return 0;
}

int fputs(const char *s, FILE *stream) {
  (void)s;
  (void)stream;
  return 0;
}

int fputc(int c, FILE *stream) {
  (void)stream;
  return c;
}

size_t fwrite(const void *ptr, size_t size, size_t nmemb, FILE *stream) {
  (void)ptr;
  (void)size;
  (void)stream;
  return nmemb;
}

int fclose(FILE *stream) {
  (void)stream;
  return 0;
}

FILE *fdopen(int fd, const char *mode) {
  (void)fd;
  (void)mode;
  return 0;
}

int dup(int fd) {
  (void)fd;
  return -1;
}

_Noreturn void abort(void) {
  __builtin_trap();
}

/* Parse timeouts are never set by helix, so a frozen clock is harmless. */
clock_t clock(void) {
  return 0;
}

size_t strlen(const char *s) {
  const char *a = s;
  while (*s) {
    s++;
  }
  return s - a;
}

int strcmp(const char *s1, const char *s2) {
  while (*s1 && *s1 == *s2) {
    s1++;
    s2++;
  }
  return (unsigned char)*s1 - (unsigned char)*s2;
}

int strncmp(const char *s1, const char *s2, size_t n) {
  for (; n > 0; n--, s1++, s2++) {
    if (*s1 != *s2 || !*s1) {
      return (unsigned char)*s1 - (unsigned char)*s2;
    }
  }
  return 0;
}

char *strcpy(char *dest, const char *src) {
  char *d = dest;
  while ((*d++ = *src++)) {
  }
  return dest;
}

char *strncpy(char *dest, const char *src, size_t n) {
  size_t i = 0;
  for (; i < n && src[i]; i++) {
    dest[i] = src[i];
  }
  for (; i < n; i++) {
    dest[i] = '\0';
  }
  return dest;
}

char *strchr(const char *s, int c) {
  for (;; s++) {
    if (*s == (char)c) {
      return (char *)s;
    }
    if (!*s) {
      return 0;
    }
  }
}

void *memchr(const void *s, int c, size_t n) {
  const unsigned char *p = s;
  for (; n > 0; n--, p++) {
    if (*p == (unsigned char)c) {
      return (void *)p;
    }
  }
  return 0;
}
