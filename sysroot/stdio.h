#pragma once
#include <stddef.h>
#include <stdarg.h>
typedef struct FILE FILE;
int fprintf(FILE *stream, const char *format, ...);
int snprintf(char *str, size_t size, const char *format, ...);
int vsnprintf(char *str, size_t size, const char *format, va_list ap);
int fputs(const char *s, FILE *stream);
int fputc(int c, FILE *stream);
int fclose(FILE *stream);
FILE *fdopen(int fd, const char *mode);
int fflush(FILE *stream);
extern FILE *stderr;
extern FILE *stdout;
