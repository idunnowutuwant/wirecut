#ifndef WIRECUT_KERNEL_H
#define WIRECUT_KERNEL_H

#include <stdint.h>

static inline uint64_t wirecut_mix64(uint64_t x) {
    x *= UINT64_C(0x9E3779B97F4A7C15);
    x ^= (x >> 31);
    x *= UINT64_C(0xBF58476D1CE4E5B9);
    x ^= (x >> 31);
    return x;
}

#endif
