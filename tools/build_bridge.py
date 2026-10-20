#!/usr/bin/env python3
"""Compile the bridge with exactly the same EDG configuration as libedgcpfe.

This is essential: the IL structures contain conditional fields controlled
by many EDG macro definitions. Guessing compile flags would corrupt offsets.
"""
import json
import pathlib
import shlex
import subprocess
import sys

if len(sys.argv) != 5:
    sys.exit("usage: build_bridge.py COMPILE_COMMANDS SOURCE_DIR BRIDGE OUTPUT")
commands_file, source_dir, bridge, output = sys.argv[1:]
commands_file = pathlib.Path(commands_file).resolve()
source_dir = pathlib.Path(source_dir).resolve()
bridge = pathlib.Path(bridge).resolve()
output = pathlib.Path(output).resolve()
entries = json.loads(commands_file.read_text())
matches = [
    e for e in entries
    if "edgcpfe.dir" in e.get("command", " ".join(e.get("arguments", [])))
    and pathlib.Path(e["file"]).name == "cfe.c"
]
if len(matches) != 1:
    sys.exit("Expected exactly one edgcpfe cfe.c compile command; configure with "
             "-DCMAKE_EXPORT_COMPILE_COMMANDS=ON -DEDG_BUILD_CALLABLE_LIBRARY=ON")
entry = matches[0]
args = entry.get("arguments") or shlex.split(entry["command"])
original = pathlib.Path(entry["file"]).resolve()
def bridge_arguments(args, original):
    """Keep compilation settings, not the original source or output artifacts."""
    result = []
    skip_next = False
    for arg in args:
        if skip_next:
            skip_next = False
            continue
        if arg in ("-o", "-MF", "-MT", "-MQ", "-include-pch"):
            skip_next = True
            continue
        if arg in ("-MD", "-MMD", "-MP", "-MG", "-M", "-MM", "-E", "-S"):
            continue
        if arg.startswith(("-MF", "-MT", "-MQ")) and len(arg) > 3:
            continue
        if arg == "-c":
            continue
        candidate = pathlib.Path(arg)
        if candidate == original or (
            not arg.startswith("-")
            and (pathlib.Path(entry.get("directory", str(source_dir))) / candidate).resolve() == original
        ):
            continue
        result.append(arg)
    return result


new_args = bridge_arguments(args, original)
# Preserve EDG's -D and -I switches, plus every other required compiler flag.
new_args += ["-I", str(source_dir / "src"), "-o", str(output), str(bridge)]
print("Compiling bridge with EDG's recorded C++ compiler configuration", flush=True)
subprocess.run(new_args, cwd=entry.get("directory", str(source_dir)), check=True)
