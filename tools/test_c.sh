#!/usr/bin/env bash
#
# Compile-only check that rapidrand.h builds as C (not C++), across language
# standards and both the native (__int128) and portable multiply paths. Only
# compilation is checked here (`-c`, no link/run); output correctness is covered
# by the C++ test and the separate Rust equivalence test.
#
#   CC            C compiler to use            (default: cc)
#   CFLAGS_EXTRA  extra flags, e.g. "-m32"     (default: none)
#   STDS          space-separated -std values  (default: c99 c11 c17)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/tools/test_rapidrand.c"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

CC="${CC:-cc}"
CFLAGS_EXTRA="${CFLAGS_EXTRA:-}"
STDS="${STDS:-c99 c11 c17}"

echo "compiler: $($CC --version | head -1)"
echo "extra flags: '${CFLAGS_EXTRA}'"

status=0
for std in $STDS; do
    if ! $CC -std="$std" -x c -E - </dev/null >/dev/null 2>&1; then
        echo "== skip -std=$std (unsupported) =="
        continue
    fi
    for path in native portable; do
        def=""
        [ "$path" = portable ] && def="-DRAPIDRAND_HAS_INT128=0"
        echo "== $CC -std=$std ($path) $CFLAGS_EXTRA =="
        # shellcheck disable=SC2086
        if $CC -std="$std" -Wall -Wextra -c $def $CFLAGS_EXTRA -I"$ROOT" "$SRC" -o "$TMP/o.o"; then
            :
        else
            echo "  FAIL: $CC -std=$std ($path) $CFLAGS_EXTRA"
            status=1
        fi
    done
done

if [ "$status" -eq 0 ]; then
    echo "ALL C COMPILE CHECKS PASSED"
fi
exit "$status"
