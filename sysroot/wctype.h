/* Freestanding wctype shim for wasm32-unknown-unknown. Classification tables
 * and implementations (borrowed from musl) live in wctype.c. */
#ifndef _WASM_SYSROOT_WCTYPE_H
#define _WASM_SYSROOT_WCTYPE_H

#include <stddef.h>

typedef unsigned int wint_t;

int iswalpha(wint_t wc);
int iswdigit(wint_t wc);
int iswalnum(wint_t wc);
int iswspace(wint_t wc);
int iswxdigit(wint_t wc);
int iswlower(wint_t wc);
int iswupper(wint_t wc);
wint_t towlower(wint_t wc);
wint_t towupper(wint_t wc);

size_t wcslen(const wchar_t *s);
wchar_t *wcschr(const wchar_t *s, wchar_t c);

#endif
