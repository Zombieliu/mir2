#!/usr/bin/env bash
# Deterministic Android compile/package gate for the native Android host.
#
# This script never installs or downloads toolchains. It validates the local
# Rust target and NDK first, then runs an offline Cargo check or cargo-apk
# package build. Set MIR2_ANDROID_MODE=package to build an APK.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MANIFEST="${SCRIPT_DIR}/Cargo.toml"
TOOLCHAIN="${MIR2_CLIENT_TOOLCHAIN:-1.95.0}"
TARGET="${MIR2_ANDROID_TARGET:-aarch64-linux-android}"
API_LEVEL="${MIR2_ANDROID_API_LEVEL:-26}"
MODE="${MIR2_ANDROID_MODE:-check}"

fail() {
  echo "[platform-android] error: $*" >&2
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || fail "required command '$1' is not on PATH"
}

case "${MODE}" in
  check|package) ;;
  *) fail "MIR2_ANDROID_MODE must be 'check' or 'package'" ;;
esac

require_command cargo
require_command rustup

rustup run "${TOOLCHAIN}" rustc --version >/dev/null 2>&1 \
  || fail "Rust toolchain '${TOOLCHAIN}' is unavailable; install it locally or set MIR2_CLIENT_TOOLCHAIN"

if ! rustup target list --installed --toolchain "${TOOLCHAIN}" | grep -Fxq "${TARGET}"; then
  fail "Rust target '${TARGET}' is not installed for '${TOOLCHAIN}'; run 'rustup target add --toolchain ${TOOLCHAIN} ${TARGET}'"
fi

NDK_HOME="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
SDK_ROOT="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}"
if [[ -z "${NDK_HOME}" && -n "${SDK_ROOT}" && -d "${SDK_ROOT}/ndk" ]]; then
  NDK_HOME="$(find "${SDK_ROOT}/ndk" -mindepth 1 -maxdepth 1 -type d -print | sort -V | tail -n 1)"
fi
if [[ -z "${NDK_HOME}" && "$(uname -s)" == "Darwin" && -d "${HOME}/Library/Android/sdk/ndk" ]]; then
  NDK_HOME="$(find "${HOME}/Library/Android/sdk/ndk" -mindepth 1 -maxdepth 1 -type d -print | sort -V | tail -n 1)"
fi
if [[ -z "${NDK_HOME}" && -d "${HOME}/Android/Sdk/ndk" ]]; then
  NDK_HOME="$(find "${HOME}/Android/Sdk/ndk" -mindepth 1 -maxdepth 1 -type d -print | sort -V | tail -n 1)"
fi
[[ -d "${NDK_HOME}" ]] || fail "Android NDK not found; set ANDROID_NDK_HOME or ANDROID_NDK_ROOT"

PREBUILT="$(find "${NDK_HOME}/toolchains/llvm/prebuilt" -mindepth 1 -maxdepth 1 -type d -print 2>/dev/null | head -n 1)"
[[ -d "${PREBUILT}" ]] || fail "NDK LLVM prebuilt directory not found below '${NDK_HOME}'"

case "${TARGET}" in
  aarch64-linux-android) target_env="AARCH64_LINUX_ANDROID" ;;
  x86_64-linux-android) target_env="X86_64_LINUX_ANDROID" ;;
  armv7-linux-androideabi) target_env="ARMV7_LINUX_ANDROIDEABI" ;;
  *) fail "unsupported Android target '${TARGET}' for this gate" ;;
esac

LINKER="${PREBUILT}/bin/${TARGET}${API_LEVEL}-clang"
AR="${PREBUILT}/bin/llvm-ar"
[[ -x "${LINKER}" || -x "${LINKER}.cmd" || -x "${LINKER}.exe" ]] \
  || fail "NDK linker '${LINKER}' is missing for API ${API_LEVEL}"
[[ -x "${AR}" || -x "${AR}.exe" ]] || fail "NDK llvm-ar '${AR}' is missing"

export "CARGO_TARGET_${target_env}_LINKER=${LINKER}"
export "CARGO_TARGET_${target_env}_AR=${AR}"

echo "[platform-android] ${MODE} ${TARGET} with Rust ${TOOLCHAIN}, NDK ${NDK_HOME}, API ${API_LEVEL}"
if [[ "${MODE}" == "package" ]]; then
  require_command cargo-apk
  cargo "+${TOOLCHAIN}" apk --manifest-path "${MANIFEST}" --target "${TARGET}" --release --locked --offline
else
  cargo "+${TOOLCHAIN}" check --manifest-path "${MANIFEST}" --target "${TARGET}" --locked --offline
fi
echo "[platform-android] ${MODE} gate passed"
