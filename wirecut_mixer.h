#ifndef WIRECUT_MIXER_H
#define WIRECUT_MIXER_H

#include <stdint.h>
#include <stddef.h>
#include <immintrin.h>

static inline uint64_t wirecut_mix64_keyed(uint64_t x, const uint64_t k[2]) {
    x *= UINT64_C(0x77BEB5B165DFB6BB);
    x ^= (x >> 33);
    x *= UINT64_C(0x00A3C9A3A955C959);
    x ^= (x >> 33);
    return x;
}

#if defined(__AVX2__)
static inline __m256i wirecut_mix64x4_avx2(__m256i v, const uint64_t k[2]) {
    __m256i k0 = _mm256_set1_epi64x(k[0]);
    __m256i k1 = _mm256_set1_epi64x(k[1]);
    {
        __m256i c = _mm256_set1_epi64x(UINT64_C(0x77BEB5B165DFB6BB));
        __m256i lo = _mm256_mul_epu32(v, c);
        __m256i hi = _mm256_mul_epu32(_mm256_srli_epi64(v, 32), c);
        v = _mm256_add_epi64(lo, _mm256_slli_epi64(hi, 32));
    }
    v = _mm256_xor_si256(v, _mm256_srli_epi64(v, 33));
    {
        __m256i c = _mm256_set1_epi64x(UINT64_C(0x00A3C9A3A955C959));
        __m256i lo = _mm256_mul_epu32(v, c);
        __m256i hi = _mm256_mul_epu32(_mm256_srli_epi64(v, 32), c);
        v = _mm256_add_epi64(lo, _mm256_slli_epi64(hi, 32));
    }
    v = _mm256_xor_si256(v, _mm256_srli_epi64(v, 33));
    return v;
}
#endif

#if defined(__AVX512F__)
static inline __m512i wirecut_mix64x8_avx512(__m512i v, const uint64_t k[2]) {
    __m512i k0 = _mm512_set1_epi64(k[0]);
    __m512i k1 = _mm512_set1_epi64(k[1]);
    {
        __m512i c = _mm512_set1_epi64(UINT64_C(0x77BEB5B165DFB6BB));
#if defined(__AVX512DQ__)
        v = _mm512_mullo_epi64(v, c);
#else
        __m512i lo = _mm512_mul_epu32(v, c);
        __m512i hi = _mm512_mul_epu32(_mm512_srli_epi64(v, 32), c);
        v = _mm512_add_epi64(lo, _mm512_slli_epi64(hi, 32));
#endif
    }
    v = _mm512_xor_si512(v, _mm512_srli_epi64(v, 33));
    {
        __m512i c = _mm512_set1_epi64(UINT64_C(0x00A3C9A3A955C959));
#if defined(__AVX512DQ__)
        v = _mm512_mullo_epi64(v, c);
#else
        __m512i lo = _mm512_mul_epu32(v, c);
        __m512i hi = _mm512_mul_epu32(_mm512_srli_epi64(v, 32), c);
        v = _mm512_add_epi64(lo, _mm512_slli_epi64(hi, 32));
#endif
    }
    v = _mm512_xor_si512(v, _mm512_srli_epi64(v, 33));
    return v;
}
#endif

#endif
