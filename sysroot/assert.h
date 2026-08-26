#pragma once
#ifdef NDEBUG
#define assert(x) ((void)0)
#else
void abort(void);
#define assert(x) ((x) ? (void)0 : abort())
#endif
/* C11 spells the keyword _Static_assert and has <assert.h> provide the
 * static_assert alias (the cpp grammar's scanner uses it). */
#ifndef static_assert
#define static_assert _Static_assert
#endif
