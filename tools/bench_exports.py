"""Alternate prebuilt export probes; compare time, memory, and output bytes."""

import argparse
import hashlib
import json
from pathlib import Path
import statistics
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repeats", type=int, default=5)
    parser.add_argument("--samples", type=int, default=7)
    parser.add_argument("--threads", type=int, default=4)
    parser.add_argument("--cases", nargs="+", default=["csv_cold", "archive_rebuild", "archive", "codegen_small"])
    args = parser.parse_args()
    if args.repeats < 3 or min(args.samples, args.threads) < 1:
        parser.error("use at least 3 repeats and positive samples/threads")
    exes = {"baseline": args.baseline.resolve(), "candidate": args.candidate.resolve()}
    result = {
        "repeats": args.repeats,
        "samples": args.samples,
        "threads": args.threads,
        "hashes": {label: hashlib.sha256(path.read_bytes()).hexdigest() for label, path in exes.items()},
        "rows": [],
    }

    def probe(label, case, memory=False):
        command = [str(exes[label]), f"--case={case}", f"--samples={args.samples}", f"--threads={args.threads}"]
        if memory:
            command.append("--memory")
        output = subprocess.check_output(command, text=True)
        return json.loads(output.strip().splitlines()[-1])

    def save():
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")

    for case in args.cases:
        row = {"case": case, "baseline": [], "candidate": []}
        for repeat in range(args.repeats):
            order = ["baseline", "candidate"] if repeat % 2 == 0 else ["candidate", "baseline"]
            for label in order:
                row[label].append(probe(label, case))
        checksums = {run["output_checksum"] for label in exes for run in row[label]}
        if len(checksums) != 1:
            raise RuntimeError(f"{case}: output checksums differ: {checksums}")
        ratios = [
            statistics.median(candidate["samples_ns"]) / statistics.median(baseline["samples_ns"])
            for baseline, candidate in zip(row["baseline"], row["candidate"])
        ]
        row["paired_delta_percent"] = (statistics.median(ratios) - 1) * 100
        row["slower_pairs"] = sum(ratio > 1 for ratio in ratios)
        # Allocator accounting affects timings: keep it in separate processes.
        row["memory"] = {label: probe(label, case, memory=True) for label in exes}
        result["rows"].append(row)
        save()
        print(f"{case}: {row['paired_delta_percent']:+.2f}% ({row['slower_pairs']}/{args.repeats} slower pairs); matching output", flush=True)


if __name__ == "__main__":
    main()
