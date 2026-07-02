# Statistical testing

`rapidrand` is a counter-based mixer: `seed += ADD`, then a folded 128-bit multiply of `seed` and
`seed ^ XOR` (`rapidrand/src/lib.rs`). This document describes how to run the statistical test
suites against it.

Two pieces drive the tests:

- **`rapidrand-cat`** — streams raw RNG bytes (little-endian `u64`) to stdout, for `rapidrand` or
  any competitor (`fastrand`, `turborand`, `nanorand`, PCG, xoshiro, ChaCha).
- **the `justfile`** — recipes that pipe `rapidrand-cat` into PractRand and TestU01.

PractRand and TestU01 consume a byte stream and are not aware of the generator. The adjacent-seed
interleaving and bit-reversal are applied by `rapidrand-cat`, not by the suites.

## Test suites

Supported test suites:

- **PractRand** — reads raw bytes on stdin and scales to terabytes of output.
- **TestU01 BigCrush** — run on both the normal and the bit-reversed stream.

For a counter-based generator, correlation between nearby seeds is a distinct failure mode from
single-stream defects. The `--mode interleave` pass interleaves the output of many adjacent seeds
into one stream to test for it.

The period is a single 2^64 cycle with no independent stream selection. This is a design property,
not a test failure, and no suite addresses it.

## Installing the tools

Both are C projects built from source; neither ships in package managers in a usable test form.

### PractRand

```sh
curl -LO https://downloads.sourceforge.net/project/pracrand/PractRand-pre0.95.zip
unzip PractRand-pre0.95.zip -d PractRand && cd PractRand
g++ -std=c++14 -c src/*.cpp src/RNGs/*.cpp src/RNGs/other/*.cpp -O3 -Iinclude -pthread
ar rcs libPractRand.a *.o
g++ -std=c++14 -O3 -Iinclude tools/RNG_test.cpp libPractRand.a -pthread -o RNG_test
```

Then point the harness at it: `export RNG_TEST=/path/to/PractRand/RNG_test`.

### TestU01

```sh
git clone https://github.com/umontreal-simul/TestU01-2009.git
cd TestU01-2009
./configure --prefix=$HOME/.local/testu01   # run ./bootstrap first if configure is missing
make -j && make install
```

Then: `export TESTU01_PREFIX=$HOME/.local/testu01`. The `just build-testu01` recipe compiles
`tools/testu01_stdin.c` — a generator-agnostic stub that feeds stdin bytes to any battery — against
this prefix.

## Running

```sh
# PractRand on rapidrand, doubling volume up to 16 TB (Ctrl-C any time):
just practrand rapidrand

# Bit-reversed stream (PractRand and TestU01 test high bits less thoroughly than low bits):
just practrand-rev rapidrand

# Inter-stream correlation: 256 adjacent seeds, interleaved:
just practrand-interleave rapidrand

# Quick 4 GB pass across every RNG for side-by-side comparison:
just practrand-sweep

# TestU01 batteries (the stub is compiled on first use):
just testu01 rapidrand smallcrush
just testu01 rapidrand bigcrush
just testu01-rev rapidrand bigcrush    # bit-reversed BigCrush

# Run competing PRNGs through the same harness:
just practrand fastrand
just testu01   xoshiro256++ bigcrush
```

Run `just list` for the full generator list, or `rapidrand-cat --help` for all flags
(`--seed`, `--mode`, `--streams`, `--bit-reverse`, `--limit`).

## Reading the results

- **PractRand** prints one row per data length. `no anomalies` and `unusual` pass; `FAIL`, or a
  p-value printed as `p = ...e-NNNN`, is a defect. Record the data length at which the first failure
  appears and compare it across generators.
- **BigCrush** runs 106 tests and summarises any with p-values outside `[1e-3, 1-1e-3]`. One or two
  marginal p-values across 106 tests is expected noise; a clear failure (p beyond ~1e-6), or the
  same test failing in both the normal and bit-reversed runs, is a defect.
- For reference, `chacha8`/`chacha20` are cryptographic and pass all batteries; use them to confirm
  the harness itself is working.

## Caveats

- `rapidrand-cat` emits **little-endian `u64`s**, so the low 32 bits of each word reach TestU01
  first. The `-rev` recipes run the bit-reversed stream so the high bits are tested directly.
- BigCrush consumes ~2^38 32-bit words (~1 TB) and never rewinds; `rapidrand-cat` streams unbounded
  by default, so this works over a pipe with nothing stored on disk.
- Runs are long: BigCrush takes hours, and a multi-TB PractRand run can take a day or more. Start
  with `just testu01 rapidrand smallcrush` and `just practrand rapidrand 4GB` to check the harness before
  committing to a full run.
