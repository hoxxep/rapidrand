# rapidrand

An extremely fast pseudo-random number generator, using the [rapidhash](https://github.com/hoxxep/rapidhash) mixing algorithm, designed for the [`rand`](https://crates.io/crates/rand) crate and based on [`wyrand`](https://github.com/wangyi-fudan/wyhash).

* **Extremely fast:** matching the performance of [`fastrand`](https://crates.io/crates/fastrand), [`nanorand`](https://crates.io/crates/nanorand), and [`turborand`](https://crates.io/crates/turborand) that all use the same PRNG construction but their own RNG traits.
* **High quality:** passes TestU01's BigCrush and PractRand up to 16TB.
* **Designed for [`rand`](https://crates.io/crates/rand):** use the `rand` crate traits 10x faster. Perfect for testing, benchmarks, and synthetic datasets.
* **Non-cryptographic:** This is **not** a cryptographic random number generator.

## Usage

`RapidRng` is built for the [`rand`](https://crates.io/crates/rand) crate. Add both crates and use the full `rand` API:

```toml
[dependencies]
rand = "0.10"
rapidrand = "0.1"
```

Seed it from `rand`'s thread-local RNG (itself seeded from the OS) with `from_rng`:

```rust
use rand::{RngExt, SeedableRng, rng};   // RngExt brings `.random()`, `.random_range()`, ...
use rapidrand::RapidRng;

let mut rapid = RapidRng::from_rng(&mut rng());

let coin: bool = rapid.random();
let roll = rapid.random_range(1..=6);
let x: u32 = rapid.random();
```

For a reproducible stream, seed it from a fixed value instead:

```rust
use rand::{RngExt, SeedableRng};
use rapidrand::RapidRng;

let mut rapid = RapidRng::seed_from_u64(42);
let x: u32 = rapid.random();
```

### Without the `rand` crate

`RapidRng` also works with just `rand_core`'s traits, using `next_u64` for a `u64`:

```rust
use rapidrand::RapidRng;
use rand_core::{Rng, SeedableRng};

let mut rng = RapidRng::seed_from_u64(42);
let x = rng.next_u64();
```

Or use the standalone function directly, threading the seed yourself:

```rust
use rapidrand::rapidrng;

let mut state: u64 = 42;
let x = rapidrng(&mut state);
```

## Features

- Default: `rand`
- `rand`: implements `rand_core`'s `Rng` / `SeedableRng` and enables `RapidRng` (rand 0.10).

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.
