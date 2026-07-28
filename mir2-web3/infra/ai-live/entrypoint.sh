#!/bin/sh
set -eu

render_url="${MIR2_AI_LIVE_RENDER_URL:-http://host.docker.internal:3002/spectate?aiLive=1&spectateMode=director}"
output_url="${MIR2_AI_LIVE_OUTPUT_URL:-}"
output_format="${MIR2_AI_LIVE_OUTPUT_FORMAT:-auto}"
video_size="${MIR2_AI_LIVE_VIDEO_SIZE:-1920x1080}"
frame_rate="${MIR2_AI_LIVE_FRAME_RATE:-30}"
video_bitrate="${MIR2_AI_LIVE_VIDEO_BITRATE:-4500k}"
audio_bitrate="${MIR2_AI_LIVE_AUDIO_BITRATE:-128k}"
heartbeat_url="${MIR2_AI_DISTRIBUTION_HEARTBEAT_URL:-}"
heartbeat_token="${MIR2_AI_DISTRIBUTION_HEARTBEAT_TOKEN:-}"
heartbeat_platform="${MIR2_AI_DISTRIBUTION_RTMP_PLATFORM:-youtube}"
heartbeat_worker_id="${MIR2_AI_DISTRIBUTION_WORKER_ID:-$(hostname)}"
heartbeat_pid=""

case "$video_size" in
  *x*) ;;
  *) echo "MIR2_AI_LIVE_VIDEO_SIZE must look like 1920x1080" >&2; exit 64 ;;
esac

case "$heartbeat_platform" in
  *[!A-Za-z0-9_-]*|"") echo "MIR2_AI_DISTRIBUTION_RTMP_PLATFORM is invalid" >&2; exit 64 ;;
esac
case "$heartbeat_worker_id" in
  *[!A-Za-z0-9_.:-]*|"") echo "MIR2_AI_DISTRIBUTION_WORKER_ID is invalid" >&2; exit 64 ;;
esac
if [ -n "$heartbeat_url" ] && [ -z "$heartbeat_token" ]; then
  echo "MIR2_AI_DISTRIBUTION_HEARTBEAT_TOKEN is required when heartbeat URL is set" >&2
  exit 64
fi

post_heartbeat() {
  runtime_state="$1"
  message="${2:-}"
  [ -n "$heartbeat_url" ] || return 0
  curl --silent --show-error --fail --max-time 4 \
    -H "Authorization: Bearer $heartbeat_token" \
    -H "Content-Type: application/json" \
    --data "{\"platform\":\"$heartbeat_platform\",\"workerId\":\"$heartbeat_worker_id\",\"runtimeState\":\"$runtime_state\",\"message\":\"$message\"}" \
    "$heartbeat_url" >/dev/null 2>&1 || true
}

heartbeat_loop() {
  watched_pid="$1"
  # Do not report live merely because FFmpeg forked. It must survive the initial
  # RTMP/RTMPS connection window first.
  sleep 3
  while kill -0 "$watched_pid" 2>/dev/null; do
    post_heartbeat live "encoder process healthy"
    sleep 10
  done
}

if [ "$output_format" = "auto" ]; then
  case "$output_url" in
    rtmp://*|rtmps://*) output_format="rtmp" ;;
    "") output_format="hls" ;;
    *) echo "Set MIR2_AI_LIVE_OUTPUT_FORMAT to rtmp or hls" >&2; exit 64 ;;
  esac
fi

if [ "$output_format" = "hls" ]; then
  mkdir -p /output
  output_url="${output_url:-/output/live.m3u8}"
elif [ "$output_format" != "rtmp" ]; then
  echo "MIR2_AI_LIVE_OUTPUT_FORMAT must be auto, rtmp, or hls" >&2
  exit 64
elif [ -z "$output_url" ]; then
  echo "MIR2_AI_LIVE_OUTPUT_URL is required for RTMP" >&2
  exit 64
fi

export DISPLAY=:99
export PULSE_SERVER=unix:/run/ai-live/pulse/native
mkdir -p /run/ai-live/pulse /tmp/chromium-ai-live

Xvfb "$DISPLAY" -screen 0 "${video_size}x24" -nolisten tcp -ac >/tmp/xvfb.log 2>&1 &
xvfb_pid=$!

pulseaudio \
  --daemonize=yes \
  --exit-idle-time=-1 \
  --disallow-exit \
  --log-target=file:/tmp/pulseaudio.log \
  --load="module-native-protocol-unix socket=/run/ai-live/pulse/native auth-anonymous=1"

pactl load-module module-null-sink sink_name=ai_live sink_properties=device.description=DubheAILive >/dev/null
pactl set-default-sink ai_live

chromium \
  --no-sandbox \
  --disable-dev-shm-usage \
  --disable-gpu-sandbox \
  --autoplay-policy=no-user-gesture-required \
  --kiosk \
  --window-size="${video_size%x*},${video_size#*x}" \
  --user-data-dir=/tmp/chromium-ai-live \
  "$render_url" >/tmp/chromium.log 2>&1 &
chromium_pid=$!

cleanup() {
  [ -n "$heartbeat_pid" ] && kill "$heartbeat_pid" 2>/dev/null || true
  post_heartbeat stopped "encoder container stopping"
  [ -f /run/ai-live/ffmpeg.pid ] && kill "$(cat /run/ai-live/ffmpeg.pid)" 2>/dev/null || true
  kill "$chromium_pid" "$xvfb_pid" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

sleep 6

while kill -0 "$chromium_pid" 2>/dev/null && kill -0 "$xvfb_pid" 2>/dev/null; do
  rm -f /run/ai-live/healthy
  if [ "$output_format" = "rtmp" ]; then
    ffmpeg \
      -hide_banner -loglevel warning \
      -thread_queue_size 1024 -f x11grab -draw_mouse 0 -framerate "$frame_rate" -video_size "$video_size" -i "$DISPLAY" \
      -thread_queue_size 1024 -f pulse -i ai_live.monitor \
      -c:v libx264 -preset veryfast -tune zerolatency -pix_fmt yuv420p -g "$((frame_rate * 2))" \
      -b:v "$video_bitrate" -maxrate "$video_bitrate" -bufsize 9000k \
      -c:a aac -b:a "$audio_bitrate" -ar 48000 \
      -f flv "$output_url" >/tmp/ffmpeg.log 2>&1 &
  else
    ffmpeg \
      -hide_banner -loglevel warning \
      -thread_queue_size 1024 -f x11grab -draw_mouse 0 -framerate "$frame_rate" -video_size "$video_size" -i "$DISPLAY" \
      -thread_queue_size 1024 -f pulse -i ai_live.monitor \
      -c:v libx264 -preset veryfast -tune zerolatency -pix_fmt yuv420p -g "$((frame_rate * 2))" \
      -b:v "$video_bitrate" -maxrate "$video_bitrate" -bufsize 9000k \
      -c:a aac -b:a "$audio_bitrate" -ar 48000 \
      -f hls -hls_time 2 -hls_list_size 8 -hls_flags delete_segments+independent_segments+program_date_time \
      "$output_url" >/tmp/ffmpeg.log 2>&1 &
  fi
  ffmpeg_pid=$!
  echo "$ffmpeg_pid" >/run/ai-live/ffmpeg.pid
  touch /run/ai-live/healthy
  if [ "$output_format" = "rtmp" ]; then
    post_heartbeat starting "encoder process started"
    heartbeat_loop "$ffmpeg_pid" &
    heartbeat_pid=$!
  fi

  wait "$ffmpeg_pid" || true
  if [ -n "$heartbeat_pid" ]; then
    kill "$heartbeat_pid" 2>/dev/null || true
    wait "$heartbeat_pid" 2>/dev/null || true
    heartbeat_pid=""
    post_heartbeat error "encoder process exited"
  fi
  rm -f /run/ai-live/ffmpeg.pid /run/ai-live/healthy
  sleep 3
done

echo "AI live browser or virtual display stopped" >&2
exit 1
