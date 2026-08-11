#!/bin/bash
# Build gate for the Mir2 mobile shell (Android APK / iOS app).
#
# Requirements:
#   - Node + Capacitor (npm install in this directory)
#   - Android: Android SDK + JDK 21 (ANDROID_HOME or ~/Library/Android/sdk)
#   - iOS: macOS with Xcode + CocoaPods (ios build)
#
# Usage:
#   ./build-mobile.sh android    # assembleDebug APK
#   ./build-mobile.sh ios        # xcodebuild Debug (macOS only)

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
MOBILE="$ROOT/apps/mir2-mobile"
TARGET="${1:-android}"

echo "[mir2-mobile] npm deps"
(cd "$MOBILE" && npm ci >/dev/null 2>&1)

echo "[mir2-mobile] web build"
(cd "$MOBILE" && npm run build)

case "${TARGET}" in
  android)
    if [ ! -d "$MOBILE/android" ]; then
      (cd "$MOBILE" && npx cap add android)
    fi
    (cd "$MOBILE" && npx cap sync android)
    echo "[mir2-mobile] assembling APK"
    (cd "$MOBILE/android" && ./gradlew assembleDebug)
    APK="$MOBILE/android/app/build/outputs/apk/debug/app-debug.apk"
    echo "[mir2-mobile] APK: $APK"
    ;;
  ios)
    if [ ! -d "$MOBILE/ios" ]; then
      (cd "$MOBILE" && npx cap add ios)
    fi
    (cd "$MOBILE" && npx cap sync ios)
    echo "[mir2-mobile] building iOS (requires Xcode + CocoaPods)"
    (cd "$MOBILE/ios/App" && xcodebuild -workspace App.xcworkspace -scheme App \
      -configuration Debug -sdk iphonesimulator \
      -destination 'generic/platform=iOS Simulator' build CODE_SIGNING_ALLOWED=NO)
    ;;
  *)
    echo "unknown target: ${TARGET}" >&2
    exit 1
    ;;
esac

echo "[mir2-mobile] ${TARGET} gate passed"
