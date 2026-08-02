#!/bin/sh
set -eu

test -f /run/ai-live/healthy
test -f /run/ai-live/ffmpeg.pid
kill -0 "$(cat /run/ai-live/ffmpeg.pid)"
test -s /tmp/chromium.log -o -d /tmp/chromium-ai-live

if [ "${MIR2_AI_LIVE_OUTPUT_FORMAT:-auto}" = "hls" ] || [ -z "${MIR2_AI_LIVE_OUTPUT_URL:-}" ]; then
  test -s /output/live.m3u8
fi
