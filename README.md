# rapidrand

An extremely fast pseudo-random number generator, using the [rapidhash](https://github.com/hoxxep/rapidhash)
mixing algorithm, designed for the [`rand`](https://crates.io/crates/rand) crate and based on [`w1rand`](https://github.com/wangyi-fudan/wyhash).

* **Extremely fast:** matching the performance of [`fastrand`], [`nanorand`], and [`turborand`] that
  all use the same construction but their own RNG traits.
* **High quality:** passes TestU01's BigCrush and PractRand up to 16TB.
* **Designed for [`rand`](https://crates.io/crates/rand):** a tiny RNG to use with the `rand` traits.
* **Non-cryptographic:** This is **not** a cryptographic random number generator.

## Usage

`RapidRng` is built for the [`rand`](https://crates.io/crates/rand) crate. Add both crates and use
the full `rand` API:

```toml
[dependencies]
rand = "0.10"
rapidrand = "0.1"
```

Seed it from `rand`'s thread-local RNG (itself seeded from the OS) with `from_rng`:

```rust,ignore
use rand::{RngExt, SeedableRng, rng};   // RngExt brings `.random()`, `.random_range()`, ...
use rapidrand::RapidRng;

let mut rapid = RapidRng::from_rng(&mut rng());

let coin: bool = rapid.random();
let roll = rapid.random_range(1..=6);
let x: u32 = rapid.random();
```

For a reproducible stream, seed it from a fixed value instead:

```rust,ignore
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

Or use the free function directly, threading the seed yourself:

```rust
use rapidrand::rapidrng;

let mut seed = 42;
let x = rapidrng(&mut seed);
```

## Features

- `default = ["rand"]`
- `rand` — implements `rand_core`'s `Rng` / `SeedableRng` for `RapidRng` (rand 0.10). This is what
  provides the `from_rng` / `seed_from_u64` constructors shown above.

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.
