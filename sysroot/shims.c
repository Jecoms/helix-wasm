/* libc shims for wasm32-unknown-unknown, compiled by the web crate's build
 * script and linked with the statically compiled tree-sitter runtime (and,
 * later, grammars).
 *
 * The stdio family only backs tree-sitter's debug logging and dot-graph
 * output, which are never enabled in the wasm build, so those are no-ops.
 * The str* functions and memchr are real implementations here (tree-sitter's
 * query parser relies on them); memcpy/memmove/memset/memcmp come from Rust's
 * compiler-builtins at link time and the allocator from the web crate's
 * c_alloc module. */
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

FILE *stdout = 0;
FILE *stderr = 0;

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

int fclose(FILE *stream) {
  (void)stream;
  return 0;
}

int fflush(FILE *stream) {
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

int close(int fd) {
  (void)fd;
  return 0;
}

_Noreturn void abort(void) {
  __builtin_trap();
}

/* Parse timeouts are never set by helix, so a frozen clock is harmless. */
int clock_gettime(clockid_t clk_id, struct timespec *tp) {
  (void)clk_id;
  if (tp) {
    tp->tv_sec = 0;
    tp->tv_nsec = 0;
  }
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

char *strdup(const char *s) {
  size_t n = strlen(s) + 1;
  char *copy = malloc(n);
  if (copy) {
    memcpy(copy, s, n);
  }
  return copy;
}

char *strndup(const char *s, size_t n) {
  size_t len = 0;
  while (len < n && s[len]) {
    len++;
  }
  char *copy = malloc(len + 1);
  if (copy) {
    memcpy(copy, s, len);
    copy[len] = '\0';
  }
  return copy;
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
