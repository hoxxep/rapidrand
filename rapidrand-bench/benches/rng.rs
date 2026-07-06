//! Throughput benchmarks comparing `rapidrand` against a spread of popular Rust RNGs.
//!
//! Three workloads are measured for each generator:
//! * `u64`   — a single [`u64`] draw (the natural word size of most of these RNGs),
//! * `u32`   — a single [`u32`] draw,
//! * `fill`  — filling a fixed-size byte buffer.
//!
//! Competitors fall into two camps:
//! * the rust-random ecosystem (`rand`, `rand_pcg`, `rand_xoshiro`, `rand_chacha`) plus
//!   `rapidrand`, which all implement `rand_core::Rng` and are driven through shared generic
//!   helpers, and
//! * `fastrand`, `turborand`, and `nanorand`, which expose their own native APIs and are wired up
//!   individually.

use std::hint::black_box;

use criterion::measurement::WallTime;
use criterion::{BenchmarkGroup, Criterion, Throughput, criterion_group, criterion_main};
use rand_core::SeedableRng;

use rapidrand::{RapidRand, RapidRand128, rapidrand, rapidrand128};

/// Deterministic seed so every run measures the same work.
const SEED: u64 = 0x1234_5678_9abc_def0;

/// Size of the buffer used by the `fill` workload.
const FILL_BYTES: usize = 1024;

// ---------------------------------------------------------------------------
// wyrand-family constructions, reimplemented locally so we can benchmark them
// side by side even though `rapidrand` only ships the `wyranda` variant
// (exported as [`rapidrand`], identical to `wyranda_parallel` below).
//
// All four share the same Weyl counter and the same folded multiply, differing
// only in the output filter — so this group demonstrates that the stronger
// `wyranda` construction costs nothing over base `wyrand`. See the coverage
// analysis in `rapidrand/tests/exhaustive.rs` for what "stronger" means.
// ---------------------------------------------------------------------------

/// Rapidhash V1 secrets (odd increment guarantees a full 2^64 period).
const ADD: u64 = 0x2d358dccaa6c78a5;
const XOR: u64 = 0x8bb84b93962eacc9;

/// Folded 64-bit widening multiply, matching `rapidhash`'s `rapid_mix`.
#[inline(always)]
fn mix(a: u64, b: u64) -> u64 {
    let r = (a as u128).wrapping_mul(b as u128);
    (r as u64) ^ (r >> 64) as u64
}

/// Original two-constant wyrand: `mix(state, state ^ XOR)`. Symmetric, ~39.3% coverage.
#[inline(always)]
fn wyrand(state: &mut u64) -> u64 {
    *state = state.wrapping_add(ADD);
    mix(*state, *state ^ XOR)
}

/// Single-constant w1rand: reuses `ADD` as the xor secret. Symmetric, plus consecutive repeats.
#[inline(always)]
fn w1rand(state: &mut u64) -> u64 {
    *state = state.wrapping_add(ADD);
    mix(*state, *state ^ ADD)
}

/// reinerp's chain variant: `mix(old, new ^ XOR)`. Asymmetric, ~63.2% coverage.
#[inline(always)]
fn wyranda_chain(state: &mut u64) -> u64 {
    let old = *state;
    *state = state.wrapping_add(ADD);
    mix(old, *state ^ XOR)
}

/// reinerp's parallel variant: `mix(new, old ^ XOR)`. Asymmetric, ~63.2% coverage. Shipped as
/// `rapidrand`.
#[inline(always)]
fn wyranda_parallel(state: &mut u64) -> u64 {
    let old = *state;
    *state = state.wrapping_add(ADD);
    mix(*state, old ^ XOR)
}

/// Experimental 128-bit variant that adds the high counter half back into the shipped
/// `rapidrand128` output: `mix(lo, hi ^ lo) + hi`. The extra `add` injects a full-entropy word that
/// flattens the folded-multiply's structural output spikes (notably the `~2x` over-representation of
/// `0`); benchmarked here to confirm the mitigation costs ~one instruction. Not shipped — see the
/// `wyrand128_addhi_*` tests in `rapidrand/tests/exhaustive.rs`.
#[inline(always)]
fn rapidrand128_addhi(state: &mut u128) -> u64 {
    let lo = *state as u64;
    let hi = (*state >> 64) as u64;
    *state = state.wrapping_add(((ADD as u128) << 64) | XOR as u128);
    mix(lo, hi ^ lo).wrapping_add(hi)
}

// ---------------------------------------------------------------------------
// Per-workload helpers for any generator implementing `rand_core::Rng`.
//
// Each `iter` closure draws a single value; Criterion runs it in its own timing
// loop and `black_box`es the returned value, so no manual batching is needed.
// ---------------------------------------------------------------------------

fn bench_u64<R: rand_core::Rng>(g: &mut BenchmarkGroup<'_, WallTime>, name: &str, mut rng: R) {
    g.bench_function(name, |b| b.iter(|| rng.next_u64()));
}

fn bench_u32<R: rand_core::Rng>(g: &mut BenchmarkGroup<'_, WallTime>, name: &str, mut rng: R) {
    g.bench_function(name, |b| b.iter(|| rng.next_u32()));
}

fn bench_fill<R: rand_core::Rng>(g: &mut BenchmarkGroup<'_, WallTime>, name: &str, mut rng: R) {
    g.bench_function(name, |b| {
        let mut buf = [0u8; FILL_BYTES];
        b.iter(|| {
            rng.fill_bytes(black_box(&mut buf));
            black_box(&buf);
        })
    });
}

/// Run `$f` against every `rand_core::Rng` generator, keeping the list in one place.
macro_rules! bench_rand_core_rngs {
    ($group:expr, $f:ident) => {{
        $f($group, "rapidrand", RapidRand::seed_from_u64(SEED));
        $f($group, "rapidrand128", RapidRand128::seed_from_u64(SEED));
        $f(
            $group,
            "rand_small",
            rand::rngs::SmallRng::seed_from_u64(SEED),
        );
        $f($group, "rand_std", rand::rngs::StdRng::seed_from_u64(SEED));
        $f($group, "pcg32", rand_pcg::Pcg32::seed_from_u64(SEED));
        $f($group, "pcg64", rand_pcg::Pcg64::seed_from_u64(SEED));
        $f(
            $group,
            "xoshiro256++",
            rand_xoshiro::Xoshiro256PlusPlus::seed_from_u64(SEED),
        );
        $f(
            $group,
            "xoshiro256**",
            rand_xoshiro::Xoshiro256StarStar::seed_from_u64(SEED),
        );
        $f(
            $group,
            "chacha8",
            rand_chacha::ChaCha8Rng::seed_from_u64(SEED),
        );
        $f(
            $group,
            "chacha20",
            rand_chacha::ChaCha20Rng::seed_from_u64(SEED),
        );
    }};
}

// ---------------------------------------------------------------------------

fn bench_u64_workload(c: &mut Criterion) {
    let mut g = c.benchmark_group("u64");
    g.throughput(Throughput::Bytes(size_of::<u64>() as u64));

    // Shipped rapidrand function (the wyranda construction), as a dependency-free baseline.
    g.bench_function("rapidrand_raw", |b| {
        let mut seed = SEED;
        b.iter(|| rapidrand(&mut seed))
    });

    // Shipped 128-bit rapidrand function, the wider longer-period variant.
    g.bench_function("rapidrand128_raw", |b| {
        let mut seed = SEED as u128;
        b.iter(|| rapidrand128(&mut seed))
    });

    // Experimental `+ hi` variant, to confirm the bias mitigation costs ~one instruction.
    g.bench_function("rapidrand128_addhi_raw", |b| {
        let mut seed = SEED as u128;
        b.iter(|| rapidrand128_addhi(&mut seed))
    });

    // The wyrand-family constructions reimplemented locally, to confirm they are all the same speed
    // (only their output quality differs — see the module comment above).
    g.bench_function("wyrand", |b| {
        let mut seed = SEED;
        b.iter(|| wyrand(&mut seed))
    });
    g.bench_function("w1rand", |b| {
        let mut seed = SEED;
        b.iter(|| w1rand(&mut seed))
    });
    g.bench_function("wyranda_chain", |b| {
        let mut seed = SEED;
        b.iter(|| wyranda_chain(&mut seed))
    });
    g.bench_function("wyranda_parallel", |b| {
        let mut seed = SEED;
        b.iter(|| wyranda_parallel(&mut seed))
    });

    g.bench_function("fastrand", |b| {
        let mut rng = fastrand::Rng::with_seed(SEED);
        b.iter(|| rng.u64(..))
    });

    // turborand uses interior mutability: its generation methods take `&self`.
    g.bench_function("turborand", |b| {
        use turborand::prelude::*;
        let rng = Rng::with_seed(SEED);
        b.iter(|| rng.gen_u64())
    });

    g.bench_function("nanorand_wyrand", |b| {
        use nanorand::Rng;
        let mut rng = nanorand::WyRand::new_seed(SEED);
        b.iter(|| rng.generate::<u64>())
    });

    bench_rand_core_rngs!(&mut g, bench_u64);
    g.finish();
}

/// Controlled experiments isolating why `&self`/`Cell` RNGs (e.g. turborand) can
/// be ~2.4x slower on a single `u64` draw than `&mut` RNGs, despite identical
/// mixing math. All four run the same `rapidrand` arithmetic; only the state
/// write-back pattern differs. Findings:
/// * `mut_store_before`  — `&mut` state, stored before the mix → register-carried, fast.
/// * `mut_store_after`   — `&mut` state, stored after the mix → register-carried,  fast.
/// * `cell_store_before` — `Cell` state, stored before the mix → register-carried, fast.
/// * `cell_store_after`  — `Cell` state, stored after the mix  → memory-carried,   slow.
///
/// The slow case is the only one where the compiler cannot keep state in a
/// register, so the state recurrence routes through store→load forwarding.
///
/// `mut_store_before` represents rapidrand; `cell_store_after` represents turborand.
fn bench_writeback_workload(c: &mut Criterion) {
    const ADD: u64 = 0x2d358dccaa6c78a5;
    const XOR: u64 = 0x8bb84b93962eacc9;
    #[inline(always)]
    fn mix(a: u64, b: u64) -> u64 {
        let r = (a as u128).wrapping_mul(b as u128);
        (r as u64) ^ (r >> 64) as u64
    }

    let mut g = c.benchmark_group("writeback");
    g.throughput(Throughput::Bytes(size_of::<u64>() as u64));

    // &self + Cell, store AFTER the mix (turborand's effective pattern).
    g.bench_function("cell_store_after", |b| {
        let state = core::cell::Cell::new(SEED);
        b.iter(|| {
            let s = state.get().wrapping_add(ADD);
            let r = mix(s, s ^ XOR);
            state.set(s);
            r
        })
    });

    // &self + Cell, store BEFORE the mix. Same Cell, different store order.
    g.bench_function("cell_store_before", |b| {
        let state = core::cell::Cell::new(SEED);
        b.iter(|| {
            let s = state.get().wrapping_add(ADD);
            state.set(s);
            mix(s, s ^ XOR)
        })
    });

    // &mut local (register-carried state, like rapidrand), store AFTER the mix.
    g.bench_function("mut_store_after", |b| {
        let mut state = SEED;
        b.iter(|| {
            let s = state.wrapping_add(ADD);
            let r = mix(s, s ^ XOR);
            state = s;
            r
        })
    });

    // &mut local (register-carried state, like rapidrand), store BEFORE the mix (like rapidrand)
    g.bench_function("mut_store_before", |b| {
        let mut state = SEED;
        b.iter(|| {
            state = state.wrapping_add(ADD);
            let r = mix(state, state ^ XOR);
            r
        })
    });

    // &mut local increment and store AFTER the mix, simply as an experiment.
    g.bench_function("mut_inc_and_store_after", |b| {
        let mut state = SEED;
        b.iter(|| {
            let r = mix(state, state ^ XOR);
            state = state.wrapping_add(ADD);
            r
        })
    });

    g.finish();
}

fn bench_u32_workload(c: &mut Criterion) {
    let mut g = c.benchmark_group("u32");
    g.throughput(Throughput::Bytes(size_of::<u32>() as u64));

    g.bench_function("fastrand", |b| {
        let mut rng = fastrand::Rng::with_seed(SEED);
        b.iter(|| rng.u32(..))
    });

    g.bench_function("turborand", |b| {
        use turborand::prelude::*;
        let rng = Rng::with_seed(SEED);
        b.iter(|| rng.gen_u32())
    });

    g.bench_function("nanorand_wyrand", |b| {
        use nanorand::Rng;
        let mut rng = nanorand::WyRand::new_seed(SEED);
        b.iter(|| rng.generate::<u32>())
    });

    bench_rand_core_rngs!(&mut g, bench_u32);
    g.finish();
}

fn bench_fill_workload(c: &mut Criterion) {
    let mut g = c.benchmark_group("fill");
    g.throughput(Throughput::Bytes(FILL_BYTES as u64));

    g.bench_function("fastrand", |b| {
        let mut rng = fastrand::Rng::with_seed(SEED);
        let mut buf = [0u8; FILL_BYTES];
        b.iter(|| {
            rng.fill(black_box(&mut buf));
            black_box(&buf);
        })
    });

    g.bench_function("turborand", |b| {
        use turborand::prelude::*;
        let rng = Rng::with_seed(SEED);
        let mut buf = [0u8; FILL_BYTES];
        b.iter(|| {
            rng.fill_bytes(black_box(&mut buf));
            black_box(&buf);
        })
    });

    g.bench_function("nanorand_wyrand", |b| {
        use nanorand::Rng;
        let mut rng = nanorand::WyRand::new_seed(SEED);
        let mut buf = [0u8; FILL_BYTES];
        b.iter(|| {
            rng.fill_bytes(black_box(&mut buf));
            black_box(&buf);
        })
    });

    bench_rand_core_rngs!(&mut g, bench_fill);
    g.finish();
}

fn benches(c: &mut Criterion) {
    bench_u64_workload(c);
    bench_u32_workload(c);
    bench_fill_workload(c);
    bench_writeback_workload(c);
}

criterion_group!(rng, benches);
criterion_main!(rng);
