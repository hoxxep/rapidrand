/*
 * rapidrand.h - an extremely fast, high quality, non-cryptographic PRNG.
 *
 * https://github.com/hoxxep/rapidrand
 *
 * A wyrand-family generator (a Weyl counter run through a folded-multiply output
 * filter) using the rapidhash V1 secrets and a stronger construction
 * proposed by Reiner Pope and Sebastiano Vigna.
 *
 * `rapidrand` (64-bit state) has a 64-bit state and a 2^64 period.
 * `rapidrand128` (128-bit state) has a 128-bit state and 2^128 period, around 20% slower.
 * `rapidrandom` A C++ <random> wrapper for `rapidrand`.
 * `rapidrandom128` A C++ <random> wrapper for `rapidrand128`.
 *
 * NOT a cryptographic random number generator.
 *
 * Licensed under either of Apache-2.0 or MIT at your option.
 *
 * Acknowledgements:
 * - Wang Yi (@wangyi-fudan)
 * - Reiner Pope (@reinerp)
 * - Sebastiano Vigna (@vigna)
 * - Liam Gray (@hoxxep)
 *
 * C usage:
 *   #include "rapidrand.h"
 *
 *   // 64-bit variant (2^64 period; fastest):
 *   rapidrand r = rapidrand_init(42);       // any seed; nearby seeds diverge
 *   uint64_t x = rapidrand_next(&r);        // draw and advance the counter
 *
 *   // 128-bit variant (2^128 period; 20% slower):
 *   rapidrand128 r128 = rapidrand128_init(42);
 *   uint64_t y = rapidrand128_next(&r128);
 *
 * C++ usage:
 *   #include <random>
 *   #include "rapidrand.h"
 *
 *   rapidrandom rng(42);
 *   std::uniform_int_distribution<int> die(1, 6);
 *   int roll = die(rng);
 *   std::shuffle(v.begin(), v.end(), rng);
 */
#ifndef RAPIDRAND_H
#define RAPIDRAND_H

#include <stdint.h>

#if defined(_MSC_VER)
#include <intrin.h>
#if defined(_M_X64)
#pragma intrinsic(_umul128)
#endif
#endif

/*
 * RAPIDRAND_INLINE - force inlining where the compiler supports it.
 *
 * Define it yourself before including this header to override (e.g. to a plain
 * `static inline` for smaller code, or to nothing to emit callable definitions).
 */
#ifndef RAPIDRAND_INLINE
#if defined(_MSC_VER)
#define RAPIDRAND_INLINE static __forceinline
#elif defined(__GNUC__) || defined(__clang__)
#define RAPIDRAND_INLINE static inline __attribute__((always_inline))
#else
#define RAPIDRAND_INLINE static inline
#endif
#endif

/* `noexcept` when compiled as C++, nothing in C. */
#ifndef RAPIDRAND_NOEXCEPT
#ifdef __cplusplus
#define RAPIDRAND_NOEXCEPT noexcept
#else
#define RAPIDRAND_NOEXCEPT
#endif
#endif

/*
 * RAPIDRAND_CONSTEXPR - `constexpr` when compiled as C++14 or newer (so the
 * generators can be evaluated at compile time, e.g. to fill a table or seed a
 * `static_assert`), and nothing in C or older C++ where it would not compile.
 */
#ifndef RAPIDRAND_CONSTEXPR
#if defined(__cplusplus) && __cplusplus >= 201402L
#define RAPIDRAND_CONSTEXPR constexpr
#else
#define RAPIDRAND_CONSTEXPR
#endif
#endif

/*
 * RAPIDRAND_HAS_INT128 - whether an unsigned 128-bit integer type is available
 * for the folded multiply. Falls back to a portable 32x32->64 decomposition
 * otherwise. Define it to 0/1 yourself to override the auto-detection.
 */
#ifndef RAPIDRAND_HAS_INT128
#if defined(__SIZEOF_INT128__)
#define RAPIDRAND_HAS_INT128 1
#else
#define RAPIDRAND_HAS_INT128 0
#endif
#endif

/* Rapidhash V1 secret[0]. Odd, so the counter cycles through the full u64 range. */
#define RAPIDRAND_SECRET_ADD UINT64_C(0x2d358dccaa6c78a5)
/* Rapidhash V1 secret[1]. */
#define RAPIDRAND_SECRET_XOR UINT64_C(0x8bb84b93962eacc9)

/*
 * Portable folded 64x64 -> 128 multiply via 32-bit halves, needing no 128-bit
 * type or intrinsics. Pure arithmetic, so it is usable in constant expressions
 * on every compiler; the fast paths below fall back to it when evaluated at
 * compile time on toolchains whose multiply intrinsic is not constexpr.
 */
RAPIDRAND_CONSTEXPR RAPIDRAND_INLINE uint64_t rapidrand_mix_portable(uint64_t a, uint64_t b) RAPIDRAND_NOEXCEPT {
    uint64_t ha = a >> 32, la = (uint32_t)a;
    uint64_t hb = b >> 32, lb = (uint32_t)b;
    uint64_t rh = ha * hb;
    uint64_t rm0 = ha * lb;
    uint64_t rm1 = hb * la;
    uint64_t rl = la * lb;
    uint64_t t = rl + (rm0 << 32);
    uint64_t c = t < rl;
    uint64_t lo = t + (rm1 << 32);
    uint64_t hi = rh + (rm0 >> 32) + (rm1 >> 32) + c + (uint64_t)(lo < t);
    return lo ^ hi;
}

/*
 * Folded 64-bit multiply: compute the 128-bit product `a * b` and XOR its high
 * and low 64-bit halves together. Matches `rapid_mix` in the Rust crate.
 */
RAPIDRAND_CONSTEXPR RAPIDRAND_INLINE uint64_t rapidrand_mix(uint64_t a, uint64_t b) RAPIDRAND_NOEXCEPT {
#if RAPIDRAND_HAS_INT128
    __uint128_t r = (__uint128_t)a * (__uint128_t)b;
    return (uint64_t)r ^ (uint64_t)(r >> 64);
#elif defined(_MSC_VER) && (defined(_M_X64) || defined(_M_ARM64))
    /* MSVC's 128-bit multiply intrinsics are not usable in constant expressions,
     * so route compile-time evaluation through the portable multiply. */
#if defined(__cplusplus) && __cplusplus >= 201402L && _MSC_VER >= 1925
    if (__builtin_is_constant_evaluated()) {
        return rapidrand_mix_portable(a, b);
    }
#endif
#if defined(_M_X64)
    {
        uint64_t hi;
        uint64_t lo = _umul128(a, b, &hi);
        return lo ^ hi;
    }
#else
    return __umulh(a, b) ^ (a * b);
#endif
#else
    return rapidrand_mix_portable(a, b);
#endif
}

/* ------------------------------------------------------------------------- *
 * rapidrand: 64-bit state.                                                  *
 * ------------------------------------------------------------------------- */

/* A generator carrying its own 64-bit state. */
typedef struct rapidrand {
    uint64_t state;
} rapidrand;

/*
 * Generate a pseudorandom uint64_t and advance the generator.
 *
 * The counter has a full 2^64 period from any seed. Matches `rapidrand` in the
 * Rust crate applied to `r->state`.
 */
RAPIDRAND_CONSTEXPR RAPIDRAND_INLINE uint64_t rapidrand_next(rapidrand *r) RAPIDRAND_NOEXCEPT {
    uint64_t old_state = r->state;
    r->state += RAPIDRAND_SECRET_ADD;
    return rapidrand_mix(r->state, old_state ^ RAPIDRAND_SECRET_XOR);
}

/*
 * Seed a generator from a single uint64_t, mixing it once so nearby seeds
 * (1, 2, 3, ...) produce well-separated streams. Matches `RapidRand::seed_from_u64`
 * in the Rust crate.
 */
RAPIDRAND_CONSTEXPR RAPIDRAND_INLINE rapidrand rapidrand_init(uint64_t seed) RAPIDRAND_NOEXCEPT {
    rapidrand r = { seed };
    r.state = rapidrand_next(&r);
    return r;
}

/* ------------------------------------------------------------------------- *
 * rapidrand128: 128-bit state. Wider, longer-period.                        *
 * ------------------------------------------------------------------------- */

/*
 * A generator carrying a 128-bit Weyl counter. When a native 128-bit integer is
 * available the state is a single `__uint128_t`; otherwise it is held as two
 * 64-bit halves `(hi:lo)` and incremented with carry, so callers see the same
 * type regardless of platform.
 */
typedef struct rapidrand128 {
#if RAPIDRAND_HAS_INT128
    __uint128_t state;
#else
    uint64_t lo;
    uint64_t hi;
#endif
} rapidrand128;

/*
 * Generate a pseudorandom uint64_t and advance the 128-bit counter.
 *
 * The two halves of the counter feed the folded multiply as `mix(lo, hi ^ lo)`
 * and the pre-increment high word is added back on top (`+ hi`) to flatten the
 * fold's structural output spikes, so the output mixes two counter streams; the
 * counter has a full 2^128 period.
 */
RAPIDRAND_CONSTEXPR RAPIDRAND_INLINE uint64_t rapidrand128_next(rapidrand128 *r) RAPIDRAND_NOEXCEPT {
#if RAPIDRAND_HAS_INT128
    uint64_t lo = (uint64_t)r->state;
    uint64_t hi = (uint64_t)(r->state >> 64);
    /* 128-bit odd increment (SECRET_ADD:SECRET_XOR) => full 2^128 period. */
    r->state += ((__uint128_t)RAPIDRAND_SECRET_ADD << 64) | RAPIDRAND_SECRET_XOR;
    return rapidrand_mix(lo, hi ^ lo) + hi;
#else
    uint64_t lo = r->lo;
    uint64_t hi = r->hi;
    /* (hi:lo) += (SECRET_ADD:SECRET_XOR), propagating the carry out of the low half. */
    uint64_t new_lo = lo + RAPIDRAND_SECRET_XOR;
    r->lo = new_lo;
    r->hi = hi + RAPIDRAND_SECRET_ADD + (uint64_t)(new_lo < lo);
    return rapidrand_mix(lo, hi ^ lo) + hi;
#endif
}

/*
 * Seed a 128-bit generator from a single uint64_t, expanding it through the
 * 64-bit generator so both halves of the counter start well-mixed and nearby
 * seeds diverge.
 */
RAPIDRAND_CONSTEXPR RAPIDRAND_INLINE rapidrand128 rapidrand128_init(uint64_t seed) RAPIDRAND_NOEXCEPT {
    rapidrand s = rapidrand_init(seed);
    uint64_t lo = rapidrand_next(&s);
    uint64_t hi = rapidrand_next(&s);

    #if RAPIDRAND_HAS_INT128
        rapidrand128 r = { ((__uint128_t)hi << 64) | lo };
    #else
        rapidrand128 r = { lo, hi };
    #endif
    return r;
}

/* ------------------------------------------------------------------------- *
 * C++ wrappers: rapidrandom / rapidrandom128.                               *
 *                                                                           *
 * Thin, zero-overhead classes over the C generators above. Each satisfies   *
 * the C++ UniformRandomBitGenerator concept (result_type, min, max,         *
 * operator()), so it plugs directly into <random> distributions and         *
 * <algorithm> without an adapter:                                           *
 *                                                                           *
 *   rapidrandom rng(42);                                                    *
 *   std::uniform_int_distribution<int> die(1, 6);                           *
 *   int roll = die(rng);                                                    *
 *   std::shuffle(v.begin(), v.end(), rng);                                  *
 *                                                                           *
 * Named `rapidrandom` (rapidrand + <random>). The C typedef also already    *
 * owns `rapidrand` at global scope, so the class takes its own name.        *
 * ------------------------------------------------------------------------- */
#ifdef __cplusplus

/* 64-bit generator. UniformRandomBitGenerator over `rapidrand`. */
class rapidrandom {
public:
    using result_type = uint64_t;

    static constexpr result_type min() noexcept { return 0; }
    static constexpr result_type max() noexcept { return UINT64_MAX; }

    /* Default seed; pass an explicit seed for a reproducible stream. */
    RAPIDRAND_CONSTEXPR rapidrandom() noexcept : engine_(rapidrand_init(0)) {}
    explicit RAPIDRAND_CONSTEXPR rapidrandom(uint64_t seed) noexcept : engine_(rapidrand_init(seed)) {}
    /* Wrap an existing C generator state as-is, without re-seeding. */
    explicit RAPIDRAND_CONSTEXPR rapidrandom(rapidrand engine) noexcept : engine_(engine) {}

    RAPIDRAND_CONSTEXPR result_type operator()() noexcept { return rapidrand_next(&engine_); }

private:
    rapidrand engine_;
};

/* 128-bit generator. UniformRandomBitGenerator over `rapidrand128` (experimental). */
class rapidrandom128 {
public:
    using result_type = uint64_t;

    static constexpr result_type min() noexcept { return 0; }
    static constexpr result_type max() noexcept { return UINT64_MAX; }

    RAPIDRAND_CONSTEXPR rapidrandom128() noexcept : engine_(rapidrand128_init(0)) {}
    explicit RAPIDRAND_CONSTEXPR rapidrandom128(uint64_t seed) noexcept : engine_(rapidrand128_init(seed)) {}
    explicit RAPIDRAND_CONSTEXPR rapidrandom128(rapidrand128 engine) noexcept : engine_(engine) {}

    RAPIDRAND_CONSTEXPR result_type operator()() noexcept { return rapidrand128_next(&engine_); }

private:
    rapidrand128 engine_;
};

#endif /* __cplusplus */
#endif /* RAPIDRAND_H */
