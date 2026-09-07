"""Alternate two prebuilt architecture probes; record hashes, timing and allocs."""
import argparse
import hashlib
import json
from pathlib import Path
import statistics
import subprocess
import re


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repeats", type=int, default=7)
    parser.add_argument("--nodes", type=int, nargs="+", default=[1000, 10000])
    parser.add_argument("--cases", nargs="+", default=["mixed_update", "mixed_fixed", "churn", "frame_idle", "frame_sparse", "frame_all"])
    parser.add_argument("--cpu", type=int, help="Optional Windows CPU affinity index; inherited by probes")
    args = parser.parse_args()
    if args.repeats < 3 or any(n < 1 for n in args.nodes):
        parser.error("use at least 3 repeats and positive node counts")
    if args.cpu is not None:
        import ctypes
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.GetCurrentProcess.restype = ctypes.c_void_p
        kernel.SetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
        if args.cpu < 0 or not kernel.SetProcessAffinityMask(kernel.GetCurrentProcess(), 1 << args.cpu):
            raise ctypes.WinError(ctypes.get_last_error())
    exes = {"baseline": args.baseline.resolve(), "candidate": args.candidate.resolve()}
    result = {"cpu": args.cpu, "repeats": args.repeats, "hashes": {k: hashlib.sha256(p.read_bytes()).hexdigest() for k, p in exes.items()}, "rows": [], "allocations": []}
    def probe(label, case, count, alloc=False):
        command = [str(exes[label]), "--architecture-probe", f"--nodes={count}", f"--case={case}"]
        if alloc:
            command.append("--architecture-alloc")
        output = subprocess.check_output(command, text=True)
        values = {k: int(v) for k, v in re.findall(r"(median_ns|p95_ns|allocs|live_delta|runtime_bytes)=(\d+)", output)}
        if "median_ns" not in values:
            raise RuntimeError(output)
        return values
    def save():
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2), encoding="utf-8")
    for count in args.nodes:
        for case in args.cases:
            row = {"case": case, "nodes": count, "baseline": [], "candidate": []}
            for repeat in range(args.repeats):
                for label in (["baseline", "candidate"] if repeat % 2 == 0 else ["candidate", "baseline"]):
                    row[label].append(probe(label, case, count))
            ratios = [c["median_ns"] / b["median_ns"] for b, c in zip(row["baseline"], row["candidate"])]
            row["paired_delta_percent"] = (statistics.median(ratios) - 1) * 100
            row["slower_pairs"] = sum(r > 1 for r in ratios)
            result["rows"].append(row)
            save()
            print(f"{case}/{count}: {row['paired_delta_percent']:+.2f}% ({row['slower_pairs']}/{args.repeats} slower pairs)", flush=True)
    for case in args.cases:
        count = max(args.nodes)
        row = {"case": case, "nodes": count, **{k: probe(k, case, count, alloc=True) for k in exes}}
        result["allocations"].append(row)
        save()
    # Report evidence only; do not turn measurement noise into an automatic pass.
    print(f"Saved {args.output}; review repeated deltas before accepting", flush=True)


if __name__ == "__main__":
    main()
