#!/usr/bin/env bash
set -euo pipefail

echo "=== 900Sheets Public Release Privacy Gate ==="
echo ""

echo "Checking for local paths, hostnames, secrets, and generated artifacts..."
echo ""

RELEASE_VERSION="0.4.0"

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

# Check authoritative release versions before packaging.
workspace_version="$(awk '
  /^\[workspace.package\]$/ { in_package = 1; next }
  /^\[/ { in_package = 0 }
  in_package && /^version = / { gsub(/[\" ]/, "", $3); print $3; exit }
' Cargo.toml)"
npm_version="$(node -p "require('./apps/desktop/package.json').version")"
npm_lock_version="$(node -p "require('./apps/desktop/package-lock.json').packages[''].version")"
tauri_version="$(node -p "require('./apps/desktop/src-tauri/tauri.conf.json').version")"

for version_entry in \
  "Cargo workspace:${workspace_version}" \
  "npm package:${npm_version}" \
  "npm lock:${npm_lock_version}" \
  "Tauri bundle:${tauri_version}"; do
  label="${version_entry%%:*}"
  value="${version_entry#*:}"
  if [[ "${value}" != "${RELEASE_VERSION}" ]]; then
    echo "FAIL: ${label} version is ${value}, expected ${RELEASE_VERSION}"
    exit 1
  fi
done

cargo_mismatches="$(cargo metadata --locked --no-deps --format-version 1 | node -e '
  const expected = process.argv[1]
  const metadata = JSON.parse(require("fs").readFileSync(0, "utf8"))
  const mismatches = metadata.packages
    .filter((pkg) => pkg.version !== expected)
    .map((pkg) => `${pkg.name}=${pkg.version}`)
  if (mismatches.length) process.stdout.write(mismatches.join("\n"))
' "${RELEASE_VERSION}")"
if [[ -n "${cargo_mismatches}" ]]; then
  echo "FAIL: Cargo package versions are not aligned with ${RELEASE_VERSION}:"
  echo "${cargo_mismatches}"
  exit 1
fi

for release_doc in README.md CHANGELOG.md docs/COMPATIBILITY.md docs/COMPATIBILITY_MATRIX.md docs/USER_GUIDE.md docs/RELEASING.md; do
  if ! grep -q "${RELEASE_VERSION}" "${release_doc}"; then
    echo "FAIL: ${release_doc} does not identify release ${RELEASE_VERSION}"
    exit 1
  fi
done

echo "PASS: Cargo, npm, Tauri, locks, and release documents align at ${RELEASE_VERSION}"

# Check for hardcoded personal local paths and hostnames.
if grep -r --exclude-dir=target --exclude-dir=node_modules --exclude-dir=dist \
  --exclude-dir=.git --exclude='verify-public-release.sh' \
  -E '(/Users/|/home/[^/[:space:]]+/|[A-Za-z]:\\Users\\)' "${SOURCE_PATHS[@]}" 2>/dev/null; then
  echo "FAIL: Found hardcoded local paths"
  exit 1
fi

if grep -rE --exclude-dir=target --exclude-dir=node_modules --exclude-dir=dist \
  --exclude-dir=.git --exclude='verify-public-release.sh' \
  -i '(^|[^[:alnum:].-])([[:alnum:]_-]+\.)+(local|lan|home)([^[:alnum:].-]|$)' \
  "${SOURCE_PATHS[@]}" 2>/dev/null; then
  echo "FAIL: Found a private-network hostname"
  exit 1
fi

# Check for secrets
if grep -rE --exclude-dir=target --exclude-dir=node_modules --exclude-dir=dist \
  --exclude-dir=.git --exclude='verify-public-release.sh' \
  -i '(^|[^[:alnum:]_])(api_key|secret_key|password)[[:space:]]*=' "${SOURCE_PATHS[@]}" 2>/dev/null; then
  echo "FAIL: Found potential secrets"
  exit 1
fi

if grep -rE --exclude-dir=target --exclude-dir=node_modules --exclude-dir=dist \
  --exclude-dir=.git --exclude='verify-public-release.sh' \
  '(-----BEGIN [A-Z ]*PRIVATE KEY-----|AKIA[0-9A-Z]{16}|gh[pousr]_[A-Za-z0-9_]{20,}|sk-(proj-)?[A-Za-z0-9_-]{20,}|Bearer[[:space:]]+[A-Za-z0-9._~-]{20,})' \
  "${SOURCE_PATHS[@]}" 2>/dev/null; then
  echo "FAIL: Found a credential-shaped value or private key"
  exit 1
fi

generated_artifact="$(git ls-files | grep -E '(^|/)\.DS_Store$|\.(pem|key|p12|dmg|msi|AppImage|deb)$' | head -n 1 || true)"
if [[ -n "${generated_artifact}" ]]; then
  echo "FAIL: Found sensitive or generated artifact ${generated_artifact}"
  exit 1
fi

echo "PASS: No local paths, secrets, or sensitive artifacts found"

signature_verifications="$(grep -c 'codesign --verify --deep --strict' .github/workflows/release.yml || true)"
if ! grep -q -- '--keepParent' .github/workflows/release.yml || \
  ! grep -q 'test -x' .github/workflows/release.yml || \
  ! grep -q 'macos.zip' .github/workflows/release.yml || \
  ! grep -q 'codesign --force --deep --sign -' .github/workflows/release.yml || \
  [[ "${signature_verifications}" -lt 2 ]]; then
  echo "FAIL: Release workflow must sign the complete app, verify it before and after archive, and preserve executable permission"
  exit 1
fi

echo "PASS: Release workflow signs and verifies the complete app before and after archive"
echo "=== Public release checks passed ==="
