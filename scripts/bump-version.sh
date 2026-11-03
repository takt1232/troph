#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/bump-version.sh <--major | --minor | --patch | --build>

Increment one component of the four-part VERSION file and sync it to the
Cargo workspace and npm package versions.

  -M, --major   increment major; reset minor, patch, and build
  -m, --minor   increment minor; reset patch and build
  -p, --patch   increment patch; reset build
  -b, --build   increment build
  -h, --help    show this help
EOF
}

if [[ $# -ne 1 ]]; then
  usage >&2
  exit 2
fi

case "$1" in
  -M|--major) mode=major ;;
  -m|--minor) mode=minor ;;
  -p|--patch) mode=patch ;;
  -b|--build) mode=build ;;
  -h|--help) usage; exit 0 ;;
  *) usage >&2; exit 2 ;;
esac

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
file="$root/VERSION"

if [[ ! -f "$file" ]]; then
  printf 'error: VERSION file not found at %s\n' "$file" >&2
  exit 1
fi

version="$(tr -d '[:space:]' < "$file")"

if [[ ! "$version" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
  printf 'error: VERSION must be four non-negative numeric components, found "%s"\n' "$version" >&2
  exit 1
fi

major="${BASH_REMATCH[1]}"
minor="${BASH_REMATCH[2]}"
patch="${BASH_REMATCH[3]}"
build="${BASH_REMATCH[4]}"

case "$mode" in
  major)
    major=$((10#$major + 1))
    minor=0
    patch=0
    build=0
    ;;
  minor)
    minor=$((10#$minor + 1))
    patch=0
    build=0
    ;;
  patch)
    patch=$((10#$patch + 1))
    build=0
    ;;
  build)
    build=$((10#$build + 1))
    ;;
esac

printf '%s.%s.%s.%s\n' "$major" "$minor" "$patch" "$build" > "$file"
node "$root/scripts/sync-version.mjs"
