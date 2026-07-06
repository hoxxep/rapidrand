# rapidrand

An extremely fast pseudo-random number generator in rust. Using the [rapidhash](https://github.com/hoxxep/rapidhash) mixing algorithm, designed for the [`rand`](https://crates.io/crates/rand) crate, and based on improved variants of [`wyrand`](https://github.com/wangyi-fudan/wyhash/issues/156).

* **Extremely fast:** matching the performance of [`fastrand`](https://crates.io/crates/fastrand), [`nanorand`](https://crates.io/crates/nanorand), and [`turborand`](https://crates.io/crates/turborand), which all use the wyrand construction behind their own RNG frameworks.
* **High quality:** reaches ~63% of its output space where plain wyrand reaches only ~39% ([see below](#quality-wyranda-vs-wyrand)). Passes [PractRand](https://pracrand.sourceforge.net/) up to 32TB, [TestU01](https://github.com/umontreal-simul/TestU01-2009)'s BigCrush, and [coll-birth](https://github.com/vigna/coll-birth-rs) for at least 4T elements.
* **Designed for [`rand`](https://crates.io/crates/rand):** use the `rand` crate traits 10x faster than the default PRNG and 2x faster than `SmallRng`. Perfect for testing, benchmarks, and synthetic datasets.
* **Tiny:** 61 source lines of code.
* **64-bit and 128-bit:** 64-bit and 128-bit versions available.
* **Rust crate and C/C++ header:** Available as either a rust crate or standalone C/C++ header, `rapidrand.h`, with full support for `<random>`..
* **Non-cryptographic:** This is **not** a cryptographic random number generator.

## Performance

Single-threaded criterion benchmarks from `rapidrand-bench` on an M1 Max. Comparing one `u64` draw, one `u32` draw, and filling a 1 KiB byte buffer (lower ns is better, higher GB/s is better).

| RNG                                                                        |    `u64` |   `u32` |   fill 1 KiB |
|:---------------------------------------------------------------------------|---------:|--------:|-------------:|
| **[rapidrand](https://crates.io/crates/rapidrand) `RapidRand`**             |  0.51 ns | 0.51 ns |   21.67 GB/s |
| [fastrand](https://crates.io/crates/fastrand) `Rng`*                       |  0.51 ns | 0.52 ns |   21.49 GB/s |
| [turborand](https://crates.io/crates/turborand) `Rng`*                     |  1.25 ns | 0.51 ns |   21.56 GB/s |
| [nanorand](https://crates.io/crates/nanorand) `WyRand`*                    |  0.51 ns | 0.51 ns |    3.57 GB/s |
| [rand](https://crates.io/crates/rand) `SmallRng`                           |  1.13 ns | 1.20 ns |    7.05 GB/s |
| [rand_xoshiro](https://crates.io/crates/rand_xoshiro) `Xoshiro256PlusPlus` |  1.14 ns | 1.19 ns |    7.04 GB/s |
| [rand_xoshiro](https://crates.io/crates/rand_xoshiro) `Xoshiro256StarStar` |  1.30 ns | 1.37 ns |    5.84 GB/s |
| [rand_pcg](https://crates.io/crates/rand_pcg) `Pcg32`                      |  2.06 ns | 1.02 ns |    3.93 GB/s |
| [rand_pcg](https://crates.io/crates/rand_pcg) `Pcg64`                      |  1.64 ns | 1.65 ns |    4.81 GB/s |
| [rand](https://crates.io/crates/rand) `StdRng` (ChaCha12)                  |  4.11 ns | 2.26 ns |    2.02 GB/s |
| [rand_chacha](https://crates.io/crates/rand_chacha) `ChaCha8Rng`           |  5.86 ns | 3.04 ns |    1.45 GB/s |
| [rand_chacha](https://crates.io/crates/rand_chacha) `ChaCha20Rng`          | 13.24 ns | 6.70 ns |    0.62 GB/s |

*These crates implement their own RNG framework and use a [lower quality](#quality-wyranda-vs-wyrand) wyrand construction.

## Usage

`RapidRand` is built for the [`rand`](https://crates.io/crates/rand) crate. Add both crates and use the full `rand` API:

```toml
[dependencies]
rand = "0.10"
rapidrand = "0.2"
```

Seed it from `rand`'s thread-local RNG (itself seeded from the OS) with `from_rng`:

```rust
use rand::{RngExt};   // RngExt brings `.random()`, `.random_range()`, ...
use rapidrand::RapidRand;

let mut rng: RapidRand = rand::make_rng();

let coin: bool = rng.random();
let roll = rng.random_range(1..=6);
let x: u32 = rng.random();
```

For a reproducible stream, seed it from a fixed value instead:

```rust
use rand::{RngExt, SeedableRng};
use rapidrand::RapidRand;

let mut rng = RapidRand::seed_from_u64(42);
let x: u32 = rng.random();
```

Or use the standalone function directly, threading the state yourself:

```rust
use rapidrand::rapidrand;

let mut state: u64 = 42;
let x = rapidrand(&mut state);
```

## Features

- default: `rand`
- `rand`: implements `rand_core`'s `Rng` / `SeedableRng` and enables `RapidRand` (rand 0.10).

## How it works

`rapidrand` is a wyrand-family generator: a Weyl counter run through a folded-multiply output filter, differing from `fastrand`, `nanorand`'s WyRand, and `turborand` in its constants (from `rapidhash`), its trait integration, and its use of the stronger **wyranda** filter ([see below](#quality-wyranda-vs-wyrand)). Each draw advances the counter and mixes it.

```rust,ignore
// Weyl counter: odd increment → full 2^64 period
let old_state = state;
state = state.wrapping_add(RAPID_SECRET_ADD);

// 128-bit multiply of the new state by the old state (XORed with a random-looking constant)
let product = (state as u128) * ((old_state ^ RAPID_SECRET_XOR) as u128);

// "Fold" the high and low halves of the resulting u128 together via XOR
output = ((product >> 64) ^ product) as u64;
```

Adding an *odd* constant turns the state into a Weyl counter: because the increment is coprime to 2^64, the counter steps through every state before repeating, giving a full 2^64 period from any seed with no bad seeds. That counter is trivially predictable on its own, so each value is scrambled by `rapid_mix`, which multiplies two of the counter's states together (the new state by the old state XORed with a random-looking constant) and XORs together the high and low halves of the resulting 128-bit product. Multiplication carries low bits upwards into the high half, and the fold brings them back down, spreading the entropy of every input bit across every output bit. Enough to pass PractRand to 32 TB and TestU01's BigCrush, using only 8 bytes of state and a few instructions per draw. The catch is that it is not cryptographic and the state is recoverable from a handful of outputs, stick to `ChaCha`/`StdRng` for cryptographic uses.

## Quality: wyranda vs wyrand

The original wyrand construction used by `fastrand`, `nanorand`, and `turborand` mixes a *single* counter state:

```text
state += ADD
output = folded_multiply(state, state ^ XOR)
```

The fold-multiply is commutative, and `x → x ^ XOR` pairs every state with a distinct partner that produces the *same* output, so this filter is exactly 2-to-1. It can reach at most half of its output values, and in practice only **~39.3%** of the output space, where an ideal random function reaches **~63.2%**. Many values are never produced, and every value that *is* produced appears an even number of times.

@vigna and @reinerp analysed this on the wyhash issue tracker ([issue #130](https://github.com/wangyi-fudan/wyhash/issues/130#issuecomment-4835746792)) and proposed **wyranda** ([issue #156](https://github.com/wangyi-fudan/wyhash/issues/156)): feed the multiply from *two* consecutive counter states so it is no longer a commutative function of one value. `rapidrand` ships this variant. It costs nothing (same instruction count, same throughput) and restores full random-function coverage.

Exhaustively measured over a complete period for a 16-bit model of each construction (`cargo test --test exhaustive -- --nocapture`):

| construction  | used by                       | output space reached |
|:--------------|:------------------------------|---------------------:|
| `wyrand`      | fastrand, nanorand, turborand |               ~39.3% |
| **`wyranda`** | **rapidrand**                 |           **~63.2%** |

The ~39.3% and ~63.2% figures are width-independent limits (`1 - e^(-1/2)` and `1 - 1/e`); the 16-bit measurement matches them within noise and is pinned to the shipped `u64` code by the same test.

## Acknowledgements

This crate is based on the excellent work of, and gives many thanks to:
* [Reiner Pope](https://github.com/reinerp) for proposing the improvements to wyhash and a 128-bit variant ([wyhash/issue #156](https://github.com/wangyi-fudan/wyhash/issues/156)).
* [Sebastiano Vigna](https://github.com/vigna) for his analysis on wyrand's weaknesses ([wyhash/issue #130](https://github.com/wangyi-fudan/wyhash/issues/130#issuecomment-4835746792)).
* [Wany Yi](https://github.com/wangyi-fudan) et al. for the [original wyrand construction](https://github.com/wangyi-fudan/wyhash).

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.
