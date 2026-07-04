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
/// `rapidrng` uses [rapidhash](https://github.com/hoxxep/rapidhash) secrets with the **wyranda**
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
/// use rapidrand::rapidrng;
///
/// let mut state: u64 = 42;
/// let value: u64 = rapidrng(&mut state);
/// ```
#[inline(always)]
#[must_use]
pub const fn rapidrng(state: &mut u64) -> u64 {
    let old_state = *state;
    *state = state.wrapping_add(RAPID_SECRET_ADD);
    rapid_mix(*state, old_state ^ RAPID_SECRET_XOR)
}

/// A random number generator that uses the rapidhash mixing algorithm.
///
/// This deterministic RNG is optimized for speed and throughput. This is not a cryptographic random
/// number generator.
///
/// With the `rand` feature, this RNG implements [`rand_core::Rng`] and [`rand_core::SeedableRng`]
/// on top of [`rapidrng`] and is fully compatible with `rand` v0.10.
///
/// # Examples
/// Seed it from `rand`'s thread-local RNG (itself seeded from the OS) with `from_rng`:
///
/// ```rust
/// use rand::{RngExt};   // RngExt brings `.random()`, `.random_range()`, ...
/// use rapidrand::RapidRng;
///
/// let mut rng: RapidRng = rand::make_rng();
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
/// use rapidrand::RapidRng;
///
/// let mut rng = RapidRng::seed_from_u64(42);
/// let value: u32 = rng.random();
/// ```
#[cfg(feature = "rand")]
#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub struct RapidRng {
    state: u64,
}

#[cfg(feature = "rand")]
impl TryRng for RapidRng {
    type Error = core::convert::Infallible;

    #[inline]
    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        Ok(rapidrng(&mut self.state) as u32)
    }

    #[inline]
    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok(rapidrng(&mut self.state))
    }

    #[inline]
    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Self::Error> {
        fill_bytes_via_next_word(dst, || self.try_next_u64())
    }
}

#[cfg(feature = "rand")]
impl SeedableRng for RapidRng {
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
            state: rapidrng(&mut state),
        }
    }
}
