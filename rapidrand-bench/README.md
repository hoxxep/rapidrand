# rapidrand-bench

Benchmarks comparing [`rapidrand`](../rapidrand) against other popular Rust PRNGs. This crate is
`publish = false`; it exists only to measure the workspace RNG. Two harnesses:

- **`rng`** — Criterion wall-clock throughput.
- **`callgrind`** — [iai-callgrind](https://docs.rs/iai-callgrind) instruction counts (valgrind's
  callgrind): deterministic and noise-free, so it catches regressions that timing variance would
  hide. Instruction count is *not* runtime; use it alongside the Criterion numbers, not instead.

## Running Criterion (`rng`)

```sh
# full run
cargo bench --bench rng

# quick smoke run
cargo bench --bench rng -- --warm-up-time 0.2 --measurement-time 0.5 --sample-size 10

# only one workload/generator (regex filter)
cargo bench --bench rng -- 'u64/rapidrand'
```

HTML reports land in `target/criterion/`.

## Running iai-callgrind (`callgrind`)

Requires [valgrind](https://valgrind.org/) on `PATH` and the matching runner binary:

```sh
cargo install --version 0.16.1 iai-callgrind-runner   # must match the iai-callgrind dep version

cargo bench --bench callgrind                 # all generators
cargo bench --bench callgrind -- 'rapidrand'  # filter by benchmark id
```

> **macOS/Apple Silicon:** valgrind has no working arm64 build, so this harness only runs on
> Linux (native or a container/VM) and in CI. It still *compiles* on macOS, so `cargo build
> --bench callgrind` catches breakage locally.

The same three workloads as `rng` (`u64`, `u32`, `fill`), each drawing `1024` values (or filling a
`1024`-byte buffer). Each generator is built in the benchmark's argument expression, which
iai-callgrind evaluates outside the measured region, so construction/seeding is excluded.

## What is measured

| group  | workload                          |
|--------|-----------------------------------|
| `u64`  | `next_u64`-equivalent draws       |
| `u32`  | `next_u32`-equivalent draws       |
| `fill` | fill a 1 KiB byte buffer          |

## Generators

The rust-random ecosystem crates and `rapidrand` all implement `rand_core::Rng` (0.10) and are
driven through shared generic helpers; `fastrand`, `turborand`, and `nanorand` use their own native
APIs.

- **rapidrand** — `RapidRng` via `rand_core::Rng`, plus a `rapidrand_raw` baseline calling
  `rapidrng` directly (no trait dispatch), in the `u64` group.
- **fastrand**, **turborand**, **nanorand** (`WyRand`) — wyrand-style competitors.
- **rand** — `SmallRng` and `StdRng`.
- **rand_pcg** — `Pcg32` and `Pcg64`.
- **rand_xoshiro** — `Xoshiro256PlusPlus` and `Xoshiro256StarStar`.
- **rand_chacha** — `ChaCha8Rng` and `ChaCha20Rng` (cryptographic, for reference).

All generators are seeded deterministically so every run measures identical work.
