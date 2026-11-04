#!/bin/sh
# PROP-11: Rust -> TypeScript -> Rust schema round trip (foundation Requirement 15).
# Exits 1 on any sample that fails to parse or round-trips to different JSON, and prints the
# type name and the sample. Cases per type: PROPTEST_CASES (Rust) and FC_NUM_RUNS (TypeScript),
# default 256 each; CI sets 512 and Phase 10 runs 10,000.
set -eu

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"

SCHEMA_PARITY_DIR="${SCHEMA_PARITY_DIR:-$ROOT/target/schema-parity}"
PROPTEST_CASES="${PROPTEST_CASES:-256}"
FC_NUM_RUNS="${FC_NUM_RUNS:-256}"
export SCHEMA_PARITY_DIR PROPTEST_CASES FC_NUM_RUNS

rm -rf "$SCHEMA_PARITY_DIR"

echo "schema-parity 1/3: Rust writes samples"
cargo test -p dormbit-core --locked --test schema_parity -- --ignored --exact write_samples

echo "schema-parity 2/3: Zod checks the Rust samples and writes its own"
DORMBIT_PARITY=1 pnpm exec vitest run packages/schema/test/parity.test.ts

echo "schema-parity 3/3: Rust checks the TypeScript samples"
cargo test -p dormbit-core --locked --test schema_parity -- --ignored --exact read_samples

echo "schema-parity: OK ($PROPTEST_CASES Rust and $FC_NUM_RUNS TypeScript samples per type)"
