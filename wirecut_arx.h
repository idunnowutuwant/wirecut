#ifndef WIRECUT_ARX_H
#define WIRECUT_ARX_H

#include <stdint.h>

static inline uint64_t wirecut_arx64(uint64_t x) {
    x ^= (x >> 31);
    x ^= (x << 18);
    x ^= (x << 9);
    x ^= (x << 30);
    x ^= (x >> 14);
    x ^= (x >> 24);
    x = (x >> 33) | (x << (64 - 33));
    x ^= (x >> 27);
    return x;
}

#endif
