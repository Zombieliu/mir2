#!/usr/bin/env bash
# UI-only specimen captures. Explicit separate package; never logs into a server.
set -euo pipefail
task_script_dir="$(cd "$(dirname "$0")" && pwd)"
task_adb="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}/platform-tools/adb"
task_output="${1:?Usage: capture-ui-preview.sh OUTPUT_DIRECTORY}"
task_package="com.mir2.web3.uipreview"
[[ -x "$task_adb" ]] || { echo "adb missing" >&2; exit 1; }
"$task_adb" shell pm path "$task_package" | grep -q '^package:' || { echo "Install uiPreview APK first" >&2; exit 1; }
mkdir -p "$task_output"
git -C "$task_script_dir" rev-parse HEAD > "$task_output/source-head.txt"
git -C "$task_script_dir" status --short > "$task_output/source-status.txt"
"$task_adb" shell getprop ro.build.fingerprint > "$task_output/device.txt"
for task_scene in login empty-roster roster create password safekey delete-character connecting starting disconnected hud inventory character skills quests options platform menu gameshop npcshop mail bigmap storage group guild trade chat-settings npc death chat help; do
  "$task_adb" shell am force-stop "$task_package"
  "$task_adb" shell am start -W -n "$task_package/com.mir2.web3.MainActivity" --es ui_scene "$task_scene" > "$task_output/$task_scene-launch.txt"
  task_pid="$("$task_adb" shell pidof "$task_package" | tr -d '\r')"
  [[ -n "$task_pid" ]] || { echo "$task_scene did not stay running" >&2; exit 1; }
  task_ready=0
  for task_attempt in {1..20}; do
    if "$task_adb" logcat -d --pid="$task_pid" | grep "ANDROID_UI_PREVIEW_READY scene=$task_scene" > /dev/null; then task_ready=1; break; fi
    sleep 1
  done
  [[ "$task_ready" == 1 ]] || { echo "$task_scene never became ready" >&2; exit 1; }
  sleep 2
  "$task_adb" exec-out screencap -p > "$task_output/$task_scene.png"
  "$task_adb" logcat -d --pid="$task_pid" > "$task_output/$task_scene-logcat.txt"
  if grep -E 'panicked at|FATAL EXCEPTION|PathNotFound' "$task_output/$task_scene-logcat.txt"; then
    echo "$task_scene has runtime errors" >&2; exit 1
  fi
  echo "$task_scene captured"
done
echo "31 offline specimens captured; NOT live, physical-device or full interaction acceptance."
