# rapidrand

An extremely fast pseudo-random number generator in rust. Using the [rapidhash](https://github.com/hoxxep/rapidhash) mixing algorithm, designed for the [`rand`](https://crates.io/crates/rand) crate, and based on [`wyrand`](https://github.com/wangyi-fudan/wyhash).

* **Extremely fast:** matching the performance of [`fastrand`](https://crates.io/crates/fastrand), [`nanorand`](https://crates.io/crates/nanorand), and [`turborand`](https://crates.io/crates/turborand) that all use the same PRNG construction but their own RNG traits.
* **High quality:** passes PractRand up to 16TB and TestU01's BigCrush.
* **Designed for [`rand`](https://crates.io/crates/rand):** use the `rand` crate traits 10x faster than the default PRNG and 2x faster than `SmallRng`. Perfect for testing, benchmarks, and synthetic datasets.
* **Tiny:** 66 source lines of code.
* **Non-cryptographic:** This is **not** a cryptographic random number generator.

## Performance

Single-threaded criterion benchmarks from `rapidrand-bench` on an M1 Max. Comparing one `u64` draw, one `u32` draw, and filling a 1 KiB byte buffer (lower ns is better, higher GB/s is better).

| RNG                                                                        |    `u64` |   `u32` |   fill 1 KiB |
|:---------------------------------------------------------------------------|---------:|--------:|-------------:|
| **[rapidrand](https://crates.io/crates/rapidrand) `RapidRng`**             |  0.51 ns | 0.51 ns |   21.67 GB/s |
| [fastrand](https://crates.io/crates/fastrand) `Rng`                        |  0.51 ns | 0.52 ns |   21.49 GB/s |
| [turborand](https://crates.io/crates/turborand) `Rng`                      |  1.25 ns | 0.51 ns |   21.56 GB/s |
| [nanorand](https://crates.io/crates/nanorand) `WyRand`                     |  0.51 ns | 0.51 ns |    3.57 GB/s |
| [rand](https://crates.io/crates/rand) `SmallRng`                           |  1.13 ns | 1.20 ns |    7.05 GB/s |
| [rand_xoshiro](https://crates.io/crates/rand_xoshiro) `Xoshiro256PlusPlus` |  1.14 ns | 1.19 ns |    7.04 GB/s |
| [rand_xoshiro](https://crates.io/crates/rand_xoshiro) `Xoshiro256StarStar` |  1.30 ns | 1.37 ns |    5.84 GB/s |
| [rand_pcg](https://crates.io/crates/rand_pcg) `Pcg32`                      |  2.06 ns | 1.02 ns |    3.93 GB/s |
| [rand_pcg](https://crates.io/crates/rand_pcg) `Pcg64`                      |  1.64 ns | 1.65 ns |    4.81 GB/s |
| [rand](https://crates.io/crates/rand) `StdRng` (ChaCha12)                  |  4.11 ns | 2.26 ns |    2.02 GB/s |
| [rand_chacha](https://crates.io/crates/rand_chacha) `ChaCha8Rng`           |  5.86 ns | 3.04 ns |    1.45 GB/s |
| [rand_chacha](https://crates.io/crates/rand_chacha) `ChaCha20Rng`          | 13.24 ns | 6.70 ns |    0.62 GB/s |

## Usage

`RapidRng` is built for the [`rand`](https://crates.io/crates/rand) crate. Add both crates and use the full `rand` API:

```toml
[dependencies]
rand = "0.10"
rapidrand = "0.1"
```

Seed it from `rand`'s thread-local RNG (itself seeded from the OS) with `from_rng`:

```rust
use rand::{RngExt};   // RngExt brings `.random()`, `.random_range()`, ...
use rapidrand::RapidRng;

let mut rng: RapidRng = rand::make_rng();

let coin: bool = rng.random();
let roll = rng.random_range(1..=6);
let x: u32 = rng.random();
```

For a reproducible stream, seed it from a fixed value instead:

```rust
use rand::{RngExt, SeedableRng};
use rapidrand::RapidRng;

let mut rng = RapidRng::seed_from_u64(42);
let x: u32 = rng.random();
```

Or use the standalone function directly, threading the state yourself:

```rust
use rapidrand::rapidrng;

let mut state: u64 = 42;
let x = rapidrng(&mut state);
```

## Features

- default: `rand`
- `rand`: implements `rand_core`'s `Rng` / `SeedableRng` and enables `RapidRng` (rand 0.10).

## How it works

`rapidrand` is a wyrand-family generator: the same construction as `fastrand`, `nanorand`'s WyRand, and `turborand`, differing only in its constants (from `rapidhash`) and trait integration. Each draw advances a counter and mixes it.

```rust,ignore
// Weyl counter: odd increment → full 2^64 period
state = state.wrapping_add(RAPID_SECRET_ADD);

// 128-bit multiply, XORing a random-looking constant to the second operand
let product = (state as u128) * ((state ^ RAPID_SECRET_XOR) as u128);

// "Fold" the high and low halves of the resulting u128 together via XOR
output = (product >> 64) ^ (product as u64) as u64;
```

Adding an *odd* constant turns the state into a Weyl counter: because the increment is coprime to 2^64, the counter steps through every state before repeating, giving a full 2^64 period from any seed with no bad seeds. That counter is trivially predictable on its own, so each value is scrambled by `rapid_mix`, which multiplies the state by the state XORed with a random-looking constant (so it is not a plain square) and XORs together the high and low halves of the resulting 128-bit product. Multiplication carries low bits upwards into the high half, and the fold brings them back down, spreading the entropy of every input bit across every output bit. Enough to pass PractRand to 16 TB and TestU01's BigCrush, using only 8 bytes of state and a few instructions per draw. The catch is that it is not cryptographic and the state is recoverable from a handful of outputs, stick to `ChaCha`/`StdRng` for cryptographic uses.

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.
