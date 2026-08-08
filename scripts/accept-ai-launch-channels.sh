#!/bin/sh
set -eu

gateway_url="${MIR2_AI_ACCEPT_GATEWAY_URL:-http://127.0.0.1:7110}"
heartbeat_token="${MIR2_AI_DISTRIBUTION_HEARTBEAT_TOKEN:-}"
worker_id="${MIR2_AI_ACCEPT_WORKER_ID:-acceptance-probe}"
status_file="$(mktemp)"
trap 'rm -f "$status_file"' EXIT INT TERM

if [ -z "$heartbeat_token" ]; then
  echo "MIR2_AI_DISTRIBUTION_HEARTBEAT_TOKEN is required" >&2
  exit 64
fi

curl --silent --show-error --fail \
  -H "Authorization: Bearer $heartbeat_token" \
  -H "Content-Type: application/json" \
  --data "{\"platform\":\"youtube\",\"workerId\":\"$worker_id\",\"runtimeState\":\"live\",\"message\":\"launch acceptance probe\"}" \
  "$gateway_url/ai-live/distribution/heartbeat" >"$status_file"

node - "$status_file" <<'NODE'
const fs = require("node:fs");
const status = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
const channels = new Map(status.channels.map((channel) => [channel.channel, channel]));
for (const required of ["gameOverlay", "webBroadcast", "rtmpBroadcast", "discordWebhook"]) {
  if (!channels.has(required)) throw new Error(`missing launch channel: ${required}`);
}
if (channels.get("rtmpBroadcast").state !== "ready") {
  throw new Error(`RTMP runtime is not ready: ${channels.get("rtmpBroadcast").state}`);
}
if (JSON.stringify(status).includes(process.env.MIR2_AI_DISTRIBUTION_HEARTBEAT_TOKEN)) {
  throw new Error("heartbeat token leaked into status response");
}
console.log(JSON.stringify({
  ok: true,
  profile: status.launch.profile,
  ready: `${status.launch.readyChannels}/${status.launch.requiredChannels}`,
  readyForLaunch: status.launch.readyForLaunch,
  blockers: status.launch.blockers,
}, null, 2));
NODE
