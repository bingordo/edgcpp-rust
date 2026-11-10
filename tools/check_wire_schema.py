#!/usr/bin/env python3
"""Check that every C++ EDG wire record matches Rust's expected arity.

This intentionally does not infer a shared schema from the other language:
the independently maintained call sites and decoder are compared in CI.
It is a guard until the native/Rust wire schema is generated from one source.
"""
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
CPP = (ROOT / "cpp" / "bridge.cpp").read_text(encoding="utf-8")
RUST = (ROOT / "src" / "lib.rs").read_text(encoding="utf-8")


def rust_arities():
    marker = "fn expected_wire_field_count(kind: u32)"
    if marker not in RUST:
        raise ValueError("Rust expected_wire_field_count function was removed")
    block = RUST.split(marker, 1)[1].split("unsafe fn collect_event", 1)[0]
    values = {}
    for match in re.finditer(r"(\d+)(?:\.\.=(\d+))?\s*=>\s*(\d+)", block):
        first = int(match[1])
        last = int(match[2]) if match[2] else first
        count = int(match[3])
        for kind in range(first, last + 1):
            if kind in values:
                raise ValueError(f"duplicate Rust wire kind {kind}")
            values[kind] = count
    if not values or min(values) != 1:
        raise ValueError("cannot locate Rust wire arity table")
    return values


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
