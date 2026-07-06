//! Exhaustive full-period output-space tests for the wyrand-family constructions behind `rapidrand`.
//!
//! # Why this exists
//!
//! Every wyrand-family generator is a Weyl counter run through a non-bijective output filter:
//!
//! ```text
//! state += ADD;                            // ADD is odd => full-period cycle over all 2^n states
//! output = fold_mul(state, state ^ XOR);   // non-bijective mixing of the state
//! ```
//!
//! Because the increment is a full-period permutation, over one complete period the *input* to the
//! output filter takes every one of the `2^n` values exactly once. The multiset of outputs is
//! therefore exactly the image multiset of the filter over a uniform domain — the object a collision
//! test probes. For a *random* function `f: [N] -> [N]` the preimage counts are Poisson(1), so:
//!
//! * fraction of the output space *hit* (>= 1 preimage): `1 - 1/e ≈ 63.2%`
//! * fraction with exactly `k` preimages: `e^-1 / k!`
//!
//! This is the "~63% of the output space" claim from
//! <https://github.com/wangyi-fudan/wyhash/issues/130#issuecomment-4835746792> (reinerp).
//!
//! ## The symmetry defect (wyrand / w1rand)
//!
//! `fold_mul` is commutative (it folds the 128-bit product `a*b == b*a`), so the plain wyrand filter
//! `f(x) = fold_mul(x, x ^ XOR)` satisfies `f(x ^ XOR) = f(x)`. Since `x -> x ^ XOR` is a
//! fixed-point-free involution (`XOR != 0`), every output has an *even* number of preimages and at
//! most `N/2` values are reachable. Such a 2-to-1 filter covers only `1 - e^(-1/2) ≈ 39.3%`. Both
//! `wyrand` (two constants) and `w1rand` (one constant) carry this defect. This is the construction
//! shipped by `fastrand`, `nanorand`, and `turborand`.
//!
//! ## reinerp's fix — wyranda (chain / parallel)
//!
//! reinerp's variants draw the two operands from the two states the counter already holds — the old
//! state `state[0]` and the new state `state[1] = state[0] + ADD` — instead of from one state:
//!
//! ```text
//! wyrand:   fold_mul( state[1], state[1] ^ XOR )    // both operands from the SAME state
//! chain:    fold_mul( state[0], state[1] ^ XOR )    // un-xored = old,  xored = new
//! parallel: fold_mul( state[1], state[0] ^ XOR )    // un-xored = new,  xored = old
//! ```
//!
//! `wyrand` feeds both operands from `state[1]`, so they are a single value and its `^ XOR` image;
//! the involution `x -> x ^ XOR` swaps them and leaves the product fixed, forcing the 2-to-1
//! collapse. chain and parallel each draw one operand from `state[0]` and the other from `state[1]`,
//! so that swap no longer maps one operand to the other — the symmetry is broken and both recover the
//! random-function `~63.2%`. `rapidrand` ships the parallel variant as `rapidrand` (and `RapidRand`).
//!
//! ## Why a u16 model proves the u64 claim
//!
//! The `1 - 1/e` and `1 - e^(-1/2)` figures are width-independent limits (already exact to ~5 digits
//! at `N = 2^16`). We reimplement the exact same arithmetic over `u16`, enumerate the entire
//! `2^16`-state period, and measure the real image multiset. [`u64_model_matches_crate`] pins the
//! parallel model to the shipped `rapidrand` at `u64`, so the u16 measurement transfers to the shipped
//! code.
//!
//! # The 128-bit variant (`wyrand128` / [`rapidrand128`])
//!
//! [`rapidrand128`] is a different animal. Its state is a `2n`-bit Weyl counter held as two `n`-bit
//! halves `(hi, lo)`, and it maps that `2n`-bit state down to an `n`-bit output — a `2n -> n` map, not
//! the `n -> n` filter of the single-state variants. It is designed for three properties the 64-bit
//! line cannot offer: a full `2^(2n)` period, **100% output coverage** in an essentially flat
//! distribution, and **stronger stream separation** (distinct seeds behave as independent streams).
//!
//! The analysis builds up in three constructions, each hardening the last:
//!
//! * [`v_wyrand128`] — the **base fold** `fold_mul(lo, hi ^ lo)`. Over the full `2^(2n)` period the pair
//!   `(lo, hi ^ lo)` ranges over *every* `(a, b)` pair exactly once (the counter is full-period and
//!   `(lo, hi) -> (lo, hi ^ lo)` is a bijection), so the output multiset is exactly
//!   `{ fold_mul(a, b) : a, b in [N] }` — `N^2` draws into an `N`-value space, ~`N` hits per value. This
//!   already reaches *every* output value (no wyrand-style coverage collapse, even though it is a 2-to-1
//!   function of the state — see [`wyrand128_output_depends_only_on_the_folded_pair`]), but its histogram
//!   is far from flat: a structural `2x` spike at `0` (every pair with `a == 0` or `b == 0` folds to `0`,
//!   giving `>= 2N - 1` preimages — [`wyrand128_zero_output_is_doubly_represented`]) and a `var/mean` that
//!   actually *worsens* with width.
//! * [`v_wyrand128_addhi`] — adds the high counter half back, flattening the tallest spike but not the
//!   overall variance.
//! * [`v_wyrand128_avalanche`] — the **shipped** [`rapidrand128`]: a second multiply (a rapidhash-style
//!   double-multiply avalanche) that drives the whole histogram to the random-function ideal,
//!   `var/mean ~1` and `peak/mean ~1` at *every* width. That flat, full coverage is what
//!   [`print_rapidrand128_summary_u16`] measures over the `2^32`-state `u16`-halves model.
//!
//! What these full-period image measurements establish is single-stream coverage and uniformity. They do
//! **not** establish cross-stream independence — that is the job of the empirical suites (PractRand
//! interleaving / TestU01), against which the 128-bit variant is not yet fully validated.
//! The paired increment `(ADD:XOR)` is odd (its low bit is `XOR`'s), so the `2n`-bit counter is
//! full-period ([`wyrand128_has_full_period`]), and [`u128_model_matches_crate`] pins the shipped
//! avalanche to [`rapidrand128`] at `n = 64` exactly as [`u64_model_matches_crate`] does for the 64-bit line.

use rapidrand::{rapidrand, rapidrand128};

/// One draw of a `u16` construction: advance the state in place and return the output.
type Step = fn(&mut u16) -> u16;

/// A narrow unsigned word we can enumerate exhaustively, carrying the same mixing arithmetic as the
/// production `u64` generator. Implemented for `u16` (the exhaustive measurement) and `u64` (the
/// crate-equivalence cross-check) from one macro, so the two widths run *identical* code.
trait Word: Copy + Eq {
    type Wide: Copy + Eq;

    const BITS: u32;
    const SPACE: usize = 1 << Self::BITS;
    /// Starting state for enumeration.
    const ZERO: Self;
    /// One (for propagating the carry in the 128-bit model).
    const ONE: Self;
    /// Odd Weyl increment (guarantees a full-period cycle), truncated from `RAPID_SECRET_ADD`.
    const ADD: Self;
    /// Non-zero xor secret, truncated from `RAPID_SECRET_XOR`. Its low bit is set, so the paired
    /// `2·BITS`-wide increment `(ADD:XOR)` used by the 128-bit model is odd (full `2·BITS` period).
    const XOR: Self;

    /// Folded widening multiply `fold(a * b)`, matching `rapid_mix` at this width.
    fn wide_mul(a: Self, b: Self) -> Self::Wide;
    fn fold_mul(a: Self, b: Self) -> Self;
    fn high_bits(a: Self::Wide) -> Self;
    fn low_bits(a: Self::Wide) -> Self;
    fn wadd(self, other: Self) -> Self;
    fn xor(self, other: Self) -> Self;
    /// Wrapping add returning `(sum, carry_out)`, for incrementing the split `(hi:lo)` counter.
    fn carrying_add(self, other: Self) -> (Self, bool);
    fn index(self) -> usize;
}

macro_rules! impl_word {
    ($ty:ty, $wide:ty, $add:expr, $xor:expr) => {
        impl Word for $ty {
            type Wide = $wide;
            const BITS: u32 = <$ty>::BITS;
            const ZERO: Self = 0;
            const ONE: Self = 1;
            const ADD: Self = $add;
            const XOR: Self = $xor;

            #[inline(always)]
            fn wide_mul(a: Self, b: Self) -> Self::Wide {
                a as $wide * b as $wide
            }
            #[inline(always)]
            fn fold_mul(a: Self, b: Self) -> Self {
                let r = Self::wide_mul(a, b);
                Self::low_bits(r) ^ Self::high_bits(r)
            }
            #[inline(always)]
            fn high_bits(a: Self::Wide) -> Self {
                (a >> <$ty>::BITS) as Self
            }
            #[inline(always)]
            fn low_bits(a: Self::Wide) -> Self {
                a as Self
            }
            #[inline(always)]
            fn wadd(self, other: Self) -> Self {
                self.wrapping_add(other)
            }
            #[inline(always)]
            fn xor(self, other: Self) -> Self {
                self ^ other
            }
            #[inline(always)]
            fn carrying_add(self, other: Self) -> (Self, bool) {
                let sum = self.wrapping_add(other);
                (sum, sum < self) // unsigned wraparound => carry out of this half
            }
            #[inline(always)]
            fn index(self) -> usize {
                self as usize
            }
        }
    };
}

// Truncated from the real secrets (ADD forced odd, top bit clear):
//   RAPID_SECRET_ADD = 0x2d358dccaa6c78a5   RAPID_SECRET_XOR = 0x8bb84b93962eacc9
// u16 ADD 0x78a5 has 8 zero bits -> exactly 2^8 = 256 structural `w1rand` repeats.
//
// The `u8` width is only used by the 128-bit `wyrand128` model: its state is two u8 halves, so its
// full period is 2^16 states — the same enumeration budget as the u16 single-state model above. The
// low bit of XOR (0xc9) is set, so the paired 16-bit increment (0xa5c9) is odd and the counter runs
// its full 2^16 period.
impl_word!(u8, u16, 0xa5, 0xc9);
impl_word!(u16, u32, 0x78a5, 0xacc9);
impl_word!(u64, u128, 0x2d358dccaa6c78a5, 0x8bb84b93962eacc9);

// The four wyrand-family constructions, generic over word width. `u64_model_matches_crate` proves
// the parallel model matches the shipped `rapidrand` line-for-line at `u64`.

/// `wyrand`: symmetric single-state filter `fold(state · (state ^ XOR))`.
#[inline(always)]
fn v_wyrand<W: Word>(state: &mut W) -> W {
    *state = state.wadd(W::ADD);
    W::fold_mul(*state, state.xor(W::XOR))
}

/// `w1rand`: symmetric, reuses `ADD` as the xor secret (adds the consecutive-repeat defect).
#[inline(always)]
fn v_w1rand<W: Word>(state: &mut W) -> W {
    *state = state.wadd(W::ADD);
    W::fold_mul(*state, state.xor(W::ADD))
}

/// `wyranda_chain` (reinerp): asymmetric, `fold(old · (new ^ XOR))`.
#[inline(always)]
fn v_wyranda_chain<W: Word>(state: &mut W) -> W {
    let old = *state;
    *state = state.wadd(W::ADD);
    W::fold_mul(old, state.xor(W::XOR))
}

/// `wyranda_parallel` (reinerp): asymmetric, `fold(new · (old ^ XOR))`. Shipped as `rapidrand`.
#[inline(always)]
fn v_wyranda_parallel<W: Word>(state: &mut W) -> W {
    let old = *state;
    *state = state.wadd(W::ADD);
    W::fold_mul(*state, old.xor(W::XOR))
}

/// `wyrand128`: the base folded output of the wider, longer-period construction. The state is a
/// `2·BITS`-wide Weyl counter held as two `W`-halves `(hi, lo)`; the output folds the two halves as
/// `fold(lo · (hi ^ lo))` on the *pre-increment* state. The counter then advances by the odd
/// `2·BITS`-wide constant `(ADD:XOR)` (add `ADD` to `hi`, `XOR` to `lo`, propagating the carry), so it
/// has a full `2^(2·BITS)` period. Unlike the single-state variants this maps a `2·BITS`-bit state to a
/// `BITS`-bit output. The shipped [`rapidrand128`] hardens this base fold with a second multiply (see
/// [`v_wyrand128_avalanche`]); this base output, and the intermediate [`v_wyrand128_addhi`], are kept as
/// references that show what that hardening buys.
#[inline(always)]
fn v_wyrand128<W: Word>(state: &mut (W, W)) -> W {
    let (hi, lo) = *state;
    let out = W::fold_mul(lo, lo.xor(hi));
    let (new_lo, carry) = lo.carrying_add(W::XOR);
    let new_hi = hi.wadd(W::ADD).wadd(if carry { W::ONE } else { W::ZERO });
    *state = (new_hi, new_lo);
    out
}

/// `wyrand128_addhi`: an *intermediate* hardening of the base [`v_wyrand128`] fold that adds the high
/// counter half back into the output, `fold(lo · (hi ^ lo)) + hi`. The folded multiply alone
/// over-represents a few *structured* output values (`0` and the all-ones word most strongly — see
/// [`wyrand128_zero_output_is_doubly_represented`]); adding the full-entropy `hi` word flattens those
/// spikes toward the ideal `N` preimages each. It is *not* what ships — it flattens the single tallest
/// spike but its `var/mean` still worsens with width (see [`print_rapidrand128_summary_u16`]), so the
/// shipped [`rapidrand128`] goes further with the second multiply of [`v_wyrand128_avalanche`]. Kept here
/// as the reference point that motivates that step; [`wyrand128_addhi_flattens_the_output`] measures its
/// improvement over the raw fold across the full `2^16` period.
#[inline(always)]
fn v_wyrand128_addhi<W: Word>(state: &mut (W, W)) -> W {
    let hi = state.0;
    v_wyrand128(state).wadd(hi)
}

/// `wyrand128_avalanche`: the **shipped** [`rapidrand128`] construction, a rapidhash-style double-multiply
/// avalanche. It multiplies the two counter halves, re-multiplies the mixed product halves (xored with
/// `XOR` and `lo`), then folds and adds `hi`. Where the base fold and the intermediate [`v_wyrand128_addhi`]
/// only flatten the tallest spike, the second multiply drives the whole histogram to the random-function
/// ideal — `var/mean ~1` and `peak/mean ~1` at *every* width (see [`print_rapidrand128_summary_u16`]) —
/// which is what earns 100% flat output coverage and the stronger stream separation the 128-bit variant
/// targets. [`u128_model_matches_crate`] pins this arithmetic to the shipped [`rapidrand128`] at `W = u64`.
#[inline(always)]
fn v_wyrand128_avalanche<W: Word>(state: &mut (W, W)) -> W {
    let (hi, lo) = *state;
    let p = W::wide_mul(lo, hi);
    let phi = W::high_bits(p);
    let plo = W::low_bits(p);
    let r = W::wide_mul(plo.xor(W::XOR), phi.xor(lo));
    let out = W::high_bits(r).xor(W::low_bits(r).wadd(hi));
    let (new_lo, carry) = lo.carrying_add(W::XOR);
    let new_hi = hi.wadd(W::ADD).wadd(if carry { W::ONE } else { W::ZERO });
    *state = (new_hi, new_lo);
    out
}

/// Measurements taken by walking one full `2^BITS`-state period.
struct Stats {
    /// `preimages[v]` = how many times output value `v` was produced over the period.
    preimages: Vec<u32>,
    /// Adjacent equal outputs `out[i] == out[i+1]` over the *cyclic* period (the last output is
    /// adjacent to the first, since the generator wraps back to its start).
    repeats: usize,
}

impl Stats {
    fn collect<W: Word>(step: impl Fn(&mut W) -> W) -> Stats {
        let mut preimages = vec![0u32; W::SPACE];
        let mut state = W::ZERO;
        let mut repeats = 0;
        let (mut first, mut prev) = (None, None);
        for _ in 0..W::SPACE {
            let out = step(&mut state);
            preimages[out.index()] += 1;
            if prev == Some(out) {
                repeats += 1;
            }
            first = first.or(Some(out));
            prev = Some(out);
        }
        // Close the cycle: the period wraps, so the final output neighbours the first.
        if prev == first {
            repeats += 1;
        }
        Stats { preimages, repeats }
    }

    /// Walk the full `2^(2·BITS)`-state period of the 128-bit [`v_wyrand128`] construction, recording
    /// the multiset of `BITS`-wide outputs. The state space (`2^(2·BITS)`) is the square of the output
    /// space (`2^BITS`), so unlike [`Stats::collect`] this is a `2·BITS -> BITS` map: every output is
    /// hit ~`2^BITS` times, not ~once.
    fn collect_128<W: Word>(step: impl Fn(&mut (W, W)) -> W) -> Stats {
        let mut preimages = vec![0u32; W::SPACE];
        let mut state = (W::ZERO, W::ZERO);
        let mut repeats = 0;
        let period = W::SPACE * W::SPACE; // 2^(2·BITS)
        let (mut first, mut prev) = (None, None);
        for _ in 0..period {
            let out = step(&mut state);
            preimages[out.index()] += 1;
            if prev == Some(out) {
                repeats += 1;
            }
            first = first.or(Some(out));
            prev = Some(out);
        }
        if prev == first {
            repeats += 1;
        }
        Stats { preimages, repeats }
    }

    /// Fraction of the output space with at least one preimage (the "coverage" claim).
    fn coverage(&self) -> f64 {
        let hit = self.preimages.iter().filter(|&&c| c > 0).count();
        hit as f64 / self.preimages.len() as f64
    }

    /// Fraction of the output space whose preimage count is exactly `k`.
    fn fraction_with(&self, k: u32) -> f64 {
        let n = self.preimages.iter().filter(|&&c| c == k).count();
        n as f64 / self.preimages.len() as f64
    }

    /// Output values reached an *odd* number of times. Zero iff the filter is a fixed-point-free
    /// 2-to-1 map (the symmetric variants); positive once that symmetry is broken.
    fn odd_preimage_values(&self) -> usize {
        self.preimages.iter().filter(|&&c| c % 2 == 1).count()
    }

    /// The largest preimage count — the height of the most over-represented output value, in units of
    /// the mean. `1.0` is perfectly flat; the folded multiply spikes well above it.
    fn peak_over_mean(&self) -> f64 {
        let max = *self.preimages.iter().max().unwrap_or(&0);
        f64::from(max) / self.mean()
    }

    /// Mean preimage count over the whole output space.
    fn mean(&self) -> f64 {
        let total: u64 = self.preimages.iter().map(|&c| u64::from(c)).sum();
        total as f64 / self.preimages.len() as f64
    }

    /// Variance of the preimage counts normalised by the mean. A random function gives `~1` (Poisson);
    /// larger means a lumpier, less uniform output histogram.
    fn variance_over_mean(&self) -> f64 {
        let mean = self.mean();
        let var = self
            .preimages
            .iter()
            .map(|&c| (f64::from(c) - mean).powi(2))
            .sum::<f64>()
            / self.preimages.len() as f64;
        var / mean
    }
}

/// `1 - 1/e`: coverage of a random function (wyranda chain / parallel).
const RANDOM_COVERAGE: f64 = 0.6321205588285577;
/// `1 - e^(-1/2)`: coverage of a random fixed-point-free 2-to-1 filter (wyrand / w1rand).
const SYMMETRIC_COVERAGE: f64 = 0.3934693402873666;

fn assert_close(what: &str, got: f64, want: f64, tol: f64) {
    assert!(
        (got - want).abs() <= tol,
        "{what}: got {got:.4}, expected {want:.4} (tol {tol})"
    );
}

/// The symmetric constructions (`wyrand`, `w1rand`) can never reach the `~63%` a good RNG should:
/// `f(x) == f(x ^ XOR)` forces every preimage count even and collapses coverage to `~39.3%`.
#[test]
fn symmetric_variants_undercover() {
    for (name, step) in [("wyrand", v_wyrand::<u16> as Step), ("w1rand", v_w1rand)] {
        let stats = Stats::collect(step);
        assert_eq!(
            stats.odd_preimage_values(),
            0,
            "{name}: 2-to-1 symmetry should force all counts even"
        );
        assert_close(
            &format!("{name} coverage"),
            stats.coverage(),
            SYMMETRIC_COVERAGE,
            0.02,
        );
    }
}

/// `w1rand` reuses `ADD` as the xor secret, so `out[i] == out[i+1]` whenever the post-increment state
/// shares no bits with `ADD` (`state & ADD == 0`): exactly `2^z` such states, where `z` is the number
/// of zero bits in `ADD`. A handful of coincidental repeats sit on top. The two-constant `wyrand`
/// filter has no such states and never repeats.
#[test]
fn w1rand_variant_has_structural_repeats() {
    let structural = 1usize << u16::ADD.count_zeros();
    assert_eq!(structural, 256, "u16 ADD 0x78a5 has 8 zero bits");

    let w1rand = Stats::collect::<u16>(v_w1rand).repeats;
    assert!(
        (structural..structural + 16).contains(&w1rand),
        "w1rand repeats {w1rand} should be the structural {structural} plus a few coincidental",
    );
    assert_eq!(
        Stats::collect::<u16>(v_wyrand).repeats,
        0,
        "wyrand should never repeat consecutively"
    );
}

/// reinerp's wyranda variants break commutativity and recover random-function statistics: ~63%
/// coverage, Poisson(1) preimage counts, odd preimage counts present, and no structural repeats.
#[test]
fn wyranda_variants_recover_random_statistics() {
    let inv_e = std::f64::consts::E.recip();
    let variants = [
        ("chain", v_wyranda_chain::<u16> as Step),
        ("parallel", v_wyranda_parallel),
    ];
    for (name, step) in variants {
        let stats = Stats::collect(step);
        assert_close(
            &format!("{name} coverage"),
            stats.coverage(),
            RANDOM_COVERAGE,
            0.02,
        );
        assert!(
            stats.odd_preimage_values() > 0,
            "{name}: broken symmetry should produce odd counts"
        );
        assert!(
            stats.repeats < 16,
            "{name}: {} repeats is above chance",
            stats.repeats
        );
        // Preimage histogram follows Poisson(1): P(k preimages) = e^-1 / k!.
        for (k, want) in [inv_e, inv_e, inv_e / 2.0, inv_e / 6.0]
            .into_iter()
            .enumerate()
        {
            assert_close(
                &format!("{name} P(preimages={k})"),
                stats.fraction_with(k as u32),
                want,
                0.02,
            );
        }
    }
}

/// Pin the parallel model to the real shipped `rapidrand` at `u64`: the arithmetic measured at `u16`
/// is bit-identical to what ships, so the small-width claims transfer.
#[test]
fn u64_model_matches_crate() {
    let seeds = [
        0u64,
        1,
        2,
        42,
        u64::MAX,
        u64::MAX - 1,
        0x2d358dccaa6c78a5,
        0x8bb84b93962eacc9,
    ];
    for seed in seeds {
        let (mut a, mut b) = (seed, seed);
        for _ in 0..64 {
            assert_eq!(
                v_wyranda_parallel(&mut a),
                rapidrand(&mut b),
                "mismatch (seed {seed:#x})"
            );
        }
    }
}

/// The 128-bit `wyrand128` counter runs its full `2^(2·BITS)` period: over the u8-halves model the
/// `2^16` states are all distinct and the counter returns to its start only after the last one. This
/// is the odd-increment guarantee — the paired increment `(ADD:XOR)` is odd because `XOR`'s low bit is
/// set — measured exactly, so it transfers to the shipped `2^128` period.
#[test]
fn wyrand128_has_full_period() {
    let period = 1usize << (2 * u8::BITS); // 2^16 states
    let mut state = (u8::ZERO, u8::ZERO);
    let mut seen = std::collections::HashSet::with_capacity(period);
    for _ in 0..period {
        assert!(
            seen.insert(state),
            "state {state:?} revisited before the full period elapsed"
        );
        let _ = v_wyrand128(&mut state);
    }
    assert_eq!(seen.len(), period, "counter did not visit all 2^16 states");
    assert_eq!(
        state,
        (u8::ZERO, u8::ZERO),
        "counter did not return to its start after one period"
    );
}

/// The `2·BITS -> BITS` folded multiply reaches its *entire* output space: every one of the `2^BITS`
/// values appears over the full period, and each appears `N` times on average. Unlike the symmetric
/// single-state variants (which collapse to `~39%`), `wyrand128` has no coverage defect — even though
/// its output is only a 2-to-1 function of the state (see
/// [`wyrand128_output_depends_only_on_the_folded_pair`]), the codomain is far smaller than the state
/// space, so nothing is left unreachable.
#[test]
fn wyrand128_covers_output_space() {
    let stats = Stats::collect_128::<u8>(v_wyrand128);
    let total: u64 = stats.preimages.iter().map(|&c| u64::from(c)).sum();
    let n = stats.preimages.len() as u64;

    assert_eq!(
        total,
        n * n,
        "expected N^2 = {} draws over the full period",
        n * n
    );
    assert_eq!(total / n, n, "mean preimage count should be exactly N");
    assert_close("wyrand128 coverage", stats.coverage(), 1.0, 0.0);
    assert!(
        stats.preimages.iter().all(|&c| c > 0),
        "some output value was never produced"
    );
}

/// The zero output is structurally over-represented: every pair with `a == 0` or `b == 0` folds to
/// `0`, giving `2N - 1` guaranteed preimages — `~2x` the mean `N` at *every* width. At u8 that bound is
/// tight (`511`): a non-zero pair could only fold to `0` if its product had equal high and low bytes,
/// i.e. were a multiple of `257`, which is prime and `> u8::MAX`, so no non-zero pair qualifies.
#[test]
fn wyrand128_zero_output_is_doubly_represented() {
    let stats = Stats::collect_128::<u8>(v_wyrand128);
    let n = stats.preimages.len() as u32;
    let zeros = stats.preimages[0];

    assert!(
        zeros >= 2 * n - 1,
        "zero should collect at least the 2N-1 zero-product pairs"
    );
    assert_eq!(
        zeros,
        2 * n - 1,
        "at u8 exactly the zero-product pairs fold to 0 (257 is prime)"
    );
    let ratio = f64::from(zeros) / f64::from(n);
    assert!(
        (1.9..=2.1).contains(&ratio),
        "zero should be ~2x over-represented, got {ratio:.3}x"
    );
}

/// `wyrand128`'s output is a commutative fold of `lo` and `hi ^ lo`, so it depends only on the
/// unordered pair `{lo, hi ^ lo}`. Swapping which half is which — state `(hi, lo)` vs `(hi, hi ^ lo)` —
/// leaves the output unchanged, making the state -> output map exactly 2-to-1 off the `hi == 0`
/// diagonal. This is the 128-bit analogue of the `fold_mul` commutativity symmetry that halves the
/// 64-bit variants; here it costs no coverage (see [`wyrand128_covers_output_space`]). Verified over
/// every one of the `2^16` states.
#[test]
fn wyrand128_output_depends_only_on_the_folded_pair() {
    let out = |hi: u8, lo: u8| -> u8 {
        let mut s = (hi, lo);
        v_wyrand128(&mut s)
    };
    let mut off_diagonal = 0u32;
    for hi in 0..=u8::MAX {
        for lo in 0..=u8::MAX {
            assert_eq!(
                out(hi, lo),
                out(hi, hi ^ lo),
                "symmetry broken at (hi {hi}, lo {lo})"
            );
            if hi != 0 {
                off_diagonal += 1;
            }
        }
    }
    // Off the hi==0 diagonal the two colliding states are distinct, so the map really is 2-to-1 there.
    assert_eq!(
        off_diagonal,
        (1 << 16) - (1 << 8),
        "expected 2^16 - 2^8 off-diagonal states"
    );
}

/// Pin the `wyrand128_avalanche` model to the real shipped [`rapidrand128`] at the full `u64` half-width:
/// the arithmetic enumerated at u8/u16 is bit-identical to what ships (the crate's double-multiply
/// avalanche of the two halves), so the small-width claims transfer. The model's `(hi, lo)` halves must
/// track the crate's `u128` state step for step.
#[test]
fn u128_model_matches_crate() {
    let seeds: [u128; 8] = [
        0,
        1,
        2,
        42,
        u128::MAX,
        u128::MAX - 1,
        0x2d358dccaa6c78a5_8bb84b93962eacc9,
        (0x8bb84b93962eacc9u128 << 64) | 0x2d358dccaa6c78a5,
    ];
    for seed in seeds {
        let mut model = ((seed >> 64) as u64, seed as u64); // (hi, lo)
        let mut crate_state = seed;
        for _ in 0..64 {
            assert_eq!(
                v_wyrand128_avalanche(&mut model),
                rapidrand128(&mut crate_state),
                "output mismatch (seed {seed:#x})",
            );
            assert_eq!(
                model,
                ((crate_state >> 64) as u64, crate_state as u64),
                "state mismatch (seed {seed:#x})",
            );
        }
    }
}

/// The intermediate `wyrand128_addhi` construction flattens the folded multiply's structural output
/// spikes. Over the full `2^16` period it keeps complete coverage but pulls the tallest spike from
/// `~5.8x` the mean down below `~1.5x`, shrinks the normalised variance several-fold, and drags the
/// over-represented `0` value back toward the mean — all for one extra `add`. This is the halfway step
/// toward the shipped [`v_wyrand128_avalanche`]: `+ hi` alone tames the tallest spike but not the overall
/// variance (which still grows with width — see [`print_rapidrand128_summary_u16`]), which is why the
/// shipped variant adds the second multiply. (The remaining spread is still a u8 artifact; the point here
/// is the *relative* improvement over [`v_wyrand128`], which is robust across widths.)
#[test]
fn wyrand128_addhi_flattens_the_output() {
    let base = Stats::collect_128::<u8>(v_wyrand128);
    let addhi = Stats::collect_128::<u8>(v_wyrand128_addhi);
    let n = addhi.preimages.len() as u32;

    // Still reaches every output value.
    assert_close("addhi coverage", addhi.coverage(), 1.0, 0.0);

    // The tallest spike collapses: base towers >4x over the mean, addhi sits below 2x.
    assert!(
        base.peak_over_mean() > 4.0,
        "base peak {:.2}x should tower over the mean",
        base.peak_over_mean()
    );
    assert!(
        addhi.peak_over_mean() < 2.0,
        "addhi peak {:.2}x should be near-flat",
        addhi.peak_over_mean()
    );

    // The whole histogram is several-fold flatter.
    assert!(
        base.variance_over_mean() > 3.0 * addhi.variance_over_mean(),
        "addhi var/mean {:.2} should be several-fold below base {:.2}",
        addhi.variance_over_mean(),
        base.variance_over_mean(),
    );

    // The structural 0 spike is pulled toward the ideal mean (though not all the way to 1x).
    let base_zero_excess = (f64::from(base.preimages[0]) / f64::from(n) - 1.0).abs();
    let addhi_zero_excess = (f64::from(addhi.preimages[0]) / f64::from(n) - 1.0).abs();
    assert!(
        addhi_zero_excess < base_zero_excess,
        "addhi zero excess {addhi_zero_excess:.3} should be below base {base_zero_excess:.3}",
    );
}

/// Human-readable summary over the full `2^16` period (u8 halves) comparing the three 128-bit
/// constructions — the base fold, the intermediate `+ hi`, and the shipped `avalanche`. Run with
/// `--nocapture`. The base fold spikes hardest at a few *structured* values — `0` (the `~2x` zero-product
/// bias) and the all-ones word — which `+ hi` partly tames and the shipped double-multiply `avalanche`
/// flattens to the ideal `N` preimages per value (`var/mean` and `peak/mean` both `~1`). See
/// [`print_rapidrand128_summary_u16`] for the higher-resolution `u16`-halves rerun.
#[test]
fn print_rapidrand128_summary() {
    let row = |name: &str, s: &Stats| {
        let max = *s.preimages.iter().max().unwrap();
        let min = *s.preimages.iter().min().unwrap();
        println!(
            "  {name:<16} {:>7.2}% {:>7.0} {:>6}/{:<6} {:>7} {:>9.1} {:>8.2}x",
            s.coverage() * 100.0,
            s.mean(),
            min,
            max,
            s.preimages[0],
            s.variance_over_mean(),
            s.peak_over_mean(),
        );
    };

    let base = Stats::collect_128::<u8>(v_wyrand128);
    let addhi = Stats::collect_128::<u8>(v_wyrand128_addhi);
    let avalanche = Stats::collect_128::<u8>(v_wyrand128_avalanche);
    println!("\n128-bit variants over the full 2^16 period (u8 halves), N=256 output values:");
    println!(
        "  {:<16} {:>8} {:>7} {:>13} {:>7} {:>9} {:>9}",
        "variant", "coverage", "mean", "min/max", "count(0)", "var/mean", "peak/mean",
    );
    row("wyrand128", &base);
    row("+ hi", &addhi);
    row("avalanche", &avalanche);
    println!("  (ideal random-function map: var/mean ~ 1, peak/mean ~ 1, count(0) ~ mean 256)\n");
}

/// Wider `u16`-halves (`u32 -> u16`) rerun of [`print_rapidrand128_summary`], walking the full `2^32`
/// period so each of the `2^16` output values collects `~2^16` preimages. At u8 the mean count is only
/// `256`, so Poisson sampling noise (`~1/sqrt(256) ≈ 6%`) blurs the histogram; at u16 the mean is `65536`
/// (noise `~0.4%`), so the numbers below reflect the *construction's* real lumpiness rather than sampling
/// jitter. `var/mean` is width-*independent* for a true random function (Poisson(N) has `var/mean == 1` at
/// every N), which makes it the discriminating column here:
///
/// * the raw fold's `var/mean` climbs from `~35` at u8 to `~550` at u16 and its `peak/mean` from `5.8x` to
///   `11.6x` — its non-uniformity is intrinsic and *grows* with width, not a u8 artifact that washes out;
/// * the intermediate `+ hi` fold flattens the single tallest spike (`peak/mean 1.44x -> 1.17x`) but its
///   `var/mean` still rises (`~3.7 -> ~6.3`), so `+ hi` alone does not converge to a random function;
/// * the shipped avalanche (see [`v_wyrand128_avalanche`], pinned to [`rapidrand128`] by
///   [`u128_model_matches_crate`]) sits at `var/mean ~1` at *both* widths (`0.9 -> 1.01`) with `peak/mean`
///   tightening toward `1` (`1.19x -> 1.02x`) and the zero-bias gone (`count(0) ≈ mean`) — the
///   width-independent random-function signature the other two lack.
///
/// So the double multiply is what actually earns the module-doc claim that the fold's shape approaches a
/// random function toward `n = 64`; the raw fold and `+ hi` do not. Final cross-stream independence is
/// still the empirical suites' job (PractRand / TestU01), not settled by this single-stream marginal.
///
/// Single-threaded it walks `~4.3e9` steps into an L2-resident 256 KB histogram (~15s in `--release`,
/// minutes in debug), so it is `#[ignore]`d. Run with:
///     cargo test --release -p rapidrand --test exhaustive -- --ignored --nocapture print_rapidrand128_summary_u16
#[test]
#[ignore = "walks the full 2^32 period; run under --release, takes several seconds"]
fn print_rapidrand128_summary_u16() {
    let row = |name: &str, s: &Stats| {
        let max = *s.preimages.iter().max().unwrap();
        let min = *s.preimages.iter().min().unwrap();
        println!(
            "  {name:<16} {:>7.2}% {:>9.0} {:>8}/{:<8} {:>9} {:>9.3} {:>8.2}x",
            s.coverage() * 100.0,
            s.mean(),
            min,
            max,
            s.preimages[0],
            s.variance_over_mean(),
            s.peak_over_mean(),
        );
    };

    let base = Stats::collect_128::<u16>(v_wyrand128);
    let addhi = Stats::collect_128::<u16>(v_wyrand128_addhi);
    let avalanche = Stats::collect_128::<u16>(v_wyrand128_avalanche);
    println!("\n128-bit variants over the full 2^32 period (u16 halves), N=65536 output values:");
    println!(
        "  {:<16} {:>8} {:>9} {:>17} {:>9} {:>9} {:>9}",
        "variant", "coverage", "mean", "min/max", "count(0)", "var/mean", "peak/mean",
    );
    row("wyrand128", &base);
    row("+ hi", &addhi);
    row("avalanche", &avalanche);
    println!("  (ideal random-function map: var/mean ~ 1, peak/mean ~ 1, count(0) ~ mean 65536)\n");
}

/// Human-readable summary of every construction over the full `2^16` period. Run with `--nocapture`.
#[test]
fn print_coverage_summary() {
    // What each column means, measured over one full period (every state visited exactly once):
    //   coverage    % of the 2^16 output values produced at least once.
    //               random function -> 1-1/e = 63.2%;  symmetric 2-to-1 filter -> 1-e^-1/2 = 39.3%
    //   odd-preimg  output values produced an ODD number of times.
    //               0 proves the filter is 2-to-1 (f(x)=f(x^XOR)); >0 proves that symmetry is broken.
    //   repeats     adjacent equal outputs out[i]==out[i+1] over the cyclic period (incl. last->first).
    //               'w1rand' has a structural 2^z=256 (z = zero bits of ADD); others only chance.
    let rows: &[(&str, &str, Step)] = &[
        ("wyrand", "symmetric", v_wyrand),
        ("w1rand", "symmetric", v_w1rand),
        ("wyranda_chain", "asymmetric", v_wyranda_chain),
        ("wyranda_parallel", "asymmetric", v_wyranda_parallel),
    ];
    println!(
        "\n{:<18} {:<11} {:>9} {:>11} {:>8}",
        "construction", "kind", "coverage", "odd-preimg", "repeats"
    );
    for &(name, kind, step) in rows {
        let s = Stats::collect(step);
        println!(
            "{name:<18} {kind:<11} {:>8.2}% {:>11} {:>8}",
            s.coverage() * 100.0,
            s.odd_preimage_values(),
            s.repeats,
        );
    }
    println!(
        "expected coverage: symmetric ~{:.1}% (1-e^-1/2), asymmetric ~{:.1}% (1-1/e)\n",
        SYMMETRIC_COVERAGE * 100.0,
        RANDOM_COVERAGE * 100.0,
    );
}
