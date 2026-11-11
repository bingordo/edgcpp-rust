#!/usr/bin/env python3
"""Check that every C++ EDG wire record matches Rust's expected arity.

Compare each C++ emission site's field count with the canonical record arities.
Run gen_wire_schema.py --check first to verify generated C++/Rust views.
"""
from pathlib import Path
import json
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
CPP = (ROOT / "cpp" / "bridge.cpp").read_text(encoding="utf-8")
# Named C++ protocol constants now reside in the generated include. Appending
# it does not affect line numbers reported for native emission call sites.
CPP += "\n" + (ROOT / "cpp" / "wire_schema_generated.inc").read_text(encoding="utf-8")
SCHEMA = json.loads((ROOT / "schema" / "wire_schema.json").read_text(encoding="utf-8"))


def rust_arities():
    # The CI generator --check verifies Rust's generated function matches
    # this same canonical arity table before the cross-language site audit.
    return {int(k): v for k, v in SCHEMA["record_arities"].items()}


def fields_and_end(source, start):
    """Read an initializer list and count top-level elements, not commas in calls."""
    assert source[start] == "{"
    pos = start + 1
    brace = 1
    paren = bracket = 0
    quote = None
    escaped = False
    field_started = False
    count = 0
    while pos < len(source):
        c = source[pos]
        pos += 1
        if quote is not None:
            if escaped:
                escaped = False
            elif c == "\\":
                escaped = True
            elif c == quote:
                quote = None
            continue
        if c in ('"', "'"):
            quote = c
            field_started = True
        elif c == "{":
            brace += 1
            field_started = True
        elif c == "}":
            brace -= 1
            if brace == 0:
                return count + int(field_started), pos
        elif c == "(":
            paren += 1
            field_started = True
        elif c == ")":
            paren -= 1
        elif c == "[":
            bracket += 1
            field_started = True
        elif c == "]":
            bracket -= 1
        elif c == "," and brace == 1 and paren == 0 and bracket == 0:
            count += int(field_started)
            field_started = False
        elif not c.isspace():
            field_started = True
    raise ValueError("unterminated C++ emit_record initializer")


def cpp_calls():
    names = {
        match[1]: int(match[2])
        for match in re.finditer(r"\b(RECORD_[A-Z_]+)\s*=\s*(\d+)", CPP)
    }
    calls = []
    for match in re.finditer(r"\bemit_record\s*\(", CPP):
        comma = CPP.find(",", match.end())
        if comma < 0:
            raise ValueError("malformed emit_record call")
        expression = CPP[match.end():comma].strip()
        pos = comma + 1
        while pos < len(CPP) and CPP[pos].isspace():
            pos += 1
        # The C++ function definition is not an emission call.
        if pos >= len(CPP) or CPP[pos] != "{":
            continue
        if expression.isdecimal():
            kind = int(expression)
        elif expression in names:
            kind = names[expression]
        else:
            raise ValueError(f"unrecognized native record kind: {expression!r}")
        count, _ = fields_and_end(CPP, pos)
        line = CPP.count("\n", 0, match.start()) + 1
        calls.append((line, kind, count, expression))
    return calls


def main():
    arities = rust_arities()
    calls = cpp_calls()
    problems = []
    seen = set()
    for line, kind, count, expression in calls:
        if expression == "RECORD_INCOMPLETE_SNAPSHOT":
            if kind != 0 or count != 0:
                problems.append("malformed snapshot sentinel must be kind 0 with no fields")
            continue
        expected = arities.get(kind)
        if expected is None:
            problems.append(f"cpp/bridge.cpp:{line}: unknown wire kind {kind}")
        elif count != expected:
            problems.append(
                f"cpp/bridge.cpp:{line}: record {kind} has {count} fields "
                f"(Rust decoder requires {expected})"
            )
        seen.add(kind)
    for kind in sorted(set(arities) - seen):
        problems.append(f"Rust wire kind {kind} has no C++ emission site")
    for problem in problems:
        print(problem, file=sys.stderr)
    if problems:
        return 1
    print(f"Wire schema verified: {len(arities)} record kinds, "
          f"{len(calls)} C++ emission sites")
    return 0


if __name__ == "__main__":
    sys.exit(main())
