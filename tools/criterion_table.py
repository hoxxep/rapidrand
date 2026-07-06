#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# dependencies = ["tabulate"]
# ///
"""Print the criterion RNG benchmarks as a markdown table for the README.

Reads `target/criterion/{u64,u32,fill}/*/new/` (the most recent `cargo bench
-p rapidrand-bench` run). Per-call times are derived from each run's recorded
`Throughput::Bytes`, so results stay correct whether an iteration measured a
single draw or a batch.

Usage:
    uv run tools/criterion_table.py
"""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

from tabulate import tabulate

CRITERION_DIR = Path(__file__).resolve().parent.parent / "target" / "criterion"

# (criterion group, bytes produced per call; None = report GB/s of the whole buffer)
GROUPS = [("u64", 8), ("u32", 4), ("fill", None)]

# Bench function id -> what the row measures. Also fixes the row order for ties.
CRATES = {
    "rapidrand": "**[rapidrand](https://crates.io/crates/rapidrand) `RapidRand`**",
    "fastrand": "[fastrand](https://crates.io/crates/fastrand) `Rng`",
    "nanorand_wyrand": "[nanorand](https://crates.io/crates/nanorand) `WyRand`",
    "turborand": "[turborand](https://crates.io/crates/turborand) `Rng`",
    "rand_small": "[rand](https://crates.io/crates/rand) `SmallRng`",
    "rand_std": "[rand](https://crates.io/crates/rand) `StdRng` (ChaCha12)",
    "pcg32": "[rand_pcg](https://crates.io/crates/rand_pcg) `Pcg32`",
    "pcg64": "[rand_pcg](https://crates.io/crates/rand_pcg) `Pcg64`",
    "xoshiro256++": "[rand_xoshiro](https://crates.io/crates/rand_xoshiro) `Xoshiro256PlusPlus`",
    "xoshiro256**": "[rand_xoshiro](https://crates.io/crates/rand_xoshiro) `Xoshiro256StarStar`",
    "chacha8": "[rand_chacha](https://crates.io/crates/rand_chacha) `ChaCha8Rng`",
    "chacha20": "[rand_chacha](https://crates.io/crates/rand_chacha) `ChaCha20Rng`",
}


def load_group(group: str) -> dict[str, tuple[float, int]]:
    """Return {function_id: (mean ns per iteration, throughput bytes per iteration)}."""
    results: dict[str, tuple[float, int]] = {}
    group_dir = CRITERION_DIR / group
    if not group_dir.is_dir():
        return results
    for bench_dir in group_dir.iterdir():
        new = bench_dir / "new"
        try:
            benchmark = json.loads((new / "benchmark.json").read_text())
            estimates = json.loads((new / "estimates.json").read_text())
        except (OSError, json.JSONDecodeError):
            continue  # `report/` dir, or an interrupted run
        name = benchmark["function_id"]
        mean_ns = estimates["mean"]["point_estimate"]
        tp_bytes = benchmark.get("throughput", {}).get("Bytes")
        if tp_bytes is None:
            print(f"warning: {group}/{name} has no byte throughput, skipped", file=sys.stderr)
            continue
        results[name] = (mean_ns, tp_bytes)
    return results


def fmt_ns(ns: float) -> str:
    return f"{ns:.2f} ns"


def fmt_gbps(gbps: float) -> str:
    return f"{gbps:.2f} GB/s"


def main() -> None:
    data = {group: load_group(group) for group, _ in GROUPS}
    # The `_raw` baselines bench the bare mixing function, not an RNG people would use.
    names = {name for results in data.values() for name in results if not name.endswith("_raw")}
    if not names:
        sys.exit(f"no criterion results under {CRITERION_DIR}; run `cargo bench -p rapidrand-bench` first")

    def geomean_gbps(name: str) -> float:
        """Geometric mean of throughput (GB/s = bytes/ns) across the benchmarks."""
        rates = [
            tp_bytes / mean_ns
            for group, _ in GROUPS
            if (entry := data[group].get(name)) is not None
            for mean_ns, tp_bytes in [entry]
        ]
        if not rates:
            return 0.0
        return math.prod(rates) ** (1 / len(rates))

    # rapidrand first, then everything else by geomean throughput (ties broken by CRATES order).
    def sort_key(name: str) -> tuple[bool, float, int]:
        crate_order = list(CRATES).index(name) if name in CRATES else len(CRATES)
        return (not name.startswith("rapidrand"), -geomean_gbps(name), crate_order)

    fill_bytes = next((tp for _, tp in data["fill"].values()), 1024)

    rows = []
    for name in sorted(names, key=sort_key):
        cells = []
        for group, word in GROUPS:
            entry = data[group].get(name)
            if entry is None:
                cells.append("–")
            else:
                mean_ns, tp_bytes = entry
                if word is not None:
                    cells.append(fmt_ns(mean_ns * word / tp_bytes))
                else:
                    cells.append(fmt_gbps(tp_bytes / mean_ns))

        if name not in CRATES:
            continue
        rows.append([CRATES.get(name, ""), *cells])

    headers = ["RNG", "`u64`", "`u32`", f"fill {fill_bytes / 1024:g} KiB"]
    align = ("left", "right", "right", "right")
    # "pipe" (not "github") emits the `---:` alignment markers GitHub renders.
    print(tabulate(rows, headers=headers, tablefmt="pipe", colalign=align))


if __name__ == "__main__":
    main()
