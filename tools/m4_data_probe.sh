#!/usr/bin/env bash
# External data relocation link+execute regression; run with EDG environment
# already configured for the Rust crate's native build.
set -euo pipefail
cd "$(dirname "$0")/.."
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cargo run --quiet --features codegen --example m4_data_probe -- "$tmp/m4_probe.o"
gcc -std=c11 -Wall -Wextra -Werror tests/fixtures/m4_data_probe_driver.c "$tmp/m4_probe.o" -o "$tmp/m4_probe"
printf '0\n42\n' > "$tmp/expected"
"$tmp/m4_probe" > "$tmp/actual"
diff -u "$tmp/expected" "$tmp/actual"
echo "M4 external data ELF relocation probe passed"
