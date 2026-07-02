//! `rapidrand-cat` — stream raw pseudo-random bytes from `rapidrand` (or a competitor RNG) to
//! stdout, for piping into statistical test suites like PractRand and TestU01/BigCrush.
//!
//! ```text
//! cargo run --release -p rapidrand-cat -- --rng rapidrand | RNG_test stdin64
//! ```
//!
//! Output is a stream of little-endian `u64` words. PractRand's `stdin64` mode consumes this
//! directly; the TestU01 `testu01_stdin` stub reads it 32 bits at a time. See `justfile` and
//! `TESTING.md` for the full harness.
//!
//! ## Modes
//! * `--mode stream` (default): a single generator seeded with `--seed`, emitted forever.
//! * `--mode interleave`: `--streams N` generators seeded `seed, seed+1, …, seed+N-1`, emitting one
//!   word each per round in round-robin order. This is the inter-stream / adjacent-seed correlation
//!   test — the failure mode that matters most for counter-based mixers like rapidrand, and one that
//!   a single-stream test never exercises. The suites just see bytes; the interleaving lives here.
//!
//! ## Transforms
//! * `--bit-reverse`: reverse the bits of every emitted word. TestU01 (and to a lesser extent
//!   PractRand) is more sensitive to low bits than high bits, so a bit-reversed pass is a distinct,
//!   worthwhile second run rather than a redundant one.

use std::io::{self, Write};
use std::process::ExitCode;

use nanorand::Rng as _;
use rand_core::{Rng as _, SeedableRng};

use rapidrand::{RapidRng, rapidrng};

/// A word source: repeatedly called to produce the next `u64`.
type Gen = Box<dyn FnMut() -> u64>;

/// Default seed. Matches the benchmark seed so streams line up with `rapidrand-bench`.
const DEFAULT_SEED: u64 = 0x1234_5678_9abc_def0;

/// Every generator `rapidrand-cat` can stream, with any accepted aliases. Keep in sync with
/// `rapidrand-bench/benches/rng.rs` and the `rngs` list in the `justfile`.
const RNGS: &[(&str, &[&str])] = &[
    ("rapidrand", &[]),
    ("rapidrand_raw", &["raw"]),
    ("fastrand", &[]),
    ("turborand", &[]),
    ("nanorand_wyrand", &["nanorand"]),
    ("rand_small", &["small"]),
    ("rand_std", &["std"]),
    ("pcg32", &[]),
    ("pcg64", &[]),
    ("xoshiro256++", &["xoshiro256pp"]),
    ("xoshiro256**", &["xoshiro256ss"]),
    ("chacha8", &[]),
    ("chacha20", &[]),
];

/// Resolve a name or alias to its canonical RNG name.
fn canonical(name: &str) -> Option<&'static str> {
    RNGS.iter()
        .find(|(canon, aliases)| *canon == name || aliases.contains(&name))
        .map(|(canon, _)| *canon)
}

/// Build a fresh generator for `rng` seeded with `seed`.
fn make_gen(rng: &str, seed: u64) -> Gen {
    match rng {
        "rapidrand" => {
            let mut r = RapidRng::seed_from_u64(seed);
            Box::new(move || r.next_u64())
        }
        "rapidrand_raw" => {
            let mut s = seed;
            Box::new(move || rapidrng(&mut s))
        }
        "fastrand" => {
            let mut r = fastrand::Rng::with_seed(seed);
            Box::new(move || r.u64(..))
        }
        "turborand" => {
            use turborand::prelude::*;
            let r = Rng::with_seed(seed);
            Box::new(move || r.gen_u64())
        }
        "nanorand_wyrand" => {
            let mut r = nanorand::WyRand::new_seed(seed);
            Box::new(move || r.generate::<u64>())
        }
        "rand_small" => {
            let mut r = rand::rngs::SmallRng::seed_from_u64(seed);
            Box::new(move || r.next_u64())
        }
        "rand_std" => {
            let mut r = rand::rngs::StdRng::seed_from_u64(seed);
            Box::new(move || r.next_u64())
        }
        "pcg32" => {
            let mut r = rand_pcg::Pcg32::seed_from_u64(seed);
            Box::new(move || r.next_u64())
        }
        "pcg64" => {
            let mut r = rand_pcg::Pcg64::seed_from_u64(seed);
            Box::new(move || r.next_u64())
        }
        "xoshiro256++" => {
            let mut r = rand_xoshiro::Xoshiro256PlusPlus::seed_from_u64(seed);
            Box::new(move || r.next_u64())
        }
        "xoshiro256**" => {
            let mut r = rand_xoshiro::Xoshiro256StarStar::seed_from_u64(seed);
            Box::new(move || r.next_u64())
        }
        "chacha8" => {
            let mut r = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
            Box::new(move || r.next_u64())
        }
        "chacha20" => {
            let mut r = rand_chacha::ChaCha20Rng::seed_from_u64(seed);
            Box::new(move || r.next_u64())
        }
        other => unreachable!("unresolved rng {other:?}"),
    }
}

/// Parsed command line.
struct Args {
    rng: &'static str,
    seed: u64,
    interleave: bool,
    streams: u64,
    bit_reverse: bool,
    /// Number of words to emit, or `None` for unbounded.
    limit: Option<u64>,
}

fn print_usage() {
    eprintln!(
        "rapidrand-cat — stream raw RNG bytes (little-endian u64) to stdout\n\
         \n\
         USAGE:\n\
         \x20   rapidrand-cat --rng <name> [--seed <u64>] [--mode stream|interleave]\n\
         \x20                 [--streams <n>] [--bit-reverse] [--limit <words>]\n\
         \n\
         OPTIONS:\n\
         \x20   --rng <name>       generator to stream (or `list` to print names)\n\
         \x20   --seed <u64>       seed, decimal or 0x-hex (default 0x1234...def0)\n\
         \x20   --mode <mode>      `stream` (default) or `interleave`\n\
         \x20   --streams <n>      generators to interleave in interleave mode (default 256)\n\
         \x20   --bit-reverse      reverse the bits of every word (high-bit pass)\n\
         \x20   --limit <words>    stop after N u64 words (default: unbounded)\n"
    );
}

/// Parse `u64` accepting `0x`/`0X` hex or plain decimal.
fn parse_u64(s: &str) -> Result<u64, String> {
    let parsed = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .map(|hex| u64::from_str_radix(hex, 16))
        .unwrap_or_else(|| s.parse::<u64>());
    parsed.map_err(|e| format!("invalid number {s:?}: {e}"))
}

fn parse_args() -> Result<Args, String> {
    let mut rng: Option<&'static str> = None;
    let mut seed = DEFAULT_SEED;
    let mut interleave = false;
    let mut streams = 256u64;
    let mut bit_reverse = false;
    let mut limit = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut next = || args.next().ok_or_else(|| format!("{arg} requires a value"));
        match arg.as_str() {
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            "--rng" => {
                let name = next()?;
                if name == "list" {
                    for (canon, _) in RNGS {
                        println!("{canon}");
                    }
                    std::process::exit(0);
                }
                rng = Some(
                    canonical(&name)
                        .ok_or_else(|| format!("unknown rng {name:?} (try `--rng list`)"))?,
                );
            }
            "--seed" => seed = parse_u64(&next()?)?,
            "--mode" => {
                interleave = match next()?.as_str() {
                    "stream" => false,
                    "interleave" => true,
                    other => return Err(format!("unknown mode {other:?}")),
                }
            }
            "--streams" => streams = parse_u64(&next()?)?,
            "--bit-reverse" => bit_reverse = true,
            "--limit" => limit = Some(parse_u64(&next()?)?),
            other => return Err(format!("unexpected argument {other:?}")),
        }
    }

    let rng = rng.ok_or("missing --rng <name> (try `--rng list`)")?;
    if streams == 0 {
        return Err("--streams must be at least 1".into());
    }
    Ok(Args {
        rng,
        seed,
        interleave,
        streams,
        bit_reverse,
        limit,
    })
}

/// Write words from `next_word` until the limit is hit or the pipe closes.
///
/// Returns `Ok(())` on a clean stop (limit reached or reader hung up), `Err` on a real I/O error.
fn pump(
    mut next_word: impl FnMut() -> u64,
    bit_reverse: bool,
    limit: Option<u64>,
) -> io::Result<()> {
    let stdout = io::stdout();
    let mut out = io::BufWriter::with_capacity(1 << 20, stdout.lock());

    let mut remaining = limit;
    loop {
        if let Some(0) = remaining {
            break;
        }
        let mut word = next_word();
        if bit_reverse {
            word = word.reverse_bits();
        }
        if let Err(e) = out.write_all(&word.to_le_bytes()) {
            // A closed reader (PractRand/TestU01 stopping) surfaces as BrokenPipe — a clean stop.
            return if e.kind() == io::ErrorKind::BrokenPipe {
                Ok(())
            } else {
                Err(e)
            };
        }
        remaining = remaining.map(|r| r - 1);
    }
    match out.flush() {
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        other => other,
    }
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("rapidrand-cat: {e}");
            return ExitCode::from(2);
        }
    };

    let result = if args.interleave {
        // One word from each generator per round, round-robin over adjacent seeds.
        let mut gens: Vec<Gen> = (0..args.streams)
            .map(|i| make_gen(args.rng, args.seed.wrapping_add(i)))
            .collect();
        let mut cursor = 0usize;
        pump(
            move || {
                let word = gens[cursor]();
                cursor = (cursor + 1) % gens.len();
                word
            },
            args.bit_reverse,
            args.limit,
        )
    } else {
        pump(make_gen(args.rng, args.seed), args.bit_reverse, args.limit)
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("rapidrand-cat: write error: {e}");
            ExitCode::FAILURE
        }
    }
}
