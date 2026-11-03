#!/usr/bin/env python3
"""M10 baseline oracle: independently compile and execute the callback fixture.

This is *not* an EDG pass gate. Run the EDG-produced object with the same GCC
driver separately once indirect-call lowering is implemented.
"""
import argparse
import pathlib
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests" / "fixtures"

def run(cmd):
    subprocess.run(cmd, check=True)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--gcc", default="gcc")
    parser.add_argument("--clang", default="clang")
    args = parser.parse_args()
    source = FIXTURES / "m10_indirect_calls.c"
    driver = FIXTURES / "m10_indirect_driver.c"
    with tempfile.TemporaryDirectory(prefix="m10-abi-") as temp:
        out = pathlib.Path(temp)
        for compiler in (args.gcc, args.clang):
            for opt in ("-O0", "-O2"):
                obj = out / (pathlib.Path(compiler).name + opt + ".o")
                binary = out / (pathlib.Path(compiler).name + opt)
                run([compiler, "-std=c11", "-Wall", "-Wextra", "-Werror",
                     opt, "-c", str(source), "-o", str(obj)])
                # GCC is always the caller to exercise cross-compiler ABI.
                run([args.gcc, "-std=c11", "-Wall", "-Wextra", "-Werror",
                     str(driver), str(obj), "-o", str(binary)])
                run([str(binary)])
                print(f"PASS {compiler} {opt} -> GCC driver")

if __name__ == "__main__":
    main()
