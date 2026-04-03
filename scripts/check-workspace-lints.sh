#!/usr/bin/env bash
set -euo pipefail

missing=0

while IFS= read -r manifest; do
  if ! rg -q '^\[lints\]$' "$manifest" || ! rg -q '^workspace = true$' "$manifest"; then
    echo "missing workspace lint opt-in: $manifest" >&2
    missing=1
  fi
done < <(
  cargo metadata --no-deps --format-version 1 \
    | rg -o '"manifest_path":"[^"]+"' \
    | sed 's/"manifest_path":"//; s/"$//'
)

if [[ "$missing" -ne 0 ]]; then
  exit 1
fi
