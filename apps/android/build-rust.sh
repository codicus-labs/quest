#!/usr/bin/env bash
# Builds libquest_mobile.so for Android and generates its Kotlin bindings.
# Called by Gradle (task cargoBuild) and usable on its own. Needs the tools from mise.toml,
# the NDK (see README) and `rustup target add aarch64-linux-android x86_64-linux-android`.
#
# Usage: build-rust.sh [<jni-libs-dir> <kotlin-dir>]   (default: app/build/rust/{jniLibs,kotlin})
#   <jni-libs-dir>/<abi>/libquest_mobile.so   for each ABI in QUEST_ANDROID_ABIS
#   <kotlin-dir>/uniffi/quest_mobile/quest_mobile.kt
# QUEST_ANDROID_ABIS: space-separated, default "arm64-v8a x86_64" (phones, Apple-Silicon and Intel
# emulators). Each needs `rustup target add` for its Rust target. arm64-v8a is always built: the
# bindings are generated from it.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
jni_libs="${1:-$here/app/build/rust/jniLibs}"
kotlin="${2:-$here/app/build/rust/kotlin}"
sdk="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$sdk/ndk/30.0.16248370}"

if [ ! -d "$ANDROID_NDK_HOME" ]; then
  echo "error: NDK not found at $ANDROID_NDK_HOME. Install it with: sdkmanager \"ndk;30.0.16248370\" (or set ANDROID_NDK_HOME)." >&2
  exit 1
fi

if ! command -v cargo-ndk >/dev/null; then
  echo "error: cargo-ndk not found. Run 'mise install' in apps/android and build from a shell where mise is active." >&2
  exit 1
fi

abis="${QUEST_ANDROID_ABIS:-arm64-v8a x86_64}"
case " $abis " in *" arm64-v8a "*) ;; *) abis="arm64-v8a $abis" ;; esac
targets=()
for abi in $abis; do targets+=(--target "$abi"); done

cd "$root"
# Profile mobile-release (root Cargo.toml): LTO, one codegen unit, opt-level "s".
# Unstripped on purpose: UniFFI reads its metadata from the library's symbols. The Android
# Gradle plugin strips the copy that goes into the APK.
cargo ndk "${targets[@]}" --platform 31 --output-dir "$jni_libs" \
  build --locked --package quest-mobile --lib --profile mobile-release

cargo run --locked --quiet --package quest-mobile --features bindgen --bin uniffi-bindgen -- \
  generate --library "${CARGO_TARGET_DIR:-$root/target}/aarch64-linux-android/mobile-release/libquest_mobile.so" \
  --language kotlin --out-dir "$kotlin" --no-format
