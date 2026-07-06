/*
 * Compile-only check that rapidrand.h builds as C (not C++).
 *
 * It calls every public function so signature/type mistakes and stray C++-isms
 * in the C path surface as compile errors. Output correctness is verified by the
 * C++ test and the separate Rust equivalence test, so nothing here is run.
 */
#include "rapidrand.h"

int main(void) {
    rapidrand seeded = rapidrand_init(1);
    rapidrand raw = { 2 };
    rapidrand128 wide = rapidrand128_init(3);
    uint64_t acc = 0;

    acc ^= rapidrand_next(&seeded);
    acc ^= rapidrand_next(&raw);
    acc ^= rapidrand128_next(&wide);
    acc ^= rapidrand_mix(acc, acc + 1);
    acc ^= rapidrand_mix_portable(acc, acc + 2);

    return (int)acc;
}
