// C++ test for rapidrand.h.
//
// Verifies the single-header generator compiles and produces the correct
// results from C++ across compilers, language standards, and targets (32/64-bit,
// native __int128 vs. the portable multiply path). This is a C++-only test;
// Rust <-> C++ stream equivalence is covered by a separate test.
//
// Exit code 0 on success, 1 on any runtime mismatch. Compile-time (constexpr)
// checks are `static_assert`s, so a mismatch there fails the build instead.

#include <cstdint>
#include <cstdio>
#include <limits>
#include <random>
#include <algorithm>
#include <vector>

#include "rapidrand.h"

namespace {

// Golden outputs, pinned to the Rust crate.
// `rapidrand` advanced from a raw state of 42:
const std::uint64_t GOLD_RAW42[4] = {
    UINT64_C(0x4d76eecb32f856cd), UINT64_C(0x603e2c990e294a34),
    UINT64_C(0x773fe4015f29bf77), UINT64_C(0xb3fb14a990912c09),
};
// `RapidRand::seed_from_u64(42)` == `rapidrand_init(42)`:
const std::uint64_t GOLD_SEED42[4] = {
    UINT64_C(0xa479de8b0772c88e), UINT64_C(0x4193166b6cbce80d),
    UINT64_C(0xc01e8d40942abd24), UINT64_C(0xd409cd732c181cab),
};
// `rapidrand128` advanced from a raw 128-bit state of 42, pinned to the Rust
// crate's `rapidrand128`:
const std::uint64_t GOLD_128_RAW42[4] = {
    UINT64_C(0xec3c6636a3a858ec), UINT64_C(0x5472dbfbaa8a2075),
    UINT64_C(0x2c545e695efaa9f4), UINT64_C(0xb0e325e61498c821),
};
// `rapidrand128_init(42)` == `RapidRand128::seed_from_u64(42)` (state = seed + 1):
const std::uint64_t GOLD_128_42[4] = {
    UINT64_C(0x77f4b1ca39d705d4), UINT64_C(0xdc146a194ad790cb),
    UINT64_C(0x021857f3710dd1de), UINT64_C(0x6b4ec955bdb6007e),
};

int g_fails = 0;

void expect(const char *what, std::uint64_t got, std::uint64_t want) {
    if (got != want) {
        std::printf("FAIL %s: got %016llx want %016llx\n", what,
                    static_cast<unsigned long long>(got),
                    static_cast<unsigned long long>(want));
        ++g_fails;
    }
}

// The generators must match the golden (Rust-derived) streams.
void check_golden() {
    rapidrand raw = { 42 };
    for (int i = 0; i < 4; ++i) expect("raw42", rapidrand_next(&raw), GOLD_RAW42[i]);

    rapidrand seeded = rapidrand_init(42);
    for (int i = 0; i < 4; ++i) expect("seed42", rapidrand_next(&seeded), GOLD_SEED42[i]);

    // Raw 128-bit state of 42, however the platform represents it.
    rapidrand128 raw128{};
#if RAPIDRAND_HAS_INT128
    raw128.state = 42;
#else
    raw128.lo = 42;
#endif
    for (int i = 0; i < 4; ++i) expect("raw128", rapidrand128_next(&raw128), GOLD_128_RAW42[i]);

    rapidrand128 wide = rapidrand128_init(42);
    for (int i = 0; i < 4; ++i) expect("wide42", rapidrand128_next(&wide), GOLD_128_42[i]);

    // Seeding adds 1 so seed 0 never lands on the all-zero state (whose output is 0).
    rapidrand128 zero = rapidrand128_init(0);
    if (rapidrand128_next(&zero) == 0) { std::printf("FAIL seed 0 produced 0\n"); ++g_fails; }
}

// The C++ wrappers must produce exactly the same stream as the C core.
void check_wrapper_parity() {
    rapidrandom rng(42);
    rapidrand core = rapidrand_init(42);
    for (int i = 0; i < 4096; ++i) expect("wrap64", rng(), rapidrand_next(&core));

    rapidrandom128 rng128(123);
    rapidrand128 core128 = rapidrand128_init(123);
    for (int i = 0; i < 4096; ++i) expect("wrap128", rng128(), rapidrand128_next(&core128));

    // Wrapping an existing C state must not re-seed it.
    rapidrand shared = rapidrand_init(99);
    rapidrandom wrapped(shared);
    expect("wrap-asis", wrapped(), rapidrand_next(&shared));
}

// UniformRandomBitGenerator conformance: the wrappers drive <random>/<algorithm>.
void check_urbg() {
    static_assert(rapidrandom::min() == 0, "min");
    static_assert(rapidrandom::max() == std::numeric_limits<std::uint64_t>::max(), "max");
    static_assert(rapidrandom128::min() == 0, "min128");

    rapidrandom rng(7);
    std::uniform_int_distribution<int> die(1, 6);
    for (int i = 0; i < 1000; ++i) {
        int r = die(rng);
        if (r < 1 || r > 6) { std::printf("FAIL die out of range: %d\n", r); ++g_fails; }
    }

    std::vector<int> v(64);
    for (int i = 0; i < 64; ++i) v[i] = i;
    std::shuffle(v.begin(), v.end(), rng);
    long sum = 0;
    for (std::size_t i = 0; i < v.size(); ++i) sum += v[i];
    if (sum != 64 * 63 / 2) { std::printf("FAIL shuffle lost elements\n"); ++g_fails; }
}

#if defined(__cplusplus) && __cplusplus >= 201402L
// Compile-time evaluation must match the runtime golden values. If the constexpr
// path (including the portable multiply used during constant evaluation) ever
// diverges, these fail to compile.
constexpr std::uint64_t ct_raw42_first() {
    rapidrand r = { 42 };
    return rapidrand_next(&r);
}
static_assert(ct_raw42_first() == UINT64_C(0x4d76eecb32f856cd), "constexpr raw mismatch");

constexpr std::uint64_t ct_seed42_first() {
    rapidrand r = rapidrand_init(42);
    return rapidrand_next(&r);
}
static_assert(ct_seed42_first() == UINT64_C(0xa479de8b0772c88e), "constexpr seed mismatch");

constexpr std::uint64_t ct_wrapper_first() {
    rapidrandom g(42);
    return g();
}
static_assert(ct_wrapper_first() == UINT64_C(0xa479de8b0772c88e), "constexpr wrapper mismatch");

constexpr std::uint64_t ct_128_first() {
    rapidrand128 r = rapidrand128_init(42);
    return rapidrand128_next(&r);
}
static_assert(ct_128_first() == UINT64_C(0x77f4b1ca39d705d4), "constexpr 128 mismatch");
#endif

}  // namespace

int main() {
    check_golden();
    check_wrapper_parity();
    check_urbg();

    if (g_fails != 0) {
        std::printf("FAILED: %d check(s)\n", g_fails);
        return 1;
    }
    std::printf("all C++ checks passed (C++ %ld)\n", static_cast<long>(__cplusplus));
    return 0;
}
