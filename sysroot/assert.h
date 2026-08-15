#pragma once
#ifdef NDEBUG
#define assert(x) ((void)0)
#else
void abort(void);
#define assert(x) ((x) ? (void)0 : abort())
#endif
