#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
file="$root/VERSION"
version="$(cat "$file")"
prefix="${version%.*}"
build="${version##*.}"

printf '%s.%d' "$prefix" "$((build + 1))" > "$file"
node "$root/scripts/sync-version.mjs"
