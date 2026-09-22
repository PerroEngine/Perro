"""Compare prebuilt runtime frame probes with the same frame lifecycle."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import statistics
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--nodes", type=int, default=10_000)
    args = parser.parse_args()
    exes = {"baseline": args.baseline.resolve(), "candidate": args.candidate.resolve()}
    result = {
        "method": "A/B/B/A independent processes; 60 warmup frames; 101 batches of 10 frames; default process priority and all CPUs; allocator probes separate",
        "scope": "fixed update + update + 2D/3D/UI extraction + command drain + clear_dirty_flags; no GPU submission",
        "nodes": args.nodes,
        "hashes": {label: hashlib.sha256(path.read_bytes()).hexdigest() for label, path in exes.items()},
        "cases": {},
    }

    def save():
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")

    def run(label, case, memory=False):
        command = [str(exes[label]), f"--nodes={args.nodes}"]
        if case.startswith("empty_"):
            command += ["--empty-extract", f"--dimension={case[-1]}"]
        else:
            command += ["--architecture-probe", f"--case={case}"]
            if memory:
                command += ["--architecture-alloc"]
        raw = subprocess.check_output(command, text=True).strip()
        values = {key: int(value) for key, value in re.findall(r"(\w+)=(\d+)", raw)}
        if "median_ns" not in values:
            raise RuntimeError(f"Probe did not produce timings: {raw}")
        return {"label": label, "raw": raw, **values}

    for case in ["frame_idle", "frame_sparse", "frame_all", "empty_2", "empty_3"]:
        row = result["cases"][case] = {"runs": []}
        for label in ["baseline", "candidate", "candidate", "baseline"]:
            row["runs"].append(run(label, case))
            save()
        baseline = statistics.mean(r["median_ns"] for r in row["runs"] if r["label"] == "baseline")
        candidate = statistics.mean(r["median_ns"] for r in row["runs"] if r["label"] == "candidate")
        row["mean_process_median_delta_percent"] = (candidate / baseline - 1) * 100
        print(f"{case}: {baseline:.0f} -> {candidate:.0f} ns ({row['mean_process_median_delta_percent']:+.2f}%)", flush=True)
        save()
    # Keep instrumentation out of ordinary timing and retain baseline allocation controls.
    for case in ["frame_idle", "frame_sparse", "frame_all"]:
        result["cases"][case]["memory"] = [run(label, case, memory=True) for label in exes]
        save()


if __name__ == "__main__":
    main()
