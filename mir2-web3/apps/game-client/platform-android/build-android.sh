#!/usr/bin/env bash
# Deterministic Android compile/package gate for the native GameActivity host.
#
# This script never installs toolchains. It builds the Rust cdylib with
# cargo-ndk, places it in Gradle's jniLibs tree, then packages the APK with the
# checked-in Gradle wrapper. MIR2_ANDROID_MODE=check stops after target check.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MANIFEST="${SCRIPT_DIR}/Cargo.toml"
ANDROID_PROJECT="${SCRIPT_DIR}/android"
JNI_ROOT="${ANDROID_PROJECT}/app/src/main/jniLibs"
NDK_OUTPUT="${SCRIPT_DIR}/target/android-jni"
TOOLCHAIN="${MIR2_CLIENT_TOOLCHAIN:-1.95.0}"
TARGET="${MIR2_ANDROID_TARGET:-aarch64-linux-android}"
ABI="${MIR2_ANDROID_ABI:-arm64-v8a}"
API_LEVEL="${MIR2_ANDROID_API_LEVEL:-31}"
MODE="${MIR2_ANDROID_MODE:-check}"
VARIANT="${MIR2_ANDROID_VARIANT:-debug}"
RUST_PROFILE="${MIR2_ANDROID_RUST_PROFILE:-release}"

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

case "${VARIANT}" in
  debug|release) ;;
  *) fail "MIR2_ANDROID_VARIANT must be 'debug' or 'release'" ;;
esac

case "${RUST_PROFILE}" in
  debug|release) ;;
  *) fail "MIR2_ANDROID_RUST_PROFILE must be 'debug' or 'release'" ;;
esac

[[ "${TARGET}" == "aarch64-linux-android" && "${ABI}" == "arm64-v8a" ]] \
  || fail "M0 supports only aarch64-linux-android / arm64-v8a"
[[ "${API_LEVEL}" =~ ^[0-9]+$ && "${API_LEVEL}" -ge 31 ]] \
  || fail "GameActivity M0 requires MIR2_ANDROID_API_LEVEL >= 31"

require_command cargo
require_command cargo-ndk
require_command rustup

rustup run "${TOOLCHAIN}" rustc --version >/dev/null 2>&1 \
  || fail "Rust toolchain '${TOOLCHAIN}' is unavailable; install it locally or set MIR2_CLIENT_TOOLCHAIN"

if ! rustup target list --installed --toolchain "${TOOLCHAIN}" | grep -Fxq "${TARGET}"; then
  fail "Rust target '${TARGET}' is not installed for '${TOOLCHAIN}'; run 'rustup target add --toolchain ${TOOLCHAIN} ${TARGET}'"
fi

SDK_ROOT="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}"
if [[ -z "${SDK_ROOT}" && "$(uname -s)" == "Darwin" && -d "${HOME}/Library/Android/sdk" ]]; then
  SDK_ROOT="${HOME}/Library/Android/sdk"
fi
if [[ -z "${SDK_ROOT}" && -d "${HOME}/Android/Sdk" ]]; then
  SDK_ROOT="${HOME}/Android/Sdk"
fi
[[ -d "${SDK_ROOT}" ]] || fail "Android SDK not found; set ANDROID_SDK_ROOT or ANDROID_HOME"

NDK_HOME="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
if [[ -z "${NDK_HOME}" && -d "${SDK_ROOT}/ndk" ]]; then
  NDK_HOME="$(find "${SDK_ROOT}/ndk" -mindepth 1 -maxdepth 1 -type d -print | sort -V | tail -n 1)"
fi
[[ -d "${NDK_HOME}" ]] || fail "Android NDK not found; set ANDROID_NDK_HOME or ANDROID_NDK_ROOT"

export ANDROID_SDK_ROOT="${SDK_ROOT}"
export ANDROID_NDK_HOME="${NDK_HOME}"

cd "${SCRIPT_DIR}"

echo "[platform-android] ${MODE} ${TARGET} with Rust ${TOOLCHAIN}, NDK ${NDK_HOME}, API ${API_LEVEL}"

ndk_args=(
  "+${TOOLCHAIN}"
  ndk
  --target "${ABI}"
  --platform "${API_LEVEL}"
)

if [[ "${MODE}" == "check" ]]; then
  cargo "${ndk_args[@]}" --manifest-path "${MANIFEST}" check --lib --locked --offline
  echo "[platform-android] target check passed"
  exit 0
fi

require_command java
[[ -x "${ANDROID_PROJECT}/gradlew" ]] || fail "checked-in Gradle wrapper is missing or not executable"

build_args=(build --lib --locked --offline)
gradle_task="assembleDebug"
apk_path="${ANDROID_PROJECT}/app/build/outputs/apk/debug/app-debug.apk"
if [[ "${RUST_PROFILE}" == "release" ]]; then
  build_args+=(--release)
fi
if [[ "${VARIANT}" == "release" ]]; then
  gradle_task="assembleRelease"
  apk_path="${ANDROID_PROJECT}/app/build/outputs/apk/release/app-release-unsigned.apk"
fi

staged_lib="${NDK_OUTPUT}/${ABI}/libmir2_platform_android.so"
native_lib="${JNI_ROOT}/${ABI}/libmir2_platform_android.so"
rm -f "${staged_lib}"
cargo "${ndk_args[@]}" --manifest-path "${MANIFEST}" --output-dir "${NDK_OUTPUT}" "${build_args[@]}"
[[ -f "${staged_lib}" ]] || fail "cargo-ndk did not produce '${staged_lib}'"
mkdir -p "${JNI_ROOT}/${ABI}"
rm -f "${native_lib}" "${JNI_ROOT}/${ABI}/libmir2_bevy_runtime.so"
cp "${staged_lib}" "${native_lib}"

(
  cd "${ANDROID_PROJECT}"
  ./gradlew --no-daemon "${gradle_task}"
)

[[ -f "${apk_path}" ]] || fail "Gradle did not produce '${apk_path}'"
if command -v shasum >/dev/null 2>&1; then
  apk_sha="$(shasum -a 256 "${apk_path}" | awk '{print $1}')"
else
  apk_sha="$(sha256sum "${apk_path}" | awk '{print $1}')"
fi

echo "[platform-android] APK: ${apk_path}"
echo "[platform-android] SHA-256: ${apk_sha}"
echo "[platform-android] package gate passed"
