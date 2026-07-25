#!/usr/bin/env bash
set -euo pipefail

write_dirs=(
  /home/node/.cargo
  /home/node/.cargo/git
  /home/node/.cargo/registry
  /home/node/.config/gh
  /home/node/.npm
  /asset-cache
  /workspace/mir2-web3/.mir2-data
  /workspace/mir2-web3/.mir2-data/developer-assets
  /workspace/mir2-web3/target
  /workspace/mir2-web3/apps/web/.next
  /workspace/mir2-web3/apps/web/node_modules
  /workspace/mir2-web3/apps/admin-web/.next
  /workspace/mir2-web3/apps/admin-web/node_modules
)

for dir in "${write_dirs[@]}"; do
  mkdir -p "$dir"
  chown node:node "$dir"
done

if [ "$(id -u)" -eq 0 ]; then
  exec gosu node "$@"
fi

exec "$@"
