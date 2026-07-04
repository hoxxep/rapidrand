//! Exhaustive full-period output-space tests for the wyrand-family constructions behind `rapidrng`.
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
//! random-function `~63.2%`. `rapidrand` ships the parallel variant as `rapidrng` (and `RapidRng`).
//!
//! ## Why a u16 model proves the u64 claim
//!
//! The `1 - 1/e` and `1 - e^(-1/2)` figures are width-independent limits (already exact to ~5 digits
//! at `N = 2^16`). We reimplement the exact same arithmetic over `u16`, enumerate the entire
//! `2^16`-state period, and measure the real image multiset. [`u64_model_matches_crate`] pins the
//! parallel model to the shipped `rapidrng` at `u64`, so the u16 measurement transfers to the shipped
//! code.

use rapidrand::rapidrng;

/// One draw of a `u16` construction: advance the state in place and return the output.
type Step = fn(&mut u16) -> u16;

/// A narrow unsigned word we can enumerate exhaustively, carrying the same mixing arithmetic as the
/// production `u64` generator. Implemented for `u16` (the exhaustive measurement) and `u64` (the
/// crate-equivalence cross-check) from one macro, so the two widths run *identical* code.
trait Word: Copy + Eq {
    const BITS: u32;
    const SPACE: usize = 1 << Self::BITS;
    /// Starting state for enumeration.
    const ZERO: Self;
    /// Odd Weyl increment (guarantees a full-period cycle), truncated from `RAPID_SECRET_ADD`.
    const ADD: Self;
    /// Non-zero xor secret, truncated from `RAPID_SECRET_XOR`.
    const XOR: Self;

    /// Folded widening multiply `fold(a * b)`, matching `rapid_mix` at this width.
    fn fold_mul(a: Self, b: Self) -> Self;
    fn wadd(self, other: Self) -> Self;
    fn xor(self, other: Self) -> Self;
    fn index(self) -> usize;
}

macro_rules! impl_word {
    ($ty:ty, $wide:ty, $add:expr, $xor:expr) => {
        impl Word for $ty {
            const BITS: u32 = <$ty>::BITS;
            const ZERO: Self = 0;
            const ADD: Self = $add;
            const XOR: Self = $xor;

            #[inline(always)]
            fn fold_mul(a: Self, b: Self) -> Self {
                let r = (a as $wide).wrapping_mul(b as $wide);
                (r as Self) ^ (r >> <$ty>::BITS) as Self
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
            fn index(self) -> usize {
                self as usize
            }
        }
    };
}

// Truncated from the real secrets (ADD forced odd, top bit clear):
//   RAPID_SECRET_ADD = 0x2d358dccaa6c78a5   RAPID_SECRET_XOR = 0x8bb84b93962eacc9
// u16 ADD 0x78a5 has 8 zero bits -> exactly 2^8 = 256 structural `w1rand` repeats.
impl_word!(u16, u32, 0x78a5, 0xacc9);
impl_word!(u64, u128, 0x2d358dccaa6c78a5, 0x8bb84b93962eacc9);

// The four wyrand-family constructions, generic over word width. `u64_model_matches_crate` proves
// the parallel model matches the shipped `rapidrng` line-for-line at `u64`.

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

/// `wyranda_parallel` (reinerp): asymmetric, `fold(new · (old ^ XOR))`. Shipped as `rapidrng`.
#[inline(always)]
fn v_wyranda_parallel<W: Word>(state: &mut W) -> W {
    let old = *state;
    *state = state.wadd(W::ADD);
    W::fold_mul(*state, old.xor(W::XOR))
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
}

/// `1 - 1/e`: coverage of a random function (wyranda chain / parallel).
const RANDOM_COVERAGE: f64 = 0.6321205588285577;
/// `1 - e^(-1/2)`: coverage of a random fixed-point-free 2-to-1 filter (wyrand / w1rand).
const SYMMETRIC_COVERAGE: f64 = 0.3934693402873666;

fn assert_close(what: &str, got: f64, want: f64, tol: f64) {
    assert!((got - want).abs() <= tol, "{what}: got {got:.4}, expected {want:.4} (tol {tol})");
}

/// The symmetric constructions (`wyrand`, `w1rand`) can never reach the `~63%` a good RNG should:
/// `f(x) == f(x ^ XOR)` forces every preimage count even and collapses coverage to `~39.3%`.
#[test]
fn symmetric_variants_undercover() {
    for (name, step) in [("wyrand", v_wyrand::<u16> as Step), ("w1rand", v_w1rand)] {
        let stats = Stats::collect(step);
        assert_eq!(stats.odd_preimage_values(), 0, "{name}: 2-to-1 symmetry should force all counts even");
        assert_close(&format!("{name} coverage"), stats.coverage(), SYMMETRIC_COVERAGE, 0.02);
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
    assert_eq!(Stats::collect::<u16>(v_wyrand).repeats, 0, "wyrand should never repeat consecutively");
}

/// reinerp's wyranda variants break commutativity and recover random-function statistics: ~63%
/// coverage, Poisson(1) preimage counts, odd preimage counts present, and no structural repeats.
#[test]
fn wyranda_variants_recover_random_statistics() {
    let inv_e = std::f64::consts::E.recip();
    let variants =
        [("chain", v_wyranda_chain::<u16> as Step), ("parallel", v_wyranda_parallel)];
    for (name, step) in variants {
        let stats = Stats::collect(step);
        assert_close(&format!("{name} coverage"), stats.coverage(), RANDOM_COVERAGE, 0.02);
        assert!(stats.odd_preimage_values() > 0, "{name}: broken symmetry should produce odd counts");
        assert!(stats.repeats < 16, "{name}: {} repeats is above chance", stats.repeats);
        // Preimage histogram follows Poisson(1): P(k preimages) = e^-1 / k!.
        for (k, want) in [inv_e, inv_e, inv_e / 2.0, inv_e / 6.0].into_iter().enumerate() {
            assert_close(&format!("{name} P(preimages={k})"), stats.fraction_with(k as u32), want, 0.02);
        }
    }
}

/// Pin the parallel model to the real shipped `rapidrng` at `u64`: the arithmetic measured at `u16`
/// is bit-identical to what ships, so the small-width claims transfer.
#[test]
fn u64_model_matches_crate() {
    let seeds = [0u64, 1, 2, 42, u64::MAX, u64::MAX - 1, 0x2d358dccaa6c78a5, 0x8bb84b93962eacc9];
    for seed in seeds {
        let (mut a, mut b) = (seed, seed);
        for _ in 0..64 {
            assert_eq!(v_wyranda_parallel(&mut a), rapidrng(&mut b), "mismatch (seed {seed:#x})");
        }
    }
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
    println!("\n{:<18} {:<11} {:>9} {:>11} {:>8}", "construction", "kind", "coverage", "odd-preimg", "repeats");
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
