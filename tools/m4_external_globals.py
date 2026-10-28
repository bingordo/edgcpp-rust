#!/usr/bin/env python3
"""M4 external-global differential gate.

Run without --candidate to validate fixture/GCC oracle. Once the EDG backend
can generate the test translation unit, pass its object with --candidate.
This script does not count a GCC-only baseline as an EDG pass.
"""
import argparse
import pathlib
import subprocess
import tempfile


def run(*args: str) -> str:
    result = subprocess.run(args, check=True, text=True, capture_output=True)
    return result.stdout


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--candidate", type=pathlib.Path,
                   help="EDG/Cranelift-generated ELF object for m4_external_globals.c")
    p.add_argument("--cc", default="gcc")
    args = p.parse_args()
    root = pathlib.Path(__file__).resolve().parent.parent / "tests" / "fixtures"
    with tempfile.TemporaryDirectory(prefix="edg-m4-") as td:
        td = pathlib.Path(td)
        driver = td / "driver.o"
        reference = td / "reference.o"
        gcc_exe = td / "gcc-reference"
        common = (args.cc, "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror")
        run(*common, "-c", str(root / "m4_external_driver.c"), "-o", str(driver))
        run(*common, "-c", str(root / "m4_external_globals.c"), "-o", str(reference))
        run(args.cc, str(driver), str(reference), "-o", str(gcc_exe))
        expected = run(str(gcc_exe))
        print("GCC baseline passed:")
        print(expected, end="")
        if args.candidate is None:
            print("EDG candidate NOT TESTED (pass --candidate path/to/object.o)")
            return 0
        candidate = args.candidate.resolve(strict=True)
        edg_exe = td / "edg-candidate"
        run(args.cc, str(driver), str(candidate), "-o", str(edg_exe))
        actual = run(str(edg_exe))
        if actual != expected:
            print("EDG/GCC mismatch:\nexpected:\n" + expected + "actual:\n" + actual)
            return 1
        print("EDG/GCC differential PASS")
        return 0


if __name__ == "__main__":
    raise SystemExit(main())
