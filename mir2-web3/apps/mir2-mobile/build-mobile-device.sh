#!/bin/bash
# Mobile device gate: build, install, launch + verify the Capacitor shell on a
# real device or emulator.
#
# Android:
#   ./build-mobile-device.sh android            # emulator (Pixel_5_API_31 AVD)
#   MIR2_ANDROID_SERIAL=XXXX build-mobile-device.sh android   # real device
#
# iOS (macOS only):
#   ./build-mobile-device.sh ios                # builds Debug simulator app
#
# The game loads the deployed web origin in a full-screen Capacitor WebView
# (native PWA-style app); the authoritative gateway WS is injected via query.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
MOBILE="$ROOT/apps/mir2-mobile"
ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
export ANDROID_HOME
export PATH="$ANDROID_HOME/platform-tools:$PATH"
TARGET="${1:-android}"

echo "[mir2-mobile] web build"
(cd "$MOBILE" && npm run build)

case "${TARGET}" in
  android)
    export JAVA_HOME="${MIR2_JAVA_HOME:-/opt/homebrew/opt/openjdk21-local/Contents/Home}"
    echo "[mir2-mobile] assembling APK (JDK $("$JAVA_HOME/bin/java" -version 2>&1 | head -1))"
    (cd "$MOBILE/android" && ./gradlew assembleDebug)
    APK="$MOBILE/android/app/build/outputs/apk/debug/app-debug.apk"

    SERIAL="${MIR2_ANDROID_SERIAL:-}"
    if [[ -z "$SERIAL" ]]; then
      echo "[mir2-mobile] no MIR2_ANDROID_SERIAL; booting Pixel_5_API_31 emulator"
      nohup "$ANDROID_HOME/emulator/emulator" -avd Pixel_5_API_31 \
        -no-window -no-audio -no-boot-anim -gpu swiftshader_indirect \
        > /tmp/mir2-emulator.log 2>&1 &
      adb wait-for-device
      until [[ "$(adb shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" == "1" ]]; do sleep 5; done
      echo "[mir2-mobile] emulator booted"

      CONNECTED_SERIALS="$(adb devices | awk 'NR > 1 && $2 == "device" { print $1 }')"
      CONNECTED_COUNT="$(printf '%s\n' "$CONNECTED_SERIALS" | awk 'NF { count += 1 } END { print count + 0 }')"
      if [[ "$CONNECTED_COUNT" != "1" ]]; then
        echo "expected exactly one Android device after emulator boot; set MIR2_ANDROID_SERIAL" >&2
        exit 1
      fi
      SERIAL="$CONNECTED_SERIALS"
    fi

    ADB=(adb -s "$SERIAL")
    "${ADB[@]}" wait-for-device

    echo "[mir2-mobile] installing $APK"
    "${ADB[@]}" install -r "$APK"
    echo "[mir2-mobile] launching com.obelisklabs.mir2"
    "${ADB[@]}" shell am start -n com.obelisklabs.mir2/.MainActivity
    sleep 8
    echo "[mir2-mobile] process:"
    "${ADB[@]}" shell "ps -A | grep mir2" || { echo "FAILED: app not running" >&2; exit 1; }
    echo "[mir2-mobile] Android device gate passed"
    ;;
  ios)
    (cd "$MOBILE/ios/App" && xcodebuild -workspace App.xcworkspace -scheme App \
      -configuration Debug -sdk iphonesimulator \
      -destination 'generic/platform=iOS Simulator' build CODE_SIGNING_ALLOWED=NO)
    echo "[mir2-mobile] iOS simulator build passed"
    ;;
  *)
    echo "unknown target: ${TARGET}" >&2
    exit 1
    ;;
esac
