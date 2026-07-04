# rapidrand statistical-test harness.
#
# `rapidrand-cat` streams raw RNG bytes to stdout; these recipes pipe that into PractRand
# (`RNG_test`) and TestU01 (`testu01_stdin`). See TESTING.md for tool installation and how to read
# the results.
#
# Both tools are built from source (neither ships on a package manager). Point these env vars at
# your local builds; the defaults assume the binaries are on PATH / under /usr/local.
#
#   RNG_TEST        path to PractRand's compiled RNG_test binary
#                   (default: RNG_test, i.e. found on PATH)
#                   e.g. export RNG_TEST=~/src/practrand/PractRand/RNG_test
#   TESTU01_PREFIX  install prefix holding TestU01's include/ and lib/
#                   (default: /usr/local)
#                   e.g. export TESTU01_PREFIX=~/src/TestU01
#
# For example:
#   RNG_TEST=~/src/practrand/PractRand/RNG_test just practrand rapidrand
#   TESTU01_PREFIX=~/src/TestU01 just testu01 rapidrand bigcrush

set shell := ["bash", "-uc"]

# PractRand's RNG_test binary (build from source; not on any package manager).
practrand := env_var_or_default("RNG_TEST", "RNG_test")

# Install prefix containing TestU01's include/ and lib/ (libtestu01, libprobdist, libmylib).
testu01_prefix := env_var_or_default("TESTU01_PREFIX", "/usr/local")

# Compiled rapidrand-cat and the TestU01 stdin stub.
cat := justfile_directory() / "target/release/rapidrand-cat"
testu01_bin := justfile_directory() / "tools/testu01_stdin"

# Every generator rapidrand-cat knows, for the sweep recipes.
rngs := "rapidrand fastrand turborand nanorand_wyrand pcg32 pcg64 xoshiro256++ xoshiro256** chacha8 chacha20 rand_small rand_std"

_default:
    @just --list
    @echo ""
    @echo "Tool locations (build both from source; override via env var):"
    @echo "  RNG_TEST        PractRand RNG_test binary    (default: RNG_test on PATH)"
    @echo "                  e.g. ~/src/practrand/PractRand/RNG_test"
    @echo "  TESTU01_PREFIX  TestU01 install prefix        (default: /usr/local)"
    @echo "                  e.g. ~/src/TestU01"

# Build the byte streamer (release, matching the workspace LTO profile).
build:
    cargo build --release -p rapidrand-cat

# List the generators available to stream.
list: build
    @{{cat}} --rng list

# ── PractRand ──────────────────────────────────────────────────────────────────
# Run PractRand against one RNG, doubling data volume until failure or `max` bytes.
# Extra flags pass straight to RNG_test, e.g. `just practrand rapidrand 1TB -tf 2`.
practrand rng max="32TB" *flags: build
    {{cat}} --rng {{rng}} | {{practrand}} stdin64 -tlmax {{max}} -tf 2 -te 1 {{flags}}

# PractRand on the bit-reversed stream (probes the high bits).
practrand-rev rng max="32TB" *flags: build
    {{cat}} --rng {{rng}} --bit-reverse | {{practrand}} stdin64 -tlmax {{max}} -tf 2 -te 1 {{flags}}

# PractRand on interleaved adjacent seeds (inter-stream correlation — key for rapidrand).
practrand-interleave rng streams="256" max="32TB" *flags: build
    {{cat}} --rng {{rng}} --mode interleave --streams {{streams}} \
        | {{practrand}} stdin64 -tlmax {{max}} -tf 2 -te 1 {{flags}}

# Quick PractRand pass (default 4GB) across every RNG, for side-by-side comparison.
practrand-sweep max="4GB": build
    #!/usr/bin/env bash
    set -uo pipefail
    for rng in {{rngs}}; do
        echo "=================  $rng  ================="
        {{cat}} --rng "$rng" | {{practrand}} stdin64 -tlmax {{max}} -tf 2 -te 1
    done

# ── TestU01 / BigCrush ─────────────────────────────────────────────────────────
# Compile the generator-agnostic stdin stub against TestU01.
build-testu01:
    cc {{justfile_directory()}}/tools/testu01_stdin.c -o {{testu01_bin}} \
        -I{{testu01_prefix}}/include -L{{testu01_prefix}}/lib \
        -ltestu01 -lprobdist -lmylib -lm

# Runtimes are single-threaded and CPU-bound on TestU01's own test math (the RNG is never
# the bottleneck); on one modern core expect roughly:
#   smallcrush  ~10^9 numbers        seconds to ~2 min
#   crush       ~2^35 numbers        ~30–60 min
#   bigcrush    ~2^38 numbers (~1TB) ~3–6 hours

# Run a TestU01 battery (smallcrush | crush | bigcrush) against one RNG.
testu01 rng battery="bigcrush": build build-testu01
    {{cat}} --rng {{rng}} | {{testu01_bin}} {{battery}}

# Same batteries on the bit-reversed stream (TestU01 under-tests high bits); same runtimes.
testu01-rev rng battery="bigcrush": build build-testu01
    {{cat}} --rng {{rng}} --bit-reverse | {{testu01_bin}} {{battery}}
