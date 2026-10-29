#!/usr/bin/env python3
"""Small M6 native differential test, independent of YARPGen generation.

Usage:
  python3 tools/m6_gnu_differential.py --codegen /path/to/codegen [--edg-environment /path/to/environment.sh]
Without --codegen, verify only the GCC fixture oracle. A baseline-only run is
never reported as a Cranelift pass.
"""
import argparse
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "tests/fixtures/m6_gnu_statement_expr.c"
FUNCTIONS = ("m6_gnu_simple", "m6_gnu_nested", "m6_gnu_order",
             "m6_gnu_short_circuit", "m6_gnu_conditional")
VALUES = (-9, -1, 0, 1, 2, 3, 17)
DRIVER = """#include <stdio.h>
%s
int main(void) {
%s
    return 0;
}
""" % ("\n".join(f"int {f}(int);" for f in FUNCTIONS),
       "\n".join(f'    printf("{f}({v})=%%d\\n", {f}({v}));'
                 for f in FUNCTIONS for v in VALUES))


def run(cmd, cwd):
    return subprocess.run(cmd, cwd=cwd, check=True, capture_output=True, text=True).stdout


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--codegen", type=Path)
    parser.add_argument("--edg-environment", type=Path)
    parser.add_argument("--codegen-cwd", type=Path)
    parser.add_argument("--gcc", default="gcc")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="m6-gnu-") as temp:
        work = Path(temp)
        (work / "driver.c").write_text(DRIVER)
        run([args.gcc, "-std=gnu11", "-O0", str(FIXTURE),
             "driver.c", "-o", "reference"], work)
        expected = run(["./reference"], work)
        if not args.codegen:
            print(f"GCC baseline OK: {len(FUNCTIONS) * len(VALUES)} cases; Cranelift NOT TESTED")
            return
        codegen = str(args.codegen.resolve())
        command = [codegen, str(work / "cranelift.o"), "--c11", "--gcc", str(FIXTURE)]
        if args.edg_environment:
            command = ["bash", "-c", 'source "$1"; shift; exec "$@"',
                       "_", str(args.edg_environment.resolve()), *command]
        run(command, args.codegen_cwd.resolve() if args.codegen_cwd else work)
        run([args.gcc, "-std=gnu11", "-O0", "driver.c",
             "cranelift.o", "-o", "cranelift"], work)
        actual = run(["./cranelift"], work)
        if actual != expected:
            expected_lines, actual_lines = expected.splitlines(), actual.splitlines()
            for index, (want, got) in enumerate(zip(expected_lines, actual_lines)):
                if want != got:
                    raise SystemExit(f"Mismatch at case {index}: GCC={want!r}, Cranelift={got!r}")
            raise SystemExit("Mismatch in differential output length")
        print(f"Cranelift matched GCC: {len(FUNCTIONS) * len(VALUES)} cases")


if __name__ == "__main__":
    main()
