"""Fast, generator-independent regression tests for the YARPGen harness."""
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
HARNESS = ROOT / "tools" / "yarpgen_smoke.py"

FAKE_GENERATOR = r'''#!/usr/bin/env python3
import pathlib
import sys
args = sys.argv[1:]
if "--version" in args:
    print("fake generator 1.0")
    raise SystemExit(0)
out = pathlib.Path(next(x.split("=",1)[1] for x in args if x.startswith("--out-dir=")))
out.mkdir(exist_ok=True, parents=True)
(out / "init.h").write_text("extern int global_var;\nvoid test(void);\n")
(out / "func.c").write_text('#include "init.h"\nvoid test(void) { global_var += 2; }\n')
(out / "driver.c").write_text('#include <stdio.h>\n#include "init.h"\nint global_var = 40;\nint main(void) { test(); printf("%d\\n", global_var); return 0; }\n')
'''


@unittest.skipUnless(shutil.which("gcc"), "GCC needed for C baseline test")
class HarnessTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.generator = self.root / "generator"
        self.generator.write_text(FAKE_GENERATOR)
        self.generator.chmod(0o755)
        self.common = [
            sys.executable, str(HARNESS), "--generator", str(self.generator),
            "--count", "2", "--start-seed", "71", "--timeout", "10",
            "--memory-mb", "512",
        ]

    def invoke(self, *extra):
        return subprocess.run(
            [*self.common, "--out", str(self.root / "cases"), *extra],
            text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            timeout=70,
        )

    def test_baseline_reports_two_passing_cases(self):
        run = self.invoke()
        self.assertEqual(run.returncode, 0, run.stderr)
        summary = json.loads((self.root / "cases" / "summary.json").read_text())
        self.assertEqual(summary["generated"], 2)
        self.assertEqual(summary["oracle_valid"], 2)
        self.assertEqual(summary["counts"]["pass"], 2)
        self.assertEqual(summary["backend_accepted"], 0)
        self.assertFalse((self.root / "cases" / "seed-71").exists())

    def test_unsupported_does_not_count_as_success(self):
        codegen = self.root / "unsupported_codegen"
        codegen.write_text("#!/bin/sh\necho 'ABI must be signed 64-bit integer' >&2\nexit 1\n")
        codegen.chmod(0o755)
        run = self.invoke("--codegen", str(codegen))
        self.assertEqual(run.returncode, 0, run.stderr)
        summary = json.loads((self.root / "cases" / "summary.json").read_text())
        self.assertEqual(summary["counts"]["unsupported_construct"], 2)
        self.assertEqual(summary["counts"]["pass"], 0)
        self.assertEqual(summary["backend_accepted"], 0)
        self.assertEqual(summary["failure_signatures"], [{"status": "unsupported_construct", "reason": "EDG/Cranelift: ABI must be signed 64-bit integer", "count": 2}])
        self.assertTrue((self.root / "cases" / "seed-71" / "result.json").is_file())

    def test_unsupported_fails_strict_mode(self):
        codegen = self.root / "unsupported_codegen"
        codegen.write_text("#!/bin/sh\necho 'unsupported construct' >&2\nexit 1\n")
        codegen.chmod(0o755)
        run = self.invoke("--codegen", str(codegen), "--strict")
        self.assertEqual(run.returncode, 1)


    def test_gnu_statement_expression_is_unsupported_not_a_codegen_crash(self):
        codegen = self.root / "gnu_codegen"
        codegen.write_text(
            "#!/bin/sh\necho 'EDG IL lowering failed: GNU statement expression requires block-value semantics (M6)' >&2\nexit 1\n"
        )
        codegen.chmod(0o755)
        run = self.invoke("--codegen", str(codegen))
        self.assertEqual(run.returncode, 0, run.stderr)
        summary = json.loads((self.root / "cases" / "summary.json").read_text())
        self.assertEqual(summary["counts"]["unsupported_construct"], 2)
        self.assertEqual(summary["counts"]["codegen_failure"], 0)
        self.assertEqual(summary["backend_accepted"], 0)

    def test_actual_codegen_failure_is_not_treated_as_unsupported(self):
        codegen = self.root / "broken_codegen"
        codegen.write_text("#!/bin/sh\necho 'Cranelift unexpected verifier error' >&2\nexit 2\n")
        codegen.chmod(0o755)
        run = self.invoke("--codegen", str(codegen))
        self.assertEqual(run.returncode, 1, run.stderr)
        summary = json.loads((self.root / "cases" / "summary.json").read_text())
        self.assertEqual(summary["counts"]["codegen_failure"], 2)
        self.assertEqual(summary["counts"]["unsupported_construct"], 0)
        self.assertEqual(summary["failure_signatures"], [{"status": "codegen_failure", "reason": "EDG/Cranelift: Cranelift unexpected verifier error", "count": 2}])

    def test_failure_signature_prefers_actionable_diagnostic(self):
        codegen = self.root / "noisy_codegen"
        codegen.write_text(
            "#!/bin/sh\\n"
            "echo 'EDG IL lowering failed: unsupported EDG statement 42' >&2\\n"
            "echo 'wrapper failed for /tmp/build-12345/func.c' >&2\\n"
            "exit 1\\n"
        )
        codegen.chmod(0o755)
        run = self.invoke("--codegen", str(codegen))
        self.assertEqual(run.returncode, 0, run.stderr)
        summary = json.loads((self.root / "cases" / "summary.json").read_text())
        self.assertEqual(summary["counts"]["unsupported_construct"], 2)
        self.assertEqual(summary["failure_signatures"], [{
            "status": "unsupported_construct",
            "reason": "EDG/Cranelift: EDG IL lowering failed: unsupported EDG statement 42",
            "count": 2,
        }])

    def test_duplicate_seed_rejected(self):
        self.common += ["--seeds", "7,7"]
        run = self.invoke()
        self.assertEqual(run.returncode, 2)
        self.assertIn("duplicate seeds", run.stderr)


if __name__ == "__main__":
    unittest.main()
