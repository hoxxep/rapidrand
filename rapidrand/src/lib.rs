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

/// Folded 64-bit multiply: compute the 128-bit product `a * b` and XOR its high and low 64-bit
/// halves together.
///
/// Vendored from the `rapidhash` crate so that `rapidrand` stays a zero-dependency crate. Keep this
/// bit-for-bit in sync with rapidhash: <https://github.com/hoxxep/rapidhash>.
#[inline(always)]
#[must_use]
const fn rapid_mix(a: u64, b: u64) -> u64 {
    let r = (a as u128).wrapping_mul(b as u128);
    (r as u64) ^ (r >> 64) as u64
}

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
    rapid_mix(*state, old_state ^ RAPID_SECRET_XOR)
}

/// Generate a pseudorandom `u64` and advance a 128-bit `state`.
///
/// A wider, longer-period sibling of [`rapidrand`]: it runs a 128-bit Weyl counter and folds its two
/// 64-bit halves through the same [`rapid_mix`] output filter, so the output depends on two counter
/// streams and the counter has a full `2^128` period.
///
/// The counter is incremented by the odd 128-bit constant `(RAPID_SECRET_ADD:RAPID_SECRET_XOR)`, and
/// the product is `rapid_mix(lo, hi ^ lo)` on the low and high halves `hi:lo` of the *pre-increment*
/// state. We then add `hi` back into the output to disperse most of the bias towards `0` and
/// `u64::MAX` output values that `rapid_mix` has, which would otherwise be
/// [over-represented](https://github.com/wangyi-fudan/wyhash/issues/156#issuecomment-4888162118)
/// 4x and 93x times respectively in the output stream.
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
    // 128-bit odd increment (RAPID_SECRET_ADD:RAPID_SECRET_XOR) => full 2^128 period.
    *state = state.wrapping_add(((RAPID_SECRET_ADD as u128) << 64) | RAPID_SECRET_XOR as u128);
    rapid_mix(lo, hi ^ lo).wrapping_add(hi)
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
        Self {
            state: u64::from_le_bytes(seed),
        }
    }

    #[inline]
    fn seed_from_u64(mut state: u64) -> Self {
        Self {
            state: rapidrand(&mut state),
        }
    }
}

/// A wider, longer-period random number generator built on the 128-bit [`rapidrand128`] construction.
///
/// Like [`RapidRand`] this is a fast, deterministic, non-cryptographic RNG, but it carries a 128-bit
/// Weyl counter (a full `2^128` period) instead of 64 bits.
///
/// With the `rand` feature, this RNG implements [`rand_core::Rng`] and [`rand_core::SeedableRng`]
/// on top of [`rapidrand128`] and is fully compatible with `rand` v0.10.
///
/// # Compatibility
///
/// [`RapidRand128`] is **not** bit-compatible with [`RapidRand`], and unlike the 64-bit generator it
/// has not yet been validated against the statistical test suites — treat it as experimental until it
/// is.
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
        Self {
            state: u128::from_le_bytes(seed),
        }
    }

    #[inline]
    fn seed_from_u64(mut seed: u64) -> Self {
        // Expand the seed through the 64-bit generator so both halves of the counter start
        // well-mixed and nearby seeds diverge.
        let mut s = rapidrand(&mut seed);
        let lo = rapidrand(&mut s);
        let hi = rapidrand(&mut s);
        Self {
            state: ((hi as u128) << 64) | lo as u128,
        }
    }
}
