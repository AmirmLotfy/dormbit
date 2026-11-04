#!/usr/bin/env bash
# Downloads a pinned gitleaks release, verifies its SHA-256, and scans the full Git history
# (foundation Requirement 24.6). Used by the CI `security` job on macos-15 (arm64).
set -euo pipefail

VERSION="8.30.1"
# SHA-256 of gitleaks_${VERSION}_darwin_arm64.tar.gz from the release's checksums.txt.
EXPECTED="b40ab0ae55c505963e365f271a8d3846efbc170aa17f2607f13df610a9aeb6a5"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

arch="$(uname -m)"
case "$arch" in
  arm64 | aarch64) asset="gitleaks_${VERSION}_darwin_arm64.tar.gz" ;;
  x86_64) echo "this runner is x86_64; update the checksum in scripts/ci/gitleaks.sh" >&2; exit 1 ;;
  *) echo "unsupported arch: $arch" >&2; exit 1 ;;
esac

url="https://github.com/gitleaks/gitleaks/releases/download/v${VERSION}/${asset}"
curl -fsSL "$url" -o "$tmp/gitleaks.tar.gz"

actual="$(shasum -a 256 "$tmp/gitleaks.tar.gz" | awk '{print $1}')"
if [ "$actual" != "$EXPECTED" ]; then
  echo "gitleaks checksum mismatch:" >&2
  echo "  expected $EXPECTED" >&2
  echo "  actual   $actual" >&2
  exit 1
fi

tar -xzf "$tmp/gitleaks.tar.gz" -C "$tmp" gitleaks
"$tmp/gitleaks" version
"$tmp/gitleaks" git --redact --no-banner --exit-code 1 .
echo "gitleaks: no leaks in the full history"
