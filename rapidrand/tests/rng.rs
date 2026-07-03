// This `#![no_std]` crate opts out of `std`, so bring it back in for the tests, which use
// `std::collections::HashSet` / `std::vec::Vec`. The test harness always links `std`.
extern crate std;

#[cfg(feature = "rand")]
use rand_core::{Rng, SeedableRng};
use rapidrand::{rapidrng, RapidRng};

#[cfg(feature = "rand")]
#[test]
fn test_rapidrng() {
    let mut rng = RapidRng::seed_from_u64(0);
    let x = rng.next_u64();
    let y = rng.next_u64();
    assert_ne!(x, 0);
    assert_ne!(x, y);
}

#[test]
fn bit_flip_trial_fast() {
    let cycles = 100_000;
    let mut seen = std::collections::HashSet::with_capacity(cycles);
    let mut flips = std::vec::Vec::with_capacity(cycles);

    let mut prev = 0;
    for _ in 0..cycles {
        let next = rapidrng(&mut prev);

        let xor = prev ^ next;
        let flipped = xor.count_ones() as u64;
        assert!(
            xor.count_ones() >= 10,
            "Flipping bit changed only {} bits",
            flipped
        );
        flips.push(flipped);

        assert!(!seen.contains(&next), "rapidrng produced a duplicate value");
        seen.insert(next);

        prev = next;
    }

    let average = flips.iter().sum::<u64>() as f64 / flips.len() as f64;
    assert!(
        average > 31.95 && average < 32.05,
        "Did not flip an average of half the bits. average: {}, expected: 32.0",
        average
    );
}

#[cfg(feature = "rand")]
#[test]
fn bit_flip_trial() {
    use rand_core::Rng;

    let cycles = 100_000;
    let mut seen = std::collections::HashSet::with_capacity(cycles);
    let mut flips = std::vec::Vec::with_capacity(cycles);
    let mut rng = RapidRng::seed_from_u64(0);

    let mut prev = 0;
    for _ in 0..cycles {
        let next = rng.next_u64();

        let xor = prev ^ next;
        let flipped = xor.count_ones() as u64;
        assert!(
            xor.count_ones() >= 10,
            "Flipping bit changed only {} bits",
            flipped
        );
        flips.push(flipped);

        assert!(!seen.contains(&next), "RapidRng produced a duplicate value");
        seen.insert(next);

        prev = next;
    }

    let average = flips.iter().sum::<u64>() as f64 / flips.len() as f64;
    assert!(
        average > 31.95 && average < 32.05,
        "Did not flip an average of half the bits. average: {}, expected: 32.0",
        average
    );
}

#[cfg(feature = "rand")]
#[test]
fn test_seedable() {
    // Same seed produces the same stream.
    let mut base = RapidRng::seed_from_u64(0x1);
    let mut same = RapidRng::seed_from_u64(0x1);
    assert_eq!(base.next_u64(), same.next_u64());
    assert_eq!(base.next_u64(), same.next_u64());
    assert_eq!(base.next_u64(), same.next_u64());

    // Different seeds produce different streams.
    let mut base = RapidRng::seed_from_u64(0x1);
    let mut diff = RapidRng::seed_from_u64(0x2);
    assert_ne!(base.next_u64(), diff.next_u64());
}
