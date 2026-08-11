#!/bin/bash
# Android compile gate for the native Android host.
#
# Requires (macOS host):
#   rustup target add --toolchain 1.95.0 aarch64-linux-android
#   Android NDK (clang + llvm-ar). Point ANDROID_NDK_HOME at the NDK root or
#   let the script discover ~/Library/Android/sdk/ndk/<version>.
#
# Real-device lifecycle/touch/GPU/memory/network gates are later M4 milestones.

set -euo pipefail

TOOLCHAIN="${MIR2_CLIENT_TOOLCHAIN:-1.95.0}"
TARGET="${MIR2_ANDROID_TARGET:-aarch64-linux-android}"

NDK_HOME="${ANDROID_NDK_HOME:-}"
if [[ -z "${NDK_HOME}" ]]; then
  NDK_HOME="$(ls -d "$HOME/Library/Android/sdk/ndk/"*/ 2>/dev/null | tail -1 | sed 's#/$##')"
fi
if [[ -z "${NDK_HOME}" || ! -d "${NDK_HOME}" ]]; then
  echo "Android NDK not found. Set ANDROID_NDK_HOME." >&2
  exit 1
fi

# API-level-suffixed clang, e.g. aarch64-linux-android26-clang.
API_LEVEL="${MIR2_ANDROID_API_LEVEL:-26}"
PREBUILT="$(ls -d "${NDK_HOME}/toolchains/llvm/prebuilt/"*/ | head -1)"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="${PREBUILT}/bin/${TARGET}${API_LEVEL}-clang"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_AR="${PREBUILT}/bin/llvm-ar"
# armv7 uses the same clang naming (armv7a prefix) but is out of scope for the
# first gate; x86_64 emulator target follows the same pattern.
export CC_aarch64_linux_android="${CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER}"
export AR_aarch64_linux_android="${CARGO_TARGET_AARCH64_LINUX_ANDROID_AR}"

echo "[platform-android] compile gate for ${TARGET} (${TOOLCHAIN}, NDK ${NDK_HOME})"
cargo "+${TOOLCHAIN}" check \
  --manifest-path apps/game-client/platform-android/Cargo.toml \
  --target "${TARGET}"

echo "[platform-android] gate passed"
