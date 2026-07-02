//! Instruction-count benchmarks comparing `rapidrand` against a spread of popular Rust RNGs,
//! measured with [iai-callgrind](https://docs.rs/iai-callgrind) (valgrind's callgrind).
//!
//! These complement the wall-clock Criterion benchmarks in `rng.rs`: callgrind counts *retired
//! instructions* (plus cache/branch estimates) deterministically, so the numbers are stable across
//! machines and noise-free in CI — ideal for catching regressions that a timing benchmark would
//! bury in variance. They do **not** replace wall-clock numbers: instruction count is not runtime.
//!
//! The three workloads mirror `rng.rs`:
//! * `u64`   — [`rand_core::Rng::next_u64`]-equivalent draws,
//! * `u32`   — [`rand_core::Rng::next_u32`]-equivalent draws,
//! * `fill`  — filling a fixed-size byte buffer.
//!
//! Each generator is constructed in the `#[bench]` argument expression, which iai-callgrind
//! evaluates outside the measured region, so the reported counts reflect generation only, not
//! seeding.

use std::hint::black_box;

use iai_callgrind::{library_benchmark, library_benchmark_group, main};
use rand_core::SeedableRng;

use rapidrand::{RapidRng, rapidrng, rapidrng_single};

/// Deterministic seed so every run measures the same work.
const SEED: u64 = 0x1234_5678_9abc_def0;

/// Size of the buffer used by the `fill` workload.
const FILL_BYTES: usize = 1024;

/// Draws per `u64`/`u32` benchmark. Callgrind counts are already deterministic for a single call,
/// but looping amortises the fixed per-benchmark harness overhead so the reported count tracks the
/// marginal per-draw cost.
const ITERS: usize = 1024;

// ---------------------------------------------------------------------------
// Generator constructors — each is called in a `#[bench]` argument expression, which
// iai-callgrind evaluates outside the measured region. Named `mk_*` to avoid colliding with
// the `#[bench::id]` names, which the macro turns into wrapper functions in the same scope.
// ---------------------------------------------------------------------------

fn mk_rapidrand() -> RapidRng {
    RapidRng::seed_from_u64(SEED)
}
fn mk_rand_small() -> rand::rngs::SmallRng {
    rand::rngs::SmallRng::seed_from_u64(SEED)
}
fn mk_rand_std() -> rand::rngs::StdRng {
    rand::rngs::StdRng::seed_from_u64(SEED)
}
fn mk_pcg32() -> rand_pcg::Pcg32 {
    rand_pcg::Pcg32::seed_from_u64(SEED)
}
fn mk_pcg64() -> rand_pcg::Pcg64 {
    rand_pcg::Pcg64::seed_from_u64(SEED)
}
fn mk_xoshiro256pp() -> rand_xoshiro::Xoshiro256PlusPlus {
    rand_xoshiro::Xoshiro256PlusPlus::seed_from_u64(SEED)
}
fn mk_xoshiro256ss() -> rand_xoshiro::Xoshiro256StarStar {
    rand_xoshiro::Xoshiro256StarStar::seed_from_u64(SEED)
}
fn mk_chacha8() -> rand_chacha::ChaCha8Rng {
    rand_chacha::ChaCha8Rng::seed_from_u64(SEED)
}
fn mk_chacha20() -> rand_chacha::ChaCha20Rng {
    rand_chacha::ChaCha20Rng::seed_from_u64(SEED)
}

fn mk_fastrand() -> fastrand::Rng {
    fastrand::Rng::with_seed(SEED)
}
fn mk_turborand() -> turborand::prelude::Rng {
    use turborand::prelude::*;
    Rng::with_seed(SEED)
}
fn mk_nanorand() -> nanorand::WyRand {
    nanorand::WyRand::new_seed(SEED)
}

// ---------------------------------------------------------------------------
// u64 workload
// ---------------------------------------------------------------------------

// Every `rand_core::Rng` generator shares this generic body; each `#[bench]` monomorphises it for
// the concrete type its argument expression constructs.
#[library_benchmark]
#[bench::rapidrand(mk_rapidrand())]
#[bench::rand_small(mk_rand_small())]
#[bench::rand_std(mk_rand_std())]
#[bench::pcg32(mk_pcg32())]
#[bench::pcg64(mk_pcg64())]
#[bench::xoshiro256pp(mk_xoshiro256pp())]
#[bench::xoshiro256ss(mk_xoshiro256ss())]
#[bench::chacha8(mk_chacha8())]
#[bench::chacha20(mk_chacha20())]
fn u64_rand_core<R: rand_core::Rng>(mut rng: R) -> u64 {
    let mut acc = 0u64;
    for _ in 0..ITERS {
        acc ^= black_box(rng.next_u64());
    }
    acc
}

// Raw `rapidrng` function, as a dependency-free baseline with no trait dispatch.
#[library_benchmark]
fn u64_rapidrand_raw() -> u64 {
    let mut seed = black_box(SEED);
    let mut acc = 0u64;
    for _ in 0..ITERS {
        acc ^= black_box(rapidrng(&mut seed));
    }
    acc
}

// Raw single-constant variant, to track its instruction count against `rapidrng`.
#[library_benchmark]
fn u64_rapidrand_single_raw() -> u64 {
    let mut seed = black_box(SEED);
    let mut acc = 0u64;
    for _ in 0..ITERS {
        acc ^= black_box(rapidrng_single(&mut seed));
    }
    acc
}

#[library_benchmark]
#[bench::fastrand(mk_fastrand())]
fn u64_fastrand(mut rng: fastrand::Rng) -> u64 {
    let mut acc = 0u64;
    for _ in 0..ITERS {
        acc ^= black_box(rng.u64(..));
    }
    acc
}

#[library_benchmark]
#[bench::turborand(mk_turborand())]
fn u64_turborand(rng: turborand::prelude::Rng) -> u64 {
    use turborand::prelude::*;
    let mut acc = 0u64;
    for _ in 0..ITERS {
        acc ^= black_box(rng.gen_u64());
    }
    acc
}

#[library_benchmark]
#[bench::nanorand_wyrand(mk_nanorand())]
fn u64_nanorand(mut rng: nanorand::WyRand) -> u64 {
    use nanorand::Rng as _;
    let mut acc = 0u64;
    for _ in 0..ITERS {
        acc ^= black_box(rng.generate::<u64>());
    }
    acc
}

// ---------------------------------------------------------------------------
// u32 workload
// ---------------------------------------------------------------------------

#[library_benchmark]
#[bench::rapidrand(mk_rapidrand())]
#[bench::rand_small(mk_rand_small())]
#[bench::rand_std(mk_rand_std())]
#[bench::pcg32(mk_pcg32())]
#[bench::pcg64(mk_pcg64())]
#[bench::xoshiro256pp(mk_xoshiro256pp())]
#[bench::xoshiro256ss(mk_xoshiro256ss())]
#[bench::chacha8(mk_chacha8())]
#[bench::chacha20(mk_chacha20())]
fn u32_rand_core<R: rand_core::Rng>(mut rng: R) -> u32 {
    let mut acc = 0u32;
    for _ in 0..ITERS {
        acc ^= black_box(rng.next_u32());
    }
    acc
}

#[library_benchmark]
#[bench::fastrand(mk_fastrand())]
fn u32_fastrand(mut rng: fastrand::Rng) -> u32 {
    let mut acc = 0u32;
    for _ in 0..ITERS {
        acc ^= black_box(rng.u32(..));
    }
    acc
}

#[library_benchmark]
#[bench::turborand(mk_turborand())]
fn u32_turborand(rng: turborand::prelude::Rng) -> u32 {
    use turborand::prelude::*;
    let mut acc = 0u32;
    for _ in 0..ITERS {
        acc ^= black_box(rng.gen_u32());
    }
    acc
}

#[library_benchmark]
#[bench::nanorand_wyrand(mk_nanorand())]
fn u32_nanorand(mut rng: nanorand::WyRand) -> u32 {
    use nanorand::Rng as _;
    let mut acc = 0u32;
    for _ in 0..ITERS {
        acc ^= black_box(rng.generate::<u32>());
    }
    acc
}

// ---------------------------------------------------------------------------
// fill workload — one 1 KiB fill already does plenty of work, so no outer loop.
// ---------------------------------------------------------------------------

#[library_benchmark]
#[bench::rapidrand(mk_rapidrand())]
#[bench::rand_small(mk_rand_small())]
#[bench::rand_std(mk_rand_std())]
#[bench::pcg32(mk_pcg32())]
#[bench::pcg64(mk_pcg64())]
#[bench::xoshiro256pp(mk_xoshiro256pp())]
#[bench::xoshiro256ss(mk_xoshiro256ss())]
#[bench::chacha8(mk_chacha8())]
#[bench::chacha20(mk_chacha20())]
fn fill_rand_core<R: rand_core::Rng>(mut rng: R) -> [u8; FILL_BYTES] {
    let mut buf = [0u8; FILL_BYTES];
    rng.fill_bytes(black_box(&mut buf));
    buf
}

#[library_benchmark]
#[bench::fastrand(mk_fastrand())]
fn fill_fastrand(mut rng: fastrand::Rng) -> [u8; FILL_BYTES] {
    let mut buf = [0u8; FILL_BYTES];
    rng.fill(black_box(&mut buf));
    buf
}

#[library_benchmark]
#[bench::turborand(mk_turborand())]
fn fill_turborand(rng: turborand::prelude::Rng) -> [u8; FILL_BYTES] {
    use turborand::prelude::*;
    let mut buf = [0u8; FILL_BYTES];
    rng.fill_bytes(black_box(&mut buf));
    buf
}

#[library_benchmark]
#[bench::nanorand_wyrand(mk_nanorand())]
fn fill_nanorand(mut rng: nanorand::WyRand) -> [u8; FILL_BYTES] {
    use nanorand::Rng as _;
    let mut buf = [0u8; FILL_BYTES];
    rng.fill_bytes(black_box(&mut buf));
    buf
}

// ---------------------------------------------------------------------------

library_benchmark_group!(
    name = group_u64;
    benchmarks = u64_rand_core, u64_rapidrand_raw, u64_rapidrand_single_raw, u64_fastrand,
        u64_turborand, u64_nanorand
);

library_benchmark_group!(
    name = group_u32;
    benchmarks = u32_rand_core, u32_fastrand, u32_turborand, u32_nanorand
);

library_benchmark_group!(
    name = group_fill;
    benchmarks = fill_rand_core, fill_fastrand, fill_turborand, fill_nanorand
);

main!(library_benchmark_groups = group_u64, group_u32, group_fill);
