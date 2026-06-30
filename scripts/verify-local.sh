#!/usr/bin/env bash
set -euo pipefail

echo "=== 900Sheets Local Quality Gate ==="
echo ""

run_check() {
  local label="$1"
  shift
  echo "${label}"
  if "$@"; then
    echo "   PASS"
  else
    echo "   FAIL"
    exit 1
  fi
  echo ""
}

run_check "1. cargo fmt --check" cargo fmt --all -- --check
run_check "2. cargo clippy" cargo clippy --workspace --all-targets -- -D warnings
run_check "3. cargo test" cargo test --workspace
run_check "4. npm install" npm install --prefix apps/desktop --silent
run_check "5. npm run check" npm run check --prefix apps/desktop
run_check "6. npm run build" npm run build --prefix apps/desktop

echo "=== All checks passed ==="
