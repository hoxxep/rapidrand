#![cfg_attr(docsrs, doc = include_str!("../README.md"))]
#![cfg_attr(not(docsrs), doc = "# Rapidrand")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, doc(auto_cfg(hide(docsrs))))]
#![no_std]
#![deny(missing_docs)]
#![deny(unused_must_use)]

#[cfg(feature = "rand")]
use rand_core::{Rng, SeedableRng, TryRng, utils::fill_bytes_via_next_word};

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

/// Generate a pseudorandom number using rapidhash mixing.
///
/// This PRNG is not a cryptographic random number generator.
///
/// This implementation is equivalent in logic and performance to
/// [wyhash::wyrand](https://github.com/wangyi-fudan/wyhash) and
/// [fastrand](https://docs.rs/fastrand/), but uses rapidhash constants/secrets.
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
    *state = state.wrapping_add(RAPID_SECRET_ADD);
    rapid_mix(*state, *state ^ RAPID_SECRET_XOR)
}

/// Generate a pseudorandom number using rapidhash mixing, with a single constant.
///
/// This PRNG is not a cryptographic random number generator.
///
/// Prefer [`rapidrng`]: it is equally fast and avoids the statistical defect described below.
///
/// # Performance
///
/// This uses a single constant instead of two. In hot loops the compiler hoists both constants
/// into registers and the generated inner loop is identical to [`rapidrng`] (instruction-for-
/// instruction on aarch64), so wall-time performance is the same. The single constant only helps
/// at cold call sites — one fewer 64-bit constant to materialise (a 10-byte `movabs` on x86-64,
/// `mov` + 3×`movk` on aarch64) — and frees one register under register pressure.
///
/// This implementation is equivalent in logic and performance to
/// [wyhash::w1rand](https://github.com/wangyi-fudan/wyhash).
///
/// # Statistical weakness: consecutive repeats
///
/// Reusing the increment constant as the xor constant creates exact consecutive repeats. Whenever
/// the post-increment state satisfies `state & RAPID_SECRET_ADD == 0`, the next increment carries
/// nowhere, so `state + RAPID_SECRET_ADD == state ^ RAPID_SECRET_ADD`. The folded multiply is
/// commutative in its arguments, so the *next* output exactly equals the current one.
///
/// The number of repeat states is 2^z where z is the number of zero bits in the constant, doubled
/// if its top bit is set (the wrapping add discards the carry out of bit 63, freeing that state
/// bit). `RAPID_SECRET_ADD` has 32 set bits and a clear top bit, so exactly 2^32 of the 2^64
/// states trigger this: an identical adjacent output pair once per ~2^32 draws on average — a few
/// seconds of continuous generation — where a truly random stream would produce one every 2^64
/// draws. A different constant cannot avoid this: good multiplicative constants have roughly half
/// their bits set, pinning the rate near 2^-32. Upstream `w1rand` shares the defect (33 set bits,
/// top bit set: also exactly 2^32 repeat states). PractRand and BigCrush do not test adjacent-word
/// equality and pass this generator regardless, but a dedicated test detects the bias within
/// seconds, and any use that treats consecutive outputs as unique (e.g. forming 128-bit values
/// from adjacent draws) inherits it.
///
/// [`rapidrng`]'s two-constant pair has zero states satisfying `state + RAPID_SECRET_ADD ==
/// state ^ RAPID_SECRET_XOR` (the required carry pattern is inconsistent), so it never repeats
/// consecutively.
#[inline(always)]
#[must_use]
pub const fn rapidrng_single(state: &mut u64) -> u64 {
    *state = state.wrapping_add(RAPID_SECRET_ADD);
    rapid_mix(*state, *state ^ RAPID_SECRET_ADD)
}

/// A random number generator that uses the rapidhash mixing algorithm.
///
/// This deterministic RNG is optimized for speed and throughput. This is not a cryptographic random
/// number generator.
///
/// With the `rand` feature, this RNG implements [`rand_core::Rng`] and [`rand_core::SeedableRng`]
/// on top of [`rapidrng`] and is fully compatible with [`rand`] v0.10.
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

    fn from_rng<R: Rng + ?Sized>(rng: &mut R) -> Self {
        Self {
            state: rng.next_u64(),
        }
    }

    fn try_from_rng<R: TryRng + ?Sized>(rng: &mut R) -> Result<Self, R::Error> {
        match rng.try_next_u64() {
            Ok(state) => Ok(Self { state }),
            Err(e) => Err(e),
        }
    }
}
