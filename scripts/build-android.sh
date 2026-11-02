#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
android_dir="$root/apps/android"
jni_dir="$android_dir/app/src/main/jniLibs"
abi="arm64-v8a"
build_type="debug"
install=1
launch=0
web=1
serial=""

usage() {
  cat <<EOF
Build and install the Android app.

Steps: build the web UI, cross-compile libpocketfiles.so with cargo-ndk,
build the APK with Gradle, then install it with adb.

Usage: $(basename "$0") [options]

Options:
  --release         Build the release APK instead of debug (signing required)
  --no-install      Build only; do not install on a device
  --no-web          Skip rebuilding the web UI (reuse apps/web/dist)
  --launch          Launch the app after installing
  --serial <id>     Target a specific adb device (see: adb devices)
  -h, --help        Show this help

Environment:
  ANDROID_HOME / ANDROID_SDK_ROOT   Android SDK (falls back to local.properties)
  ANDROID_NDK_HOME / NDK_HOME       Android NDK (falls back to \$SDK/ndk/*)

Examples:
  $(basename "$0")
  $(basename "$0") --release
  $(basename "$0") --no-install
  $(basename "$0") --serial emulator-5554 --launch
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release) build_type="release"; shift ;;
    --no-install) install=0; shift ;;
    --no-web) web=0; shift ;;
    --launch) launch=1; shift ;;
    --serial) serial="${2:?missing value for --serial}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown option '$1'" >&2; usage >&2; exit 1 ;;
  esac
done

if [[ "$launch" == 1 && "$install" == 0 ]]; then
  echo "error: --launch requires installation (remove --no-install)" >&2
  exit 1
fi

# Resolve the Android SDK.
sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
if [[ -z "$sdk" && -f "$android_dir/local.properties" ]]; then
  sdk="$(sed -n 's/^sdk\.dir=//p' "$android_dir/local.properties")"
fi
if [[ -z "$sdk" || ! -d "$sdk" ]]; then
  echo "error: Android SDK not found; set ANDROID_HOME or sdk.dir in apps/android/local.properties" >&2
  exit 1
fi

# Resolve the NDK.
ndk="${ANDROID_NDK_HOME:-${NDK_HOME:-${ANDROID_NDK_ROOT:-}}}"
if [[ -z "$ndk" && -d "$sdk/ndk" ]]; then
  ndk="$(find "$sdk/ndk" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -n 1)"
fi
if [[ -z "$ndk" || ! -d "$ndk" ]]; then
  echo "error: Android NDK not found; install it under $sdk/ndk or set ANDROID_NDK_HOME" >&2
  exit 1
fi

export ANDROID_HOME="$sdk"
export ANDROID_SDK_ROOT="$sdk"
export ANDROID_NDK_HOME="$ndk"
export NDK_HOME="$ndk"
export ANDROID_NDK_ROOT="$ndk"
export PATH="$sdk/platform-tools:$PATH"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo not found; install the Rust toolchain" >&2
  exit 1
fi
if ! cargo ndk --version >/dev/null 2>&1; then
  echo "error: cargo-ndk not found; install it with: cargo install cargo-ndk" >&2
  exit 1
fi
if ! command -v java >/dev/null 2>&1; then
  echo "error: java not found; install JDK 17 or newer" >&2
  exit 1
fi

if command -v rustup >/dev/null 2>&1; then
  rustup target add aarch64-linux-android >/dev/null
fi

if [[ "$web" == 1 ]]; then
  echo "building web assets"
  npm --prefix "$root" run web:build
fi

echo "cross-compiling libpocketfiles.so for $abi"
cargo ndk -t "$abi" -o "$jni_dir" build -p pocketfiles-android --release

task="assembleDebug"
[[ "$build_type" == "release" ]] && task="assembleRelease"

echo "building $build_type APK"
(cd "$android_dir" && ./gradlew "$task")

apk_dir="$android_dir/app/build/outputs/apk/$build_type"
apk="$(find "$apk_dir" -maxdepth 1 -type f -name '*.apk' | sort | tail -n 1)"
if [[ -z "$apk" ]]; then
  echo "error: no APK produced under $apk_dir" >&2
  exit 1
fi

echo
echo "APK: $apk"

if [[ "$install" == 0 ]]; then
  exit 0
fi

adb="$(command -v adb || true)"
if [[ -z "$adb" && -x "$sdk/platform-tools/adb" ]]; then
  adb="$sdk/platform-tools/adb"
fi
if [[ -z "$adb" ]]; then
  echo "error: adb not found; install Android platform-tools" >&2
  exit 1
fi

if [[ -z "$serial" ]]; then
  mapfile -t devices < <("$adb" devices | awk 'NR > 1 && $2 == "device" { print $1 }')
  if [[ "${#devices[@]}" -eq 0 ]]; then
    echo "error: no Android device found; connect one or start an emulator (adb devices)" >&2
    exit 1
  elif [[ "${#devices[@]}" -gt 1 ]]; then
    echo "error: multiple devices connected; choose one with --serial <id>:" >&2
    printf '  %s\n' "${devices[@]}" >&2
    exit 1
  fi
  serial="${devices[0]}"
fi

if [[ "$apk" == *-unsigned.apk ]]; then
  echo "error: release APK is unsigned; configure signing before installing" >&2
  echo "       built: $apk" >&2
  exit 1
fi

echo "installing on $serial"
"$adb" -s "$serial" install -r -d "$apk"

if [[ "$launch" == 1 ]]; then
  echo "launching com.sixdaggers.pocketfiles"
  "$adb" -s "$serial" shell am start -n com.sixdaggers.pocketfiles/.MainActivity >/dev/null
fi

echo "done"
