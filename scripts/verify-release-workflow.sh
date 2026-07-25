#!/usr/bin/env bash
set -euo pipefail

workflow=".github/workflows/release.yml"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

[[ -f "${workflow}" ]] || fail "${workflow} is missing"

required_patterns=(
  '^permissions:'
  'contents: read'
  'Validate source and tag version'
  'Run complete source quality gate'
  'needs: \[release-metadata, source-gates, macos-app, windows-installer\]'
  'contents: write'
  'APPLE_CERTIFICATE'
  'APPLE_SIGNING_IDENTITY'
  'xcrun notarytool submit'
  'xcrun stapler validate'
  'result.status !== "Accepted"'
  'codesign --verify --deep --strict'
  'notarization=not-notarized'
  'npm run tauri:build --prefix apps/desktop -- --bundles nsis'
  'Smoke install and uninstall package'
  'Get-AuthenticodeSignature'
  'signing=unsigned'
  'Verify release asset checksums and provenance'
  'Public releases require a Developer ID signed macOS artifact'
  'Release tag .* must be annotated'
  'git merge-base --is-ancestor'
  'is not a reviewed merge commit'
  'Refusing to overwrite published assets'
  'gh release create'
  '--verify-tag'
)

for pattern in "${required_patterns[@]}"; do
  grep -Eq -- "${pattern}" "${workflow}" || fail "release workflow is missing required gate: ${pattern}"
done

publish_line="$(grep -n '^  publish-release:' "${workflow}" | cut -d: -f1)"
[[ -n "${publish_line}" ]] || fail "publish-release job is missing"
if tail -n "+${publish_line}" "${workflow}" | grep -Eq 'upload-artifact@'; then
  fail "artifact build or upload steps must not run inside the publication job"
fi

if grep -Eq 'gh release upload|--clobber' "${workflow}"; then
  fail "release assets must not be overwritten"
fi

echo "PASS: release workflow is tag/version gated, verifies both platforms, records signing provenance, and publishes immutably"
