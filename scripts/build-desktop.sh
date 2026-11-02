#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

engine="${CONTAINER_ENGINE:-podman}"
image="${BUILDER_IMAGE:-docker.io/library/fedora:43}"
container="${BUILDER_CONTAINER:-pocketfiles-desktop-builder}"
bundles="${BUNDLE_TARGETS:-appimage}"
target_dir="${CARGO_TARGET_DIR:-/app/target/appimage}"
pull=0
clean=0

usage() {
  cat <<EOF
Build production desktop bundles in a persistent container.

The container is created once and reused, so system packages, the Cargo
registry, the npm cache, and Tauri's bundle tools are downloaded only on the
first run.

Usage: $(basename "$0") [options]

Options:
  --engine <name>       Container engine to use (default: podman)
  --image <ref>         Builder image (default: docker.io/library/fedora:43)
  --container <name>    Builder container name (default: pocketfiles-desktop-builder)
  --bundles <targets>   Comma-separated Tauri bundles (default: appimage)
  --pull                Pull the builder image before building
  --clean               Remove the builder container and exit
  -h, --help            Show this help

Environment:
  CONTAINER_ENGINE, BUILDER_IMAGE, BUILDER_CONTAINER,
  BUNDLE_TARGETS, CARGO_TARGET_DIR

Examples:
  $(basename "$0")
  $(basename "$0") --bundles appimage,deb,rpm
  $(basename "$0") --engine docker
  $(basename "$0") --clean
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --engine) engine="${2:?missing value for --engine}"; shift 2 ;;
    --image) image="${2:?missing value for --image}"; shift 2 ;;
    --container) container="${2:?missing value for --container}"; shift 2 ;;
    --bundles) bundles="${2:?missing value for --bundles}"; shift 2 ;;
    --pull) pull=1; shift ;;
    --clean) clean=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown option '$1'" >&2; usage >&2; exit 1 ;;
  esac
done

if ! command -v "$engine" >/dev/null 2>&1; then
  echo "error: '$engine' not found; install podman or set --engine docker" >&2
  exit 1
fi

if [[ "$clean" == 1 ]]; then
  "$engine" rm -f "$container" >/dev/null 2>&1 || true
  echo "removed builder container '$container'"
  exit 0
fi

if [[ "$pull" == 1 ]]; then
  "$engine" pull "$image"
fi

if ! "$engine" inspect "$container" >/dev/null 2>&1; then
  echo "creating builder container '$container' from $image"
  "$engine" run -d \
    --name "$container" \
    --network=host \
    -v "$root":/app:Z \
    -w /app \
    -e CARGO_TARGET_DIR="$target_dir" \
    "$image" sleep infinity >/dev/null
elif [[ "$("$engine" inspect -f '{{.State.Running}}' "$container")" != "true" ]]; then
  "$engine" start "$container" >/dev/null
fi

run() {
  "$engine" exec -w /app -e CARGO_TARGET_DIR="$target_dir" "$container" "$@"
}

if ! run test -f /root/.pocketfiles-deps; then
  echo "installing build dependencies (first run only)"
  run bash -lc '
    set -e
    dnf install -y --setopt=install_weak_deps=False \
      cargo rust gcc gcc-c++ make pkgconf-pkg-config \
      webkit2gtk4.1-devel openssl-devel curl wget file \
      libappindicator-gtk3-devel librsvg2-devel libxdo-devel nodejs npm
    dnf clean all
    touch /root/.pocketfiles-deps
  '
fi

echo "installing npm dependencies"
run npm install --prefer-offline --no-audit --no-fund

echo "building web assets"
run npm run web:build

echo "building desktop bundles: $bundles"
run npm run tauri -w troph-desktop -- build --bundles "$bundles"

echo
echo "artifacts:"
run bash -lc "find '$target_dir/release/bundle' -maxdepth 3 -type f \
  \( -name '*.AppImage' -o -name '*.deb' -o -name '*.rpm' \) \
  -printf '  %p\n' 2>/dev/null || true"
