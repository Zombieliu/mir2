#!/usr/bin/env bash
set -uo pipefail

HEALTH_URL="${MIR2_GATEWAY_HEALTH_URL:-http://127.0.0.1:7110/health}"
ENV_FILE="${MIR2_GATEWAY_ENV_FILE:-/etc/mir2/gateway.env}"
SHOW_LOGS=0

usage() {
  cat <<'USAGE'
Usage: mir2-status [--logs]

Shows host, service, Gateway, Postgres, Redis, disk, and network status for the
Mir2 internal-test server.

Options:
  --logs   Include recent Gateway logs and system warnings.
  -h       Show this help.

Tip:
  Run with sudo for Postgres table counts and root-only journal visibility:
    sudo mir2-status --logs
USAGE
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --logs)
      SHOW_LOGS=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

section() {
  printf '\n== %s ==\n' "$1"
}

kv() {
  printf '%-28s %s\n' "$1" "$2"
}

command_exists() {
  command -v "$1" >/dev/null 2>&1
}

service_state() {
  local service="$1"
  if systemctl is-active --quiet "$service" 2>/dev/null; then
    printf '[OK] active'
  else
    printf '[WARN] %s' "$(systemctl is-active "$service" 2>/dev/null || echo unknown)"
  fi
}

cpu_line() {
  if command_exists top; then
    LC_ALL=C top -bn1 | awk '/%Cpu/ {print $0; exit}'
  fi
}

mem_line() {
  if command_exists free; then
    free -h | awk '/^Mem:/ {printf "%s total, %s used, %s available", $2, $3, $7}'
  fi
}

disk_line() {
  df -h / | awk 'NR==2 {printf "%s used of %s (%s), %s free", $3, $2, $5, $4}'
}

gateway_health() {
  local health
  if ! health="$(curl -fsS --max-time 3 "$HEALTH_URL" 2>/dev/null)"; then
    echo "[WARN] Gateway health request failed: $HEALTH_URL"
    return
  fi

  if command_exists jq; then
    printf '%s\n' "$health" | jq '{
      ok,
      http,
      ws,
      tcp_stub,
      session_cache: ((.session_cache // .sessionCache // {}) | {
        backend,
        healthy,
        ttlSeconds,
        recordCount,
        routeLeaseCount,
        staleRecordCount,
        lastError
      }),
      capacity
    }'
  else
    printf '%s\n' "$health"
  fi
}

service_processes() {
  ps -eo pid,comm,%cpu,%mem,rss,args --sort=-rss |
    awk 'NR==1 || /mir2-gateway|postgres|redis-server/ {print}' |
    sed -n '1,20p'
}

postgres_status() {
  kv "service" "$(service_state postgresql)"
  if ! command_exists psql; then
    kv "detail" "psql not installed"
    return
  fi

  local sql
  sql="select 'connections|' || count(*) from pg_stat_activity; select 'db_size|' || pg_size_pretty(pg_database_size('mir2')); select 'accounts|' || count(*) from accounts; select 'characters|' || count(*) from characters; select 'character_saves|' || count(*) from character_saves;"

  if [ "$(id -u)" -eq 0 ]; then
    if sudo -u postgres psql -d mir2 -Atc "$sql" 2>/dev/null; then
      return
    fi
  elif sudo -n -u postgres true 2>/dev/null; then
    if sudo -n -u postgres psql -d mir2 -Atc "$sql" 2>/dev/null; then
      return
    fi
  fi

  kv "detail" "run sudo mir2-status for table counts"
}

redis_status() {
  kv "service" "$(service_state redis-server)"
  if ! command_exists redis-cli; then
    kv "detail" "redis-cli not installed"
    return
  fi

  if ! redis-cli ping >/dev/null 2>&1; then
    kv "ping" "[WARN] failed"
    return
  fi

  kv "ping" "[OK] PONG"
  redis-cli info memory stats clients 2>/dev/null |
    awk -F: '/^(used_memory_human|used_memory_peak_human|connected_clients|total_commands_processed|keyspace_hits|keyspace_misses|expired_keys):/ {gsub(/\r/,"",$2); printf "%-28s %s\n", $1, $2}'
}

network_status() {
  if command_exists ss; then
    ss -s 2>/dev/null || true
    printf '\n'
    ss -tnp 'sport = :7110' 2>/dev/null | sed -n '1,12p' || true
  else
    kv "detail" "ss not installed"
  fi
}

recent_logs() {
  section "Gateway Logs"
  journalctl -u mir2-gateway --since '30 minutes ago' --no-pager 2>/dev/null |
    tail -n 80 || true

  section "System Warnings"
  journalctl -p warning..alert --since '24 hours ago' --no-pager 2>/dev/null |
    tail -n 80 || true
}

main() {
  section "Host"
  kv "time" "$(date -Is)"
  kv "host" "$(hostname)"
  kv "uptime" "$(uptime -p 2>/dev/null || uptime)"
  kv "load" "$(uptime | sed 's/.*load average: //')"
  kv "kernel" "$(uname -r)"

  section "System"
  kv "cpu" "$(cpu_line)"
  kv "memory" "$(mem_line)"
  kv "disk /" "$(disk_line)"
  kv "inodes /" "$(df -ih / | awk 'NR==2 {printf "%s used of %s (%s)", $3, $2, $5}')"

  section "Services"
  kv "mir2-gateway" "$(service_state mir2-gateway)"
  kv "postgresql" "$(service_state postgresql)"
  kv "redis-server" "$(service_state redis-server)"

  section "Gateway Health"
  gateway_health

  section "Processes"
  service_processes

  section "Postgres"
  postgres_status

  section "Redis"
  redis_status

  section "Network"
  network_status

  if [ "$SHOW_LOGS" -eq 1 ]; then
    recent_logs
  fi
}

main
