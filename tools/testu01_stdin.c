/* testu01_stdin.c — run a TestU01 battery against raw 32-bit words read from stdin.
 *
 * This is deliberately generator-agnostic: it reimplements no RNG. It hands TestU01 an "external"
 * generator whose next-bits callback simply reads 4 bytes from stdin, so *any* producer (our
 * `rng-cat`, or any other byte source) can be tested by piping into it:
 *
 *     rapidrand-cat --rng rapidrand | ./testu01_stdin bigcrush
 *
 * Batteries: smallcrush | crush | bigcrush (default: bigcrush).
 *
 * Build (see the `justfile` `build-testu01` recipe):
 *     cc tools/testu01_stdin.c -o tools/testu01_stdin \
 *        -I$TESTU01_PREFIX/include -L$TESTU01_PREFIX/lib \
 *        -ltestu01 -lprobdist -lmylib -lm
 *
 * Notes:
 *  - BigCrush pulls ~2^38 32-bit words (~1 TB) and never rewinds, so the upstream stream must run
 *    effectively unbounded (rng-cat does by default).
 *  - Input is consumed 32 bits at a time in the byte order it arrives. rng-cat emits little-endian
 *    u64s, i.e. the low 32 bits of each word first. Run a second pass with `rng-cat --bit-reverse`
 *    to exercise the high bits, which TestU01 probes far less thoroughly than the low bits.
 *  - On EOF (upstream stopped) the callback exits cleanly.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "unif01.h"
#include "bbattery.h"

static unsigned int next_bits(void) {
    unsigned char b[4];
    if (fread(b, 1, 4, stdin) != 4) {
        /* Upstream closed or ran dry; nothing left to test. */
        exit(0);
    }
    /* Little-endian assembly, matching rng-cat's byte order. */
    return (unsigned int) b[0]
         | ((unsigned int) b[1] << 8)
         | ((unsigned int) b[2] << 16)
         | ((unsigned int) b[3] << 24);
}

int main(int argc, char **argv) {
    const char *battery = (argc > 1) ? argv[1] : "bigcrush";

    unif01_Gen *gen = unif01_CreateExternGenBits("stdin", next_bits);

    if (strcmp(battery, "smallcrush") == 0) {
        bbattery_SmallCrush(gen);
    } else if (strcmp(battery, "crush") == 0) {
        bbattery_Crush(gen);
    } else if (strcmp(battery, "bigcrush") == 0) {
        bbattery_BigCrush(gen);
    } else {
        fprintf(stderr, "testu01_stdin: unknown battery '%s' "
                        "(want smallcrush | crush | bigcrush)\n", battery);
        unif01_DeleteExternGenBits(gen);
        return 2;
    }

    unif01_DeleteExternGenBits(gen);
    return 0;
}
