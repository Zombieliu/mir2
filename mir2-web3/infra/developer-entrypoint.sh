#!/usr/bin/env bash
set -euo pipefail

save_recovery_secret_file="/run/secrets/mir2_save_recovery_mac_key"
if [[ -e "${save_recovery_secret_file}" ]]; then
  if [[ ! -r "${save_recovery_secret_file}" ]]; then
    echo "[developer entrypoint] Mounted save-recovery secret is not readable before privilege drop." >&2
    exit 78
  fi
  save_recovery_mac_key="$(tr -d '\r\n' <"${save_recovery_secret_file}")"
  if ! [[ "${save_recovery_mac_key}" =~ ^[0-9a-f]{64}$ ]]; then
    unset save_recovery_mac_key
    echo "[developer entrypoint] Mounted save-recovery secret is invalid." >&2
    exit 78
  fi
  export MIR2_SAVE_RECOVERY_MAC_KEY="${save_recovery_mac_key}"
  unset save_recovery_mac_key
fi
unset save_recovery_secret_file

write_dirs=(
  /home/node/.cargo
  /home/node/.cargo/git
  /home/node/.cargo/registry
  /home/node/.npm
  /asset-cache
  /workspace/mir2-web3/.mir2-data
  /workspace/mir2-web3/.mir2-data/developer-assets
  /workspace/mir2-web3/target
  /workspace/mir2-web3/apps/web/.next
  /workspace/mir2-web3/apps/web/node_modules
  /workspace/mir2-web3/apps/web/public/generated/map-atlas
  /workspace/mir2-web3/apps/admin-web/.next
  /workspace/mir2-web3/apps/admin-web/node_modules
)

write_files=(
  /workspace/mir2-web3/apps/web/next-env.d.ts
)

for dir in "${write_dirs[@]}"; do
  mkdir -p "$dir"
  chown node:node "$dir"
done

for file in "${write_files[@]}"; do
  touch "$file"
  chown node:node "$file"
done

if [ "$(id -u)" -eq 0 ]; then
  exec gosu node "$@"
fi

exec "$@"
