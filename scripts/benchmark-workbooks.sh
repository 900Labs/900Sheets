#!/usr/bin/env bash
set -euo pipefail

mode="${1:-default}"
case "${mode}" in
  quick)
    cargo run --release -p sheets-core --example workbook_workloads -- --quick
    ;;
  default)
    cargo run --release -p sheets-core --example workbook_workloads
    ;;
  *)
    echo "Usage: $0 [quick|default]" >&2
    exit 2
    ;;
esac
