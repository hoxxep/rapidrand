#!/usr/bin/env bash
#
# Compile and run the rapidrand.h C++ test across language standards and both the
# native (__int128) and portable multiply paths. The compiler and any target
# flags come from the environment so CI can drive the compiler/version/target
# matrix:
#
#   CXX             C++ compiler to use          (default: c++)
#   CXXFLAGS_EXTRA  extra flags, e.g. "-m32"     (default: none)
#   STDS            space-separated -std values  (default: c++11 c++14 c++17 c++20)
#
# Unsupported standards are skipped rather than failed, so the same script runs
# on old and new compilers.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/tools/test_rapidrand.cpp"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

CXX="${CXX:-c++}"
CXXFLAGS_EXTRA="${CXXFLAGS_EXTRA:-}"
STDS="${STDS:-c++11 c++14 c++17 c++20}"

echo "compiler: $($CXX --version | head -1)"
echo "extra flags: '${CXXFLAGS_EXTRA}'"

status=0
for std in $STDS; do
    # Skip standards this compiler does not understand.
    if ! $CXX -std="$std" -x c++ -E - </dev/null >/dev/null 2>&1; then
        echo "== skip -std=$std (unsupported) =="
        continue
    fi
    for path in native portable; do
        def=""
        [ "$path" = portable ] && def="-DRAPIDRAND_HAS_INT128=0"
        bin="$TMP/t_${std}_${path}"
        echo "== $CXX -std=$std ($path) $CXXFLAGS_EXTRA =="
        # shellcheck disable=SC2086
        if $CXX -std="$std" -Wall -Wextra -O2 $def $CXXFLAGS_EXTRA -I"$ROOT" "$SRC" -o "$bin" \
            && "$bin"; then
            :
        else
            echo "  FAIL: $CXX -std=$std ($path) $CXXFLAGS_EXTRA"
            status=1
        fi
    done
done

if [ "$status" -eq 0 ]; then
    echo "ALL C++ BUILDS PASSED"
fi
exit "$status"
