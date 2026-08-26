/* Freestanding wchar shim for wasm32-unknown-unknown: the markdown grammar's
 * scanner includes it for wint_t. The wide-string functions and the
 * classification/case family live in wctype.h alongside their
 * implementations in wctype.c. */
#pragma once
#include <wctype.h>
