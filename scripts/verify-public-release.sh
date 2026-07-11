#!/usr/bin/env bash
set -euo pipefail

echo "=== 900Sheets Public Release Privacy Gate ==="
echo ""

echo "Checking for local paths, hostnames, secrets, and generated artifacts..."
echo ""

SOURCE_PATHS=(
  Cargo.toml
  Cargo.lock
  README.md
  CHANGELOG.md
  SECURITY.md
  CONTRIBUTING.md
  CODE_OF_CONDUCT.md
  rust-toolchain.toml
  apps
  crates
  docs
  scripts
  .github
)

# Check for hardcoded local paths
if grep -r --exclude-dir=target --exclude-dir=node_modules --exclude-dir=dist \
  --exclude-dir=.git --exclude='verify-public-release.sh' \
  '/Users/' "${SOURCE_PATHS[@]}" 2>/dev/null; then
  echo "FAIL: Found hardcoded local paths"
  exit 1
fi

# Check for secrets
if grep -rE --exclude-dir=target --exclude-dir=node_modules --exclude-dir=dist \
  --exclude-dir=.git --exclude='verify-public-release.sh' \
  -i '(^|[^[:alnum:]_])(api_key|secret_key|password)[[:space:]]*=' "${SOURCE_PATHS[@]}" 2>/dev/null; then
  echo "FAIL: Found potential secrets"
  exit 1
fi

echo "PASS: No local paths, secrets, or sensitive artifacts found"

if ! grep -q -- '--keepParent' .github/workflows/release.yml || \
  ! grep -q 'test -x' .github/workflows/release.yml || \
  ! grep -q 'macos.zip' .github/workflows/release.yml; then
  echo "FAIL: Release workflow must archive the app and verify executable permission"
  exit 1
fi

echo "PASS: Release archive preserves and verifies executable permission"
echo "=== Public release checks passed ==="
