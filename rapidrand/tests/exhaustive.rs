//! Exhaustive full-period output-space tests for the `rapidrng` variants.
//!
//! # Why this exists
//!
//! `rapidrng` (like wyrand) is a Weyl sequence run through a non-bijective output filter:
//!
//! ```text
//! state += ADD;              // ADD is odd  =>  bijection: cycles through all 2^n states
//! output = fold_mul(state, state ^ XOR);   // non-bijective mixing of the state
//! ```
//!
//! Because the state increment is a full-period permutation, over one complete period the
//! *input* to the output filter takes every one of the `2^n` values exactly once. The multiset
//! of outputs is therefore precisely the image multiset of the output filter applied to a uniform
//! domain — which is exactly the object a collision test probes.
//!
//! For a *random* function `f: [N] -> [N]` the preimage counts are Poisson(1) distributed, so:
//!
//! * fraction of the output space that is *hit* (>= 1 preimage): `1 - 1/e ≈ 63.2%`
//! * fraction with exactly `k` preimages: `e^-1 / k!`
//!
//! This is the "~63% of the output space" claim from
//! <https://github.com/wangyi-fudan/wyhash/issues/130#issuecomment-4835746792> (reinerp).
//!
//! ## The symmetry defect
//!
//! `fold_mul` is commutative (it folds the 128-bit product `a * b`, and `a*b == b*a`). The main
//! filter `f(x) = fold_mul(x, x ^ XOR)` therefore satisfies
//!
//! ```text
//! f(x ^ XOR) = fold_mul(x ^ XOR, x) = fold_mul(x, x ^ XOR) = f(x)
//! ```
//!
//! `x -> x ^ XOR` is a fixed-point-free involution (XOR != 0), so every output value has an *even*
//! number of preimages and at most `N/2` values can be hit. A random such 2-to-1 filter covers only
//! `1 - e^(-1/2) ≈ 39.3%`. Both [`rapidrng`] and [`rapidrng_single`] carry this defect.
//!
//! reinerp's fix — feed the two sides of the multiply from *different* states so the filter is no
//! longer a commutative function of a single value — is implemented here as [`rapidrng_chain`] and
//! [`rapidrng_parallel`]. Those recover the full `~63.2%` coverage. The default `RapidRng` uses
//! [`rapidrng_parallel`].
//!
//! ## Why a u16 model proves the u64 claim
//!
//! The `1 - 1/e` and `1 - e^(-1/2)` figures are width-independent limits (the finite-`N` correction
//! `(1 - 1/N)^N -> e^-1` is already exact to ~5 digits at `N = 2^16`). We reimplement the exact same
//! arithmetic over `u16`, enumerate the *entire* `2^16`-state period, and measure the real image
//! multiset. The [`u64_model_matches_crate`] test pins the generic model to the production functions,
//! so the u16 measurement transfers to the shipped u64 code.

use rapidrand::{rapidrng, rapidrng_chain, rapidrng_parallel, rapidrng_single};

/// A narrow unsigned word we can enumerate exhaustively, carrying the same mixing arithmetic as the
/// production `u64` generator. Implemented for `u16` (the exhaustive claims) and `u64` (the
/// crate-equivalence cross-check).
trait Word: Copy + Eq + std::fmt::Debug {
    /// Number of state/output bits.
    const BITS: u32;
    /// Size of the state and output space, `2^BITS`.
    const SPACE: usize = 1 << Self::BITS;
    /// Zero, the starting state for enumeration.
    const ZERO: Self;
    /// Odd Weyl increment (guarantees a full-period cycle). Truncated from `RAPID_SECRET_ADD`, with
    /// the top bit kept clear so the `single`-variant repeat count is exactly `2^(zero bits)`.
    const ADD: Self;
    /// Non-zero xor secret. Truncated from `RAPID_SECRET_XOR`.
    const XOR: Self;

    /// Folded widening multiply: `fold(a * b)`, matching `rapid_mix` at this width.
    fn fold_mul(a: Self, b: Self) -> Self;
    fn wadd(self, other: Self) -> Self;
    fn xor(self, other: Self) -> Self;
    /// Index into a `SPACE`-sized tally array.
    fn index(self) -> usize;
}

macro_rules! impl_word {
    ($ty:ty, $wide:ty, $add:expr, $xor:expr) => {
        impl Word for $ty {
            const BITS: u32 = <$ty>::BITS;
            const ZERO: Self = 0;
            const ADD: Self = $add;
            const XOR: Self = $xor;

            #[inline]
            fn fold_mul(a: Self, b: Self) -> Self {
                let r = (a as $wide).wrapping_mul(b as $wide);
                (r as Self) ^ (r >> <$ty>::BITS) as Self
            }
            #[inline]
            fn wadd(self, other: Self) -> Self {
                self.wrapping_add(other)
            }
            #[inline]
            fn xor(self, other: Self) -> Self {
                self ^ other
            }
            #[inline]
            fn index(self) -> usize {
                self as usize
            }
        }
    };
}

// Constants are truncated from the real secrets, with ADD forced odd and top-bit-clear:
//   RAPID_SECRET_ADD = 0x2d358dccaa6c78a5   RAPID_SECRET_XOR = 0x8bb84b93962eacc9
// u16 ADD 0x78a5 = 0b0111_1000_1010_0101 (odd, top clear, 8 zero bits -> 256 `single` repeats)
impl_word!(u16, u32, 0x78a5, 0xacc9);
impl_word!(u64, u128, 0x2d358dccaa6c78a5, 0x8bb84b93962eacc9);

// The four variants, reimplemented generically. These mirror `rapidrand::lib` line-for-line; the
// `u64_model_matches_crate` test proves the equivalence against the production functions.

/// `rapidrng`: symmetric single-state filter `fold(state · (state ^ XOR))`.
fn v_main<W: Word>(state: &mut W) -> W {
    *state = state.wadd(W::ADD);
    W::fold_mul(*state, state.xor(W::XOR))
}

/// `rapidrng_single`: symmetric, reuses `ADD` as the xor secret. Adds the consecutive-repeat defect.
fn v_single<W: Word>(state: &mut W) -> W {
    *state = state.wadd(W::ADD);
    W::fold_mul(*state, state.xor(W::ADD))
}

/// `rapidrng_chain` (reinerp): asymmetric, `fold(old · (new ^ XOR))`.
fn v_chain<W: Word>(state: &mut W) -> W {
    let old = *state;
    *state = state.wadd(W::ADD);
    W::fold_mul(old, state.xor(W::XOR))
}

/// `rapidrng_parallel` (reinerp): asymmetric, `fold(new · (old ^ XOR))`. The default `RapidRng`.
fn v_parallel<W: Word>(state: &mut W) -> W {
    let old = *state;
    *state = state.wadd(W::ADD);
    W::fold_mul(*state, old.xor(W::XOR))
}

/// Statistics gathered by walking one full `2^BITS`-state period.
struct Stats {
    space: usize,
    /// `preimages[v]` = number of times output value `v` was produced over the period.
    preimages: Vec<u32>,
    /// Number of adjacent output pairs that were exactly equal (`out[i] == out[i+1]`).
    adjacent_repeats: usize,
}

impl Stats {
    /// Enumerate the entire period of `step`, starting from `ZERO`.
    fn collect<W: Word>(step: impl Fn(&mut W) -> W) -> Stats {
        let mut preimages = vec![0u32; W::SPACE];
        let mut state = W::ZERO;
        let mut prev: Option<W> = None;
        let mut adjacent_repeats = 0usize;
        for _ in 0..W::SPACE {
            let out = step(&mut state);
            preimages[out.index()] += 1;
            if prev == Some(out) {
                adjacent_repeats += 1;
            }
            prev = Some(out);
        }
        Stats { space: W::SPACE, preimages, adjacent_repeats }
    }

    /// Fraction of the output space with at least one preimage (the "coverage" claim).
    fn coverage(&self) -> f64 {
        let hit = self.preimages.iter().filter(|&&c| c > 0).count();
        hit as f64 / self.space as f64
    }

    /// Fraction of the output space whose preimage count is exactly `k`.
    fn fraction_with(&self, k: u32) -> f64 {
        let n = self.preimages.iter().filter(|&&c| c == k).count();
        n as f64 / self.space as f64
    }

    /// Count of output values reached an *odd* number of times. Must be 0 for the symmetric filters
    /// (their fixed-point-free 2-to-1 symmetry forces every preimage count even).
    fn odd_preimage_values(&self) -> usize {
        self.preimages.iter().filter(|&&c| c % 2 == 1).count()
    }
}

/// `1 - 1/e`: coverage of a random function (the `chain`/`parallel` variants).
const RANDOM_COVERAGE: f64 = 0.6321205588285577;
/// `1 - e^(-1/2)`: coverage of a random fixed-point-free 2-to-1 filter (the `main`/`single`
/// variants, limited by the `f(x) == f(x ^ XOR)` symmetry).
const SYMMETRIC_COVERAGE: f64 = 0.3934693402873666;

fn assert_close(what: &str, got: f64, want: f64, tol: f64) {
    assert!(
        (got - want).abs() <= tol,
        "{what}: got {got:.4}, expected {want:.4} (tol {tol})"
    );
}

// --- The symmetric (defective) variants: rapidrng and rapidrng_single ---

/// The main `rapidrng` filter is symmetric, so it can never reach the `~63%` a good RNG should:
/// its `f(x) == f(x ^ XOR)` symmetry forces every preimage count even and collapses coverage to
/// `~39.3%`.
#[test]
fn main_variant_is_symmetric_and_undercovers() {
    let stats = Stats::collect::<u16>(v_main);
    // Structural: the 2-to-1 symmetry forces every preimage count to be even. Exact, no slack.
    assert_eq!(
        stats.odd_preimage_values(),
        0,
        "rapidrng: symmetry f(x)=f(x^XOR) should make every preimage count even"
    );
    // Coverage collapses to ~39.3%, well under the ~63% a random filter reaches.
    assert_close("rapidrng coverage", stats.coverage(), SYMMETRIC_COVERAGE, 0.02);
}

/// `rapidrng_single` shares the symmetry defect *and* adds exact consecutive repeats: `ADD = 0x78a5`
/// has 8 zero bits, so exactly `2^8 = 256` adjacent output pairs are identical over the period.
#[test]
fn single_variant_is_symmetric_and_repeats() {
    let stats = Stats::collect::<u16>(v_single);
    assert_eq!(stats.odd_preimage_values(), 0, "single should stay symmetric");
    assert_eq!(stats.adjacent_repeats, 1 << u16::ADD.count_zeros(), "single repeat count");
    assert_close("single coverage", stats.coverage(), SYMMETRIC_COVERAGE, 0.02);
}

// --- The asymmetric (fixed) variants: rapidrng_chain and rapidrng_parallel ---

/// reinerp's variants break commutativity and recover random-function statistics: ~63% coverage,
/// Poisson(1) preimage counts, odd preimage counts present, and no exact consecutive repeats.
#[test]
fn reinerp_variants_recover_random_coverage() {
    for (name, step) in
        [("chain", v_chain::<u16> as fn(&mut u16) -> u16), ("parallel", v_parallel::<u16>)]
    {
        let stats = Stats::collect(step);
        assert_close(&format!("{name} coverage"), stats.coverage(), RANDOM_COVERAGE, 0.02);
        // Poisson(1): fraction with exactly one preimage is also 1/e ≈ 0.368.
        assert_close(
            &format!("{name} single-preimage fraction"),
            stats.fraction_with(1),
            std::f64::consts::E.recip(),
            0.03,
        );
        // The symmetry is gone: odd preimage counts now exist (all would be even if symmetric).
        assert!(stats.odd_preimage_values() > 0, "{name}: asymmetric filter should have odd counts");
        // The `single` repeat defect is absent: adjacent repeats stay at random-chance level (~1
        // expected for a random function over 2^16), far below `single`'s structural 256.
        assert!(stats.adjacent_repeats < 16, "{name}: repeats {} above chance", stats.adjacent_repeats);
    }
}

/// The u16 asymmetric preimage histogram should track Poisson(1): `e^-1 / k!`.
#[test]
fn asymmetric_preimage_histogram_is_poisson() {
    let stats = Stats::collect::<u16>(v_parallel);
    let inv_e = std::f64::consts::E.recip();
    // e^-1/k! for k = 0,1,2,3,4.
    let expected = [inv_e, inv_e, inv_e / 2.0, inv_e / 6.0, inv_e / 24.0];
    for (k, &want) in expected.iter().enumerate() {
        assert_close(
            &format!("parallel P(preimages = {k})"),
            stats.fraction_with(k as u32),
            want,
            0.02,
        );
    }
}

/// Pin the generic narrow-word model to the real crate functions at `u64`: the arithmetic tested at
/// `u16` is bit-identical to what ships. If this passes, the small-width claims transfer.
#[test]
fn u64_model_matches_crate() {
    // A spread of seeds including edge cases.
    let seeds = [0u64, 1, 2, 42, u64::MAX, u64::MAX - 1, 0x2d358dccaa6c78a5, 0x8bb84b93962eacc9];
    let cases: &[(&str, fn(&mut u64) -> u64, fn(&mut u64) -> u64)] = &[
        ("main", v_main, rapidrng),
        ("single", v_single, rapidrng_single),
        ("chain", v_chain, rapidrng_chain),
        ("parallel", v_parallel, rapidrng_parallel),
    ];
    for &(name, model, crate_fn) in cases {
        for seed in seeds {
            let (mut a, mut b) = (seed, seed);
            for _ in 0..64 {
                assert_eq!(model(&mut a), crate_fn(&mut b), "{name} mismatch (seed {seed:#x})");
            }
        }
    }
}

/// Human-readable summary of every variant. Run with `--nocapture` to see the table.
#[test]
fn print_coverage_summary() {
    println!("\n{:<10} {:>10} {:>12} {:>10}", "variant", "coverage", "odd-preimg", "repeats");
    let rows: &[(&str, fn(&mut u16) -> u16)] =
        &[("main", v_main), ("single", v_single), ("chain", v_chain), ("parallel", v_parallel)];
    for &(name, step) in rows {
        let s = Stats::collect(step);
        println!(
            "{name:<10} {:>9.2}% {:>12} {:>10}",
            s.coverage() * 100.0,
            s.odd_preimage_values(),
            s.adjacent_repeats
        );
    }
    println!(
        "expected: symmetric(main/single) ~{:.1}%, asymmetric(chain/parallel) ~{:.1}%\n",
        SYMMETRIC_COVERAGE * 100.0,
        RANDOM_COVERAGE * 100.0
    );
}
