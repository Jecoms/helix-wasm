/* Freestanding stdio shim for wasm32-unknown-unknown.
 *
 * The tree-sitter runtime only writes to streams from its debug logging and
 * dot-graph output, which are never enabled in the wasm build, so every
 * stream and sink here is a no-op (implementations in shims.c). */
#ifndef _WASM_SYSROOT_STDIO_H
#define _WASM_SYSROOT_STDIO_H

#include <stdarg.h>
#include <stddef.h>

typedef struct _WasmShimFile FILE;

#define stdout ((FILE *)1)
#define stderr ((FILE *)2)

int fprintf(FILE *stream, const char *format, ...);
int snprintf(char *buf, size_t size, const char *format, ...);
int vsnprintf(char *buf, size_t size, const char *format, va_list ap);
int fputs(const char *s, FILE *stream);
size_t fwrite(const void *ptr, size_t size, size_t nmemb, FILE *stream);
int fputc(int c, FILE *stream);
int fclose(FILE *stream);
FILE *fdopen(int fd, const char *mode);

#endif
