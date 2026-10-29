#!/usr/bin/env python3
"""Deterministic, bounded YARPGen C differential testing.

Uses the version-pinned generator binary, not the YARPGen build tree.
The generator's func.c is compiled by EDG/Cranelift; its driver.c is
compiled by GCC. Until Cranelift supports YARPGen's C subset, explicitly
unsupported generated programs are counted as UNSUPPORTED, never PASS.

Python >=3.10, Linux x86-64, GCC. No external Python dependencies.
"""
from __future__ import annotations

import argparse
from collections import Counter
from dataclasses import dataclass
import hashlib
import json
import math
import os
from pathlib import Path
import re
import resource
import shutil
import signal
import subprocess
import sys
import time

DIGEST_RE = re.compile(r"^[0-9]+$")
CATEGORIES = {
    "pass", "generator_failure", "baseline_failure", "baseline_disagree",
    "unsupported_construct", "frontend_error", "codegen_failure",
    "link_failure", "runtime_failure", "wrong_output",
}


@dataclass
class ProcessResult:
    cmd: list[str]
    returncode: int | None
    stdout: str
    stderr: str
    duration_s: float
    timed_out: bool = False

    @property
    def ok(self) -> bool:
        return self.returncode == 0 and not self.timed_out


def run(cmd: list[str], *, cwd: Path, seconds: int, memory_mb: int) -> ProcessResult:
    def limits() -> None:
        if memory_mb:
            size = memory_mb * 1024 * 1024
            resource.setrlimit(resource.RLIMIT_AS, (size, size))
        resource.setrlimit(resource.RLIMIT_CPU, (max(2, seconds + 1), max(2, seconds + 2)))
        resource.setrlimit(resource.RLIMIT_CORE, (0, 0))

    started = time.monotonic()
    try:
        p = subprocess.Popen(
            cmd, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            text=True, start_new_session=True, preexec_fn=limits,
        )
    except OSError as exc:
        return ProcessResult(cmd, None, "", str(exc), time.monotonic() - started)
    try:
        stdout, stderr = p.communicate(timeout=seconds)
        timed_out = False
    except subprocess.TimeoutExpired:
        # Kill compiler subprocesses too; a timed-out test must not leak workers.
        os.killpg(p.pid, signal.SIGKILL)
        stdout, stderr = p.communicate()
        timed_out = True
    return ProcessResult(cmd, p.returncode, stdout[-65536:], stderr[-65536:],
                         time.monotonic() - started, timed_out)


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        while data := stream.read(1024 * 1024):
            h.update(data)
    return h.hexdigest()


def result_value(step: ProcessResult) -> str | None:
    if not step.ok:
        return None
    stdout = step.stdout.strip()
    if "ERROR:" in stdout or not DIGEST_RE.fullmatch(stdout):
        return None
    return stdout


def classify_codegen(step: ProcessResult) -> str:
    log = (step.stdout + "\n" + step.stderr).lower()
    # Keep unsupported C separate from frontend errors, backend crashes, and
    # wrong code; do not treat every error as expected "unsupported".
    if any(msg in log for msg in (
        "abi must be signed 64-bit integer",
        "unsupported edg statement", "unsupported edg expression",
        "unsupported edg operation", "unsupported c expression type",
        "gnu statement expression requires block-value semantics",
        "unsupported non-loop goto", "unsupported target",
        "unsupported local", "not supported", "unsupported construct",
        "initializer has no supported", "only simple local assignment",
        "requires signed 64-bit",
    )):
        return "unsupported_construct"
    if "edg compilation failed" in log or "edg bridge:" in log:
        return "frontend_error"
    return "codegen_failure"


def seeds_from_args(args: argparse.Namespace) -> list[int]:
    if args.seeds:
        seeds = [int(s.strip()) for s in args.seeds.split(",") if s.strip()]
    elif args.seeds_file:
        seeds = [
            int(line.strip()) for line in Path(args.seeds_file).read_text().splitlines()
            if line.strip() and not line.lstrip().startswith("#")
        ]
    else:
        seeds = list(range(args.start_seed, args.start_seed + args.count))
    if not seeds or any(s <= 0 or s >= 2**64 for s in seeds):
        raise ValueError("seeds must be nonzero unsigned 64-bit integers")
    if len(set(seeds)) != len(seeds):
        raise ValueError("duplicate seeds would overwrite test results")
    return seeds


def recorded(
    record: dict, label: str, cmd: list[str], *, cwd: Path,
    seconds: int, memory_mb: int, case: Path,
) -> ProcessResult:
    step = run(cmd, cwd=cwd, seconds=seconds, memory_mb=memory_mb)
    path = case / "logs"
    path.mkdir(exist_ok=True)
    (path / f"{label}.stdout").write_text(step.stdout)
    (path / f"{label}.stderr").write_text(step.stderr)
    record["steps"].append({
        "label": label, "argv": cmd, "cwd": str(cwd),
        "returncode": step.returncode, "timed_out": step.timed_out,
        "duration_s": round(step.duration_s, 3),
    })
    return step


def execute_case(
    seed: int, args: argparse.Namespace, root: Path, provenance: dict,
) -> dict:
    case = root / f"seed-{seed}"
    case.mkdir()
    record: dict = {
        "seed": seed, "status": None, "steps": [],
        "generator": provenance["generator"], "generator_sha256": provenance["generator_sha256"],
        "generator_version": provenance["generator_version"],
        "generator_options": args.generator_option,
        "gcc": provenance["gcc"], "clang": provenance.get("clang"),
        "codegen": str(args.codegen) if args.codegen else None,
        "edg_options": args.edg_option,
        "target": "linux-x86_64",
    }

    def finish(status: str, *, baseline: str | None = None,
               cranelift: str | None = None, reason: str | None = None) -> dict:
        assert status in CATEGORIES
        record.update(status=status, baseline=baseline, cranelift=cranelift, reason=reason)
        record["source_sha256"] = {
            name: digest(case / name)
            for name in ("init.h", "func.c", "driver.c")
            if (case / name).is_file()
        }
        (case / "result.json").write_text(json.dumps(record, indent=2, sort_keys=True) + "\n")
        if status == "pass" and not args.keep_passing:
            shutil.rmtree(case)
        return record

    generated = recorded(record, "generate",
        [str(args.generator), f"--seed={seed}", "--std=c", f"--out-dir={case}",
         *args.generator_option],
        cwd=case, seconds=args.timeout, memory_mb=args.memory_mb, case=case)
    if not generated.ok or not all((case / p).is_file() and (case / p).stat().st_size
                                   for p in ("init.h", "func.c", "driver.c")):
        return finish("generator_failure", reason="generator error or missing output")

    results: dict[str, str] = {}
    profiles = [
        ("gcc-O0", [args.gcc, "-std=gnu11", "-O0", "-Wno-unknown-pragmas"]),
        ("gcc-O2", [args.gcc, "-std=gnu11", "-O2", "-Wno-unknown-pragmas"]),
    ]
    if args.clang:
        profiles.append(("clang-O2", [args.clang, "-std=gnu11", "-O2",
                                       "-Wno-unknown-pragmas"]))
    for label, compiler in profiles:
        binary = case / label
        built = recorded(record, f"build-{label}",
            [*compiler, "func.c", "driver.c", "-o", str(binary)],
            cwd=case, seconds=args.timeout, memory_mb=args.memory_mb, case=case)
        if not built.ok:
            return finish("baseline_failure", reason=f"could not build {label}")
        ran = recorded(record, f"run-{label}", [str(binary)],
                       cwd=case, seconds=args.timeout, memory_mb=args.memory_mb, case=case)
        value = result_value(ran)
        if value is None:
            return finish("baseline_failure", reason=f"bad runtime or checksum: {label}")
        results[label] = value
    baseline = results["gcc-O0"]
    if len(set(results.values())) != 1:
        return finish("baseline_disagree", baseline=str(results),
                      reason="reference compiler outputs disagree")

    if not args.codegen:
        return finish("pass", baseline=baseline, reason="baseline-only verified")

    obj = case / "cranelift.o"
    codegen_cmd = [str(args.codegen), str(obj), *args.edg_option, str(case / "func.c")]
    if args.edg_environment:
        # Match the repository's existing EDG runner: source environment.sh
        # from the configured EDG build directory before invoking the binary.
        codegen_cmd = ["bash", "-c",
                       'source "$1"; shift; exec "$@"', "_",
                       str(args.edg_environment), *codegen_cmd]
    codegen_cwd = args.codegen_cwd or case
    compiled = recorded(record, "edg-cranelift", codegen_cmd,
        cwd=codegen_cwd, seconds=args.timeout, memory_mb=args.memory_mb, case=case)
    if not compiled.ok or not obj.exists():
        return finish(classify_codegen(compiled) if not compiled.ok else "codegen_failure",
                      baseline=baseline, reason="EDG/Cranelift could not emit object")

    output = case / "cranelift-test"
    linked = recorded(record, "link-cranelift",
        [args.gcc, "-std=gnu11", "-O2", "driver.c", str(obj), "-o", str(output)],
        cwd=case, seconds=args.timeout, memory_mb=args.memory_mb, case=case)
    if not linked.ok:
        return finish("link_failure", baseline=baseline, reason="GCC link failed")
    ran = recorded(record, "run-cranelift", [str(output)], cwd=case,
                   seconds=args.timeout, memory_mb=args.memory_mb, case=case)
    generated_value = result_value(ran)
    if generated_value is None:
        return finish("runtime_failure", baseline=baseline,
                      reason="Cranelift binary crashed, hung, or returned malformed checksum")
    if generated_value != baseline:
        return finish("wrong_output", baseline=baseline, cranelift=generated_value)
    return finish("pass", baseline=baseline, cranelift=generated_value)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--generator", type=Path, required=True,
                        help="path to verified, unpacked pinned YARPGen executable")
    parser.add_argument("--generator-sha256", default="",
                        help="optional pinned executable SHA-256 (not archive digest)")
    parser.add_argument("--out", type=Path, required=True, help="new/empty output directory")
    selector = parser.add_mutually_exclusive_group()
    selector.add_argument("--seeds", help="comma-separated decimal seeds")
    selector.add_argument("--seeds-file", type=Path)
    parser.add_argument("--start-seed", type=int, default=1)
    parser.add_argument("--count", type=int, default=10)
    parser.add_argument("--generator-option", action="append", default=[],
                        help="extra generator CLI option (repeatable)")
    parser.add_argument("--gcc", default="gcc")
    parser.add_argument("--clang", default="", help="optional Clang reference")
    parser.add_argument("--codegen", type=Path, default=None,
                        help="optional EDG/Cranelift example executable")
    parser.add_argument("--edg-option", action="append", default=[],
                        help="EDG frontend option (repeatable)")
    parser.add_argument("--edg-environment", type=Path,
                        help="source matching EDG environment.sh in subprocess")
    parser.add_argument("--codegen-cwd", type=Path,
                        help="EDG build directory (cwd for codegen process)")
    parser.add_argument("--timeout", type=int, default=45,
                        help="wall seconds per individual compile or execution")
    parser.add_argument("--memory-mb", type=int, default=2048)
    parser.add_argument("--keep-passing", action="store_true")
    parser.add_argument("--strict", action="store_true",
                        help="fail if any case did not pass, including unsupported")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        seeds = seeds_from_args(args)
        if args.timeout < 1 or args.memory_mb < 64 or len(seeds) > 100000:
            raise ValueError("invalid resource limits or seed count")
        args.generator = args.generator.resolve(strict=True)
        args.out = args.out.resolve()
        args.codegen = args.codegen.resolve(strict=True) if args.codegen else None
        args.codegen_cwd = args.codegen_cwd.resolve(strict=True) if args.codegen_cwd else None
        args.edg_environment = (args.edg_environment.resolve(strict=True)
                                if args.edg_environment else None)
        if args.out.exists() and any(args.out.iterdir()):
            raise ValueError("output directory must be empty")
        args.out.mkdir(parents=True, exist_ok=True)
    except (ValueError, OSError) as exc:
        print(f"invalid configuration: {exc}", file=sys.stderr)
        return 2

    actual_sha = digest(args.generator)
    if args.generator_sha256 and actual_sha.lower() != args.generator_sha256.lower():
        print("pinned generator executable SHA-256 mismatch", file=sys.stderr)
        return 2
    def version(command: list[str]) -> str:
        step = run(command, cwd=args.out, seconds=10, memory_mb=512)
        return (step.stdout or step.stderr).strip()[:1000] if step.ok else "unavailable"

    provenance = {
        "generator": str(args.generator), "generator_sha256": actual_sha,
        "generator_version": version([str(args.generator), "--version"]),
        "gcc": version([args.gcc, "--version"]).splitlines()[0],
        "clang": version([args.clang, "--version"]).splitlines()[0] if args.clang else None,
    }
    (args.out / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
    counts = Counter()
    records = args.out / "results.jsonl"
    with records.open("w") as output:
        for seed in seeds:
            record = execute_case(seed, args, args.out, provenance)
            counts[record["status"]] += 1
            output.write(json.dumps(record, sort_keys=True) + "\n")
            output.flush()
            print(f"seed {seed}: {record['status']}", flush=True)

    summary = {
        "generated": len(seeds),
        "counts": {name: counts[name] for name in sorted(CATEGORIES)},
        "oracle_valid": len(seeds) - counts["generator_failure"] -
                        counts["baseline_failure"] - counts["baseline_disagree"],
        "backend_accepted": (counts["pass"] + counts["wrong_output"] +
                             counts["runtime_failure"] + counts["link_failure"])
                            if args.codegen else 0,
        "mode": "comparison" if args.codegen else "baseline",
    }
    (args.out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(summary, sort_keys=True))
    severe = {"generator_failure", "baseline_failure", "baseline_disagree",
              "codegen_failure", "link_failure", "runtime_failure", "wrong_output"}
    if args.strict:
        return 0 if counts["pass"] == len(seeds) else 1
    return 1 if any(counts[name] for name in severe) else 0


if __name__ == "__main__":
    sys.exit(main())
