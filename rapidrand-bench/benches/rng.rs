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

use rapidrand::{RapidRng, rapidrng, rapidrng_single};

/// Deterministic seed so every run measures the same work.
const SEED: u64 = 0x1234_5678_9abc_def0;

/// Size of the buffer used by the `fill` workload.
const FILL_BYTES: usize = 1024;

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
        $f($group, "rapidrand", RapidRng::seed_from_u64(SEED));
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

    // Raw rapidrand function, as a dependency-free baseline.
    g.bench_function("rapidrand_raw", |b| {
        let mut seed = SEED;
        b.iter(|| rapidrng(&mut seed))
    });

    // Raw single-constant variant, to compare against `rapidrng`.
    g.bench_function("rapidrand_single_raw", |b| {
        let mut seed = SEED;
        b.iter(|| rapidrng_single(&mut seed))
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
}

criterion_group!(rng, benches);
criterion_main!(rng);
