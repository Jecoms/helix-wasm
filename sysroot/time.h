#pragma once
typedef long long time_t;
typedef int clockid_t;
struct timespec { time_t tv_sec; long tv_nsec; };
#define CLOCK_MONOTONIC 1
int clock_gettime(clockid_t clk_id, struct timespec *tp);
