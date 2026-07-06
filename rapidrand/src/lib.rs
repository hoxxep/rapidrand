#![cfg_attr(docsrs, doc = include_str!("../README.md"))]
#![cfg_attr(not(docsrs), doc = "# Rapidrand")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, doc(auto_cfg(hide(docsrs))))]
#![no_std]
#![deny(missing_docs)]
#![deny(unused_must_use)]

#[cfg(feature = "rand")]
use rand_core::{SeedableRng, TryRng, utils::fill_bytes_via_next_word};

/// Rapidhash V1 secret[0].
///
/// Use an odd number for incrementing to guarantee the state cycles through the full u64 range.
const RAPID_SECRET_ADD: u64 = 0x2d358dccaa6c78a5;

/// Rapidhash V1 secret[1].
const RAPID_SECRET_XOR: u64 = 0x8bb84b93962eacc9;

/// Generate a pseudorandom `u64` and advance `state`.
///
/// This PRNG is not a cryptographic random number generator.
///
/// # Why not plain wyrand?
///
/// `rapidrand` uses [rapidhash](https://github.com/hoxxep/rapidhash) secrets with the **wyranda**
/// construction: a [wyrand](https://github.com/wangyi-fudan/wyhash)-family Weyl generator whose
/// output multiply is fed from *two* consecutive counter states instead of one. This improvement
/// was proposed by Sebastiano Vigna and Reiner Pope
/// ([wyhash/issue#130](https://github.com/wangyi-fudan/wyhash/issues/130#issuecomment-4835746792) and
/// [wyhash/issue#156](https://github.com/wangyi-fudan/wyhash/issues/156)).
///
/// Plain wyrand mixes a single state, `rapid_mix(state, state ^ XOR)`. Because `rapid_mix` is
/// commutative and `x -> x ^ XOR` is a fixed-point-free involution, `f(x) == f(x ^ XOR)`: the output
/// filter is exactly 2-to-1, so it can only ever reach `1 - e^(-1/2) ≈ 39.3%` of its output space,
/// where a well-behaved generator reaches `1 - 1/e ≈ 63.2%`. wyranda multiplies the *old* state by
/// the *new* (xored) state, which is not a commutative function of a single value; that removes the
/// symmetry and restores the full `~63.2%` coverage at identical speed. See the crate-level docs for
/// the full explanation and coverage table.
///
/// # Example
/// ```rust
/// use rapidrand::rapidrand;
///
/// let mut state: u64 = 42;
/// let value: u64 = rapidrand(&mut state);
/// ```
#[inline(always)]
#[must_use]
pub const fn rapidrand(state: &mut u64) -> u64 {
    let old_state = *state;
    *state = state.wrapping_add(RAPID_SECRET_ADD);

    // folded multiply: (new state) * (old state ^ XOR)
    let r = (*state as u128).wrapping_mul((old_state ^ RAPID_SECRET_XOR) as u128);
    (r as u64) ^ (r >> 64) as u64
}

/// Generate a pseudorandom `u64` and advance a 128-bit `state`.
///
/// A wider, longer-period sibling of [`rapidrand`], intended for parallel and distributed workloads
/// that need many independent streams. It runs a **128-bit** Weyl counter and passes both of its
/// 64-bit halves `hi:lo` through a rapidhash-style **double-multiply** avalanche. Compared to the
/// 64-bit [`rapidrand`] this is designed to buy three properties:
///
/// * **Full `2^128` period.** The counter is incremented by the odd 128-bit constant
///   `(RAPID_SECRET_ADD:RAPID_SECRET_XOR)`, so it steps through every 128-bit state before repeating —
///   a full period from any seed, with no bad seeds.
/// * **100% output coverage.** The double multiply reaches *every* one of the `2^64` output values, in
///   an essentially flat distribution (each value produced about equally often). A single fold-multiply
///   instead structurally over-represents a handful of outputs — `0` and `u64::MAX`
///   [most of all](https://github.com/wangyi-fudan/wyhash/issues/156#issuecomment-4888162118) — which
///   the second multiply and `.wrapping_add(hi)` disperses. This is measured exhaustively on
///   narrow-width models in `tests/exhaustive.rs` (`var/mean` and `peak/mean` both `~1`).
/// * **Stronger stream separation.** Because each output is a strong avalanche of the *entire* 128-bit
///   counter, states seeded from different values (or offset within the period) decorrelate, so distinct
///   seeds behave as independent streams. The 64-bit generator cannot safely offer this: its `2^64`
///   period is too short to hand out non-overlapping streams, and its single multiply mixes related
///   seeds too weakly.
///
/// Concretely, with `p = hi · lo` the widening multiply of the two *pre-increment* counter halves, the
/// output is `hi(r) ^ (lo(r) + hi)` where `r = (lo(p) ^ XOR) · (hi(p) ^ lo)` is the second widening
/// multiply and `hi(·)`/`lo(·)` are the high and low 64-bit halves of a 128-bit product.
///
/// This PRNG is not a cryptographic random number generator.
///
/// # Example
/// ```rust
/// use rapidrand::rapidrand128;
///
/// let mut state: u128 = 42;
/// let value: u64 = rapidrand128(&mut state);
/// ```
#[inline(always)]
#[must_use]
pub const fn rapidrand128(state: &mut u128) -> u64 {
    let lo = *state as u64;
    let hi = (*state >> 64) as u64;
    *state = state.wrapping_add(((RAPID_SECRET_ADD as u128) << 64) | RAPID_SECRET_XOR as u128);

    // first product
    let p = (hi as u128).wrapping_mul(lo as u128);
    let phi = (p >> 64) as u64;
    let plo = p as u64;

    // second product; XOR `lo`
    let r = ((plo ^ RAPID_SECRET_XOR) as u128).wrapping_mul((phi ^ lo) as u128);

    // fold; ADD `hi`
    (r >> 64) as u64 ^ (r as u64).wrapping_add(hi)
}

/// A random number generator that uses the rapidhash mixing algorithm.
///
/// This deterministic RNG is optimized for speed and throughput. This is not a cryptographic random
/// number generator.
///
/// With the `rand` feature, this RNG implements [`rand_core::Rng`] and [`rand_core::SeedableRng`]
/// on top of [`rapidrand`] and is fully compatible with `rand` v0.10.
///
/// # Examples
/// Seed it from `rand`'s thread-local RNG (itself seeded from the OS) with `from_rng`:
///
/// ```rust
/// use rand::{RngExt};   // RngExt brings `.random()`, `.random_range()`, ...
/// use rapidrand::RapidRand;
///
/// let mut rng: RapidRand = rand::make_rng();
///
/// let coin: bool = rng.random();
/// let roll = rng.random_range(1..=6);
/// let value: u32 = rng.random();
/// ```
///
/// For a reproducible stream, seed it from a fixed value instead:
///
/// ```rust
/// use rand::{RngExt, SeedableRng};
/// use rapidrand::RapidRand;
///
/// let mut rng = RapidRand::seed_from_u64(42);
/// let value: u32 = rng.random();
/// ```
#[cfg(feature = "rand")]
#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub struct RapidRand {
    state: u64,
}

#[cfg(feature = "rand")]
impl TryRng for RapidRand {
    type Error = core::convert::Infallible;

    #[inline]
    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        Ok(rapidrand(&mut self.state) as u32)
    }

    #[inline]
    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok(rapidrand(&mut self.state))
    }

    #[inline]
    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Self::Error> {
        fill_bytes_via_next_word(dst, || self.try_next_u64())
    }
}

#[cfg(feature = "rand")]
impl SeedableRng for RapidRand {
    type Seed = [u8; 8];

    #[inline]
    fn from_seed(seed: Self::Seed) -> Self {
        let state = u64::from_le_bytes(seed);
        Self { state }
    }

    #[inline]
    fn seed_from_u64(mut state: u64) -> Self {
        let state = rapidrand(&mut state);
        Self { state }
    }
}

/// A wider, longer-period random number generator built on the 128-bit [`rapidrand128`] construction.
///
/// Like [`RapidRand`] this is a fast, deterministic, non-cryptographic RNG, but it carries a 128-bit
/// Weyl counter (a full `2^128` period) instead of 64 bits. The extra width and its double-multiply
/// avalanche are designed for parallel and distributed workloads: 100% output coverage and stronger
/// stream separation, so distinct seeds behave as independent streams. See [`rapidrand128`] for the
/// construction and the properties it targets.
///
/// With the `rand` feature, this RNG implements [`rand_core::Rng`] and [`rand_core::SeedableRng`]
/// on top of [`rapidrand128`] and is fully compatible with `rand` v0.10.
///
/// # Compatibility
///
/// [`RapidRand128`] is **not** bit-compatible with [`RapidRand`]. Its design properties are checked
/// exhaustively on narrow-width models (`tests/exhaustive.rs`), but broad empirical validation to the
/// volumes [`RapidRand`] has cleared (PractRand / TestU01 / coll-birth) is still in progress — treat it
/// as experimental until that lands.
///
/// # Examples
/// Seed it from `rand`'s thread-local RNG (itself seeded from the OS) with `from_rng`:
///
/// ```rust
/// use rand::{RngExt};   // RngExt brings `.random()`, `.random_range()`, ...
/// use rapidrand::RapidRand128;
///
/// let mut rng: RapidRand128 = rand::make_rng();
///
/// let coin: bool = rng.random();
/// let roll = rng.random_range(1..=6);
/// let value: u32 = rng.random();
/// ```
///
/// For a reproducible stream, seed it from a fixed value instead:
///
/// ```rust
/// use rand::{RngExt, SeedableRng};
/// use rapidrand::RapidRand128;
///
/// let mut rng = RapidRand128::seed_from_u64(42);
/// let value: u32 = rng.random();
/// ```
#[cfg(feature = "rand")]
#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub struct RapidRand128 {
    state: u128,
}

#[cfg(feature = "rand")]
impl TryRng for RapidRand128 {
    type Error = core::convert::Infallible;

    #[inline]
    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        Ok(rapidrand128(&mut self.state) as u32)
    }

    #[inline]
    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok(rapidrand128(&mut self.state))
    }

    #[inline]
    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Self::Error> {
        fill_bytes_via_next_word(dst, || self.try_next_u64())
    }
}

#[cfg(feature = "rand")]
impl SeedableRng for RapidRand128 {
    type Seed = [u8; 16];

    #[inline]
    fn from_seed(seed: Self::Seed) -> Self {
        let state = u128::from_le_bytes(seed);
        Self { state }
    }

    #[inline]
    fn seed_from_u64(seed: u64) -> Self {
        // Avalanche is strong, no seed pre-mixing is required, no weak seeds. To avoid starting
        // on state = 0 (which produces output = 0) we make state = 0 impossible when seeding from
        // a single u64 via u128.wrapping_add(1).
        let state = (seed as u128).wrapping_add(1);
        Self { state }
    }
}
