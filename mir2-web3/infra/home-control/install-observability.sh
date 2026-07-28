#!/usr/bin/env bash
set -euo pipefail

prometheus_version="3.13.1"
prometheus_sha256="962b812371aff838d152b6ff2d56fdb7a6396f5542f48ebf73421b9721f0d103"
grafana_version="13.1.1"
grafana_build="29761037902"
grafana_sha256="f6b7ffa4cb7680820d3b75e842febf99b828248a7ac8a6923c726c03846e9ded"
public_origin="https://telemetry.obelisk.build"
secret_dir="/var/lib/mir2/home-control/secrets"
operator_token_file="$secret_dir/telemetry-operator.token"
proxy_token_file="$secret_dir/observability-proxy.token"

sudo -n test -s "$operator_token_file"
if ! sudo -n test -s "$proxy_token_file"; then
  sudo -n -u mir2 sh -c \
    'umask 077; openssl rand -hex 32 > "$1"' \
    sh "$proxy_token_file"
fi
proxy_token="$(sudo -n cat "$proxy_token_file")"

download_verified() {
  local url="$1"
  local output="$2"
  local expected="$3"
  curl --fail --location --retry 3 --silent --show-error "$url" --output "$output"
  printf '%s  %s\n' "$expected" "$output" | sha256sum --check --status
}

if ! id prometheus >/dev/null 2>&1; then
  sudo -n useradd --system --home /var/lib/prometheus --shell /usr/sbin/nologin prometheus
fi
sudo -n install -d -o root -g root -m 0755 \
  /opt/prometheus/releases \
  "/opt/prometheus/releases/$prometheus_version"
sudo -n install -d -o prometheus -g prometheus -m 0750 /var/lib/prometheus
sudo -n install -d -o root -g prometheus -m 0750 /etc/prometheus /etc/prometheus/rules

if ! sudo -n test -x "/opt/prometheus/releases/$prometheus_version/prometheus"; then
  archive="$(mktemp)"
  staging="$(mktemp -d)"
  trap 'rm -f "${archive:-}"; rm -rf "${staging:-}"' EXIT
  download_verified \
    "https://github.com/prometheus/prometheus/releases/download/v${prometheus_version}/prometheus-${prometheus_version}.linux-amd64.tar.gz" \
    "$archive" \
    "$prometheus_sha256"
  tar -xzf "$archive" -C "$staging"
  sudo -n install -o root -g root -m 0755 \
    "$staging/prometheus-${prometheus_version}.linux-amd64/prometheus" \
    "/opt/prometheus/releases/$prometheus_version/prometheus"
  sudo -n install -o root -g root -m 0755 \
    "$staging/prometheus-${prometheus_version}.linux-amd64/promtool" \
    "/opt/prometheus/releases/$prometheus_version/promtool"
fi
sudo -n ln -sfn "/opt/prometheus/releases/$prometheus_version" /opt/prometheus/current
sudo -n install -o root -g prometheus -m 0640 \
  "$operator_token_file" \
  /etc/prometheus/dubhe-telemetry.token

sudo -n tee /etc/prometheus/prometheus.yml >/dev/null <<'EOF'
global:
  scrape_interval: 10s
  evaluation_interval: 10s

rule_files:
  - /etc/prometheus/rules/*.yml

scrape_configs:
  - job_name: dubhe-home-telemetry
    authorization:
      type: Bearer
      credentials_file: /etc/prometheus/dubhe-telemetry.token
    static_configs:
      - targets:
          - 127.0.0.1:18081
EOF

sudo -n tee /etc/prometheus/rules/dubhe-home-nodes.yml >/dev/null <<'EOF'
groups:
  - name: dubhe-home-nodes
    rules:
      - alert: DubheNoLiveHomeNodes
        expr: dubhe_home_nodes_admitted > 0 and dubhe_home_nodes_live == 0
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: No admitted Dubhe Home Nodes are reporting live telemetry
      - alert: DubheHomeNodeTelemetryStale
        expr: dubhe_home_node_live == 0
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: A Dubhe Home Node telemetry stream is stale
      - alert: DubheHomeNodeRelayLatencyHigh
        expr: dubhe_home_node_relay_rtt_ms > 150
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: A Dubhe Home Node has high Relay latency
      - alert: DubheHomeNodePacketLossHigh
        expr: dubhe_home_node_packet_loss_bps > 100
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: A Dubhe Home Node has more than 1 percent packet loss
EOF

sudo -n /opt/prometheus/current/promtool check config /etc/prometheus/prometheus.yml
sudo -n /opt/prometheus/current/promtool check rules /etc/prometheus/rules/dubhe-home-nodes.yml

sudo -n tee /etc/systemd/system/dubhe-prometheus.service >/dev/null <<EOF
[Unit]
Description=Dubhe Home Node Prometheus
After=network-online.target dubhe-home-telemetry.service
Wants=network-online.target

[Service]
Type=simple
User=prometheus
Group=prometheus
ExecStart=/opt/prometheus/current/prometheus \\
  --config.file=/etc/prometheus/prometheus.yml \\
  --storage.tsdb.path=/var/lib/prometheus \\
  --storage.tsdb.retention.time=30d \\
  --storage.tsdb.retention.size=4GB \\
  --web.listen-address=127.0.0.1:19090 \\
  --web.external-url=$public_origin/ops/prometheus/ \\
  --web.route-prefix=/
Restart=on-failure
RestartSec=3s
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/prometheus
MemoryMax=768M
CPUQuota=100%
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF

installed_grafana_version="$(dpkg-query -W -f='${Version}' grafana 2>/dev/null || true)"
if [[ "$installed_grafana_version" != "$grafana_version"* ]]; then
  deb="$(mktemp --suffix=.deb)"
  trap 'rm -f "${archive:-}" "${deb:-}"; rm -rf "${staging:-}"' EXIT
  download_verified \
    "https://dl.grafana.com/grafana/release/${grafana_version}/grafana_${grafana_version}_${grafana_build}_linux_amd64.deb" \
    "$deb" \
    "$grafana_sha256"
  sudo -n apt-get update -qq
  sudo -n apt-get install -y -qq adduser libfontconfig1 musl
  sudo -n dpkg -i "$deb"
fi

sudo -n install -d -o root -g grafana -m 0750 \
  /etc/grafana/provisioning/datasources \
  /etc/grafana/provisioning/dashboards \
  /var/lib/grafana/dashboards

if ! sudo -n test -s /etc/grafana/dubhe.env; then
  grafana_admin_password="$(openssl rand -hex 32)"
  sudo -n tee /etc/grafana/dubhe.env >/dev/null <<EOF
GF_SECURITY_ADMIN_PASSWORD=$grafana_admin_password
EOF
fi
sudo -n chown root:grafana /etc/grafana/dubhe.env
sudo -n chmod 0640 /etc/grafana/dubhe.env

sudo -n tee /etc/grafana/provisioning/datasources/dubhe.yml >/dev/null <<'EOF'
apiVersion: 1
deleteDatasources:
  - name: Dubhe Prometheus
    orgId: 1
datasources:
  - name: Dubhe Prometheus
    uid: dubhe-prometheus
    type: prometheus
    access: proxy
    url: http://127.0.0.1:19090
    isDefault: true
    editable: false
    jsonData:
      httpMethod: POST
      timeInterval: 10s
EOF

sudo -n tee /etc/grafana/provisioning/dashboards/dubhe.yml >/dev/null <<'EOF'
apiVersion: 1
providers:
  - name: Dubhe Home Nodes
    orgId: 1
    folder: Dubhe
    type: file
    disableDeletion: true
    editable: false
    updateIntervalSeconds: 30
    options:
      path: /var/lib/grafana/dashboards
EOF

sudo -n tee /var/lib/grafana/dashboards/dubhe-home-nodes.json >/dev/null <<'EOF'
{
  "annotations": {"list": []},
  "editable": false,
  "fiscalYearStartMonth": 0,
  "graphTooltip": 1,
  "id": null,
  "links": [],
  "liveNow": false,
  "panels": [
    {
      "datasource": {"type": "prometheus", "uid": "dubhe-prometheus"},
      "fieldConfig": {"defaults": {"color": {"mode": "thresholds"}, "thresholds": {"mode": "absolute", "steps": [{"color": "red"}, {"color": "green", "value": 1}]}}, "overrides": []},
      "gridPos": {"h": 5, "w": 6, "x": 0, "y": 0},
      "id": 1,
      "options": {"colorMode": "background", "graphMode": "area", "justifyMode": "auto", "orientation": "auto", "reduceOptions": {"calcs": ["lastNotNull"], "fields": "", "values": false}, "textMode": "auto"},
      "targets": [{"editorMode": "code", "expr": "dubhe_home_nodes_live", "legendFormat": "Live", "range": true, "refId": "A"}],
      "title": "Live Home Nodes",
      "type": "stat"
    },
    {
      "datasource": {"type": "prometheus", "uid": "dubhe-prometheus"},
      "fieldConfig": {"defaults": {"color": {"fixedColor": "purple", "mode": "fixed"}, "min": 0}, "overrides": []},
      "gridPos": {"h": 5, "w": 6, "x": 6, "y": 0},
      "id": 2,
      "options": {"colorMode": "value", "graphMode": "area", "justifyMode": "auto", "orientation": "auto", "reduceOptions": {"calcs": ["lastNotNull"], "fields": "", "values": false}, "textMode": "auto"},
      "targets": [{"editorMode": "code", "expr": "sum(dubhe_home_node_active_sessions)", "legendFormat": "Sessions", "range": true, "refId": "A"}],
      "title": "Active Sessions",
      "type": "stat"
    },
    {
      "datasource": {"type": "prometheus", "uid": "dubhe-prometheus"},
      "fieldConfig": {"defaults": {"color": {"mode": "continuous-GrYlRd"}, "min": 0}, "overrides": []},
      "gridPos": {"h": 5, "w": 6, "x": 12, "y": 0},
      "id": 3,
      "options": {"colorMode": "value", "graphMode": "area", "justifyMode": "auto", "orientation": "auto", "reduceOptions": {"calcs": ["lastNotNull"], "fields": "", "values": false}, "textMode": "auto"},
      "targets": [{"editorMode": "code", "expr": "sum(dubhe_home_node_active_zones)", "legendFormat": "Zones", "range": true, "refId": "A"}],
      "title": "Active Zones",
      "type": "stat"
    },
    {
      "datasource": {"type": "prometheus", "uid": "dubhe-prometheus"},
      "fieldConfig": {"defaults": {"color": {"fixedColor": "purple", "mode": "fixed"}, "min": 0}, "overrides": []},
      "gridPos": {"h": 5, "w": 6, "x": 18, "y": 0},
      "id": 4,
      "options": {"colorMode": "value", "graphMode": "area", "justifyMode": "auto", "orientation": "auto", "reduceOptions": {"calcs": ["lastNotNull"], "fields": "", "values": false}, "textMode": "auto"},
      "targets": [{"editorMode": "code", "expr": "sum(dubhe_home_node_capacity_sessions)", "legendFormat": "Capacity", "range": true, "refId": "A"}],
      "title": "Certified Session Capacity",
      "type": "stat"
    },
    {
      "datasource": {"type": "prometheus", "uid": "dubhe-prometheus"},
      "fieldConfig": {"defaults": {"color": {"mode": "palette-classic"}, "custom": {"axisCenteredZero": false, "axisColorMode": "text", "axisLabel": "", "axisPlacement": "auto", "drawStyle": "line", "fillOpacity": 12, "lineInterpolation": "smooth", "lineWidth": 2, "pointSize": 5, "showPoints": "never", "spanNulls": false}}, "overrides": []},
      "gridPos": {"h": 9, "w": 12, "x": 0, "y": 5},
      "id": 5,
      "options": {"legend": {"calcs": ["lastNotNull"], "displayMode": "table", "placement": "bottom", "showLegend": true}, "tooltip": {"mode": "multi", "sort": "desc"}},
      "targets": [
        {"editorMode": "code", "expr": "dubhe_home_node_active_sessions", "legendFormat": "{{node_id}} · {{assigned_zone}}", "range": true, "refId": "A"}
      ],
      "title": "Player Sessions by Home Node and Zone",
      "type": "timeseries"
    },
    {
      "datasource": {"type": "prometheus", "uid": "dubhe-prometheus"},
      "fieldConfig": {"defaults": {"color": {"mode": "palette-classic"}, "custom": {"axisCenteredZero": false, "axisColorMode": "text", "axisLabel": "ms", "axisPlacement": "auto", "drawStyle": "line", "fillOpacity": 10, "lineInterpolation": "smooth", "lineWidth": 2, "pointSize": 5, "showPoints": "never", "spanNulls": false}, "unit": "ms"}, "overrides": []},
      "gridPos": {"h": 9, "w": 12, "x": 12, "y": 5},
      "id": 6,
      "options": {"legend": {"calcs": ["lastNotNull", "max"], "displayMode": "table", "placement": "bottom", "showLegend": true}, "tooltip": {"mode": "multi", "sort": "desc"}},
      "targets": [
        {"editorMode": "code", "expr": "dubhe_home_node_relay_rtt_ms", "legendFormat": "{{node_id}} · {{region}}", "range": true, "refId": "A"},
        {"editorMode": "code", "expr": "dubhe_home_node_checkpoint_lag_ms", "legendFormat": "checkpoint · {{node_id}}", "range": true, "refId": "B"}
      ],
      "title": "Relay RTT and Checkpoint Lag",
      "type": "timeseries"
    },
    {
      "datasource": {"type": "prometheus", "uid": "dubhe-prometheus"},
      "fieldConfig": {"defaults": {"color": {"mode": "thresholds"}, "custom": {"align": "auto", "cellOptions": {"type": "auto"}, "inspect": false}, "mappings": [{"options": {"0": {"color": "red", "text": "Offline"}, "1": {"color": "green", "text": "Live"}}, "type": "value"}]}, "overrides": []},
      "gridPos": {"h": 8, "w": 24, "x": 0, "y": 14},
      "id": 7,
      "options": {"cellHeight": "sm", "showHeader": true},
      "targets": [{"editorMode": "code", "expr": "dubhe_home_node_live", "format": "table", "instant": true, "legendFormat": "__auto", "range": false, "refId": "A"}],
      "title": "Current Home Node Fleet",
      "transformations": [
        {"id": "labelsToFields", "options": {"mode": "columns"}},
        {"id": "organize", "options": {"excludeByName": {"Time": true, "__name__": true, "job": true, "instance": true}, "indexByName": {"node_id": 0, "assigned_zone": 1, "region": 2, "provider": 3, "work_mode": 4, "Value": 5}, "renameByName": {"Value": "Status", "assigned_zone": "Assigned Zone", "node_id": "Node ID", "provider": "Provider", "region": "Region", "work_mode": "Mode"}}}
      ],
      "type": "table"
    }
  ],
  "refresh": "10s",
  "schemaVersion": 41,
  "tags": ["dubhe", "home-node", "mir2"],
  "templating": {"list": []},
  "time": {"from": "now-6h", "to": "now"},
  "timepicker": {},
  "timezone": "browser",
  "title": "Dubhe Home Node Fleet",
  "uid": "dubhe-home-nodes",
  "version": 1
}
EOF
sudo -n chown root:grafana \
  /etc/grafana/provisioning/datasources/dubhe.yml \
  /etc/grafana/provisioning/dashboards/dubhe.yml
sudo -n chown grafana:grafana /var/lib/grafana/dashboards/dubhe-home-nodes.json
sudo -n chmod 0640 \
  /etc/grafana/provisioning/datasources/dubhe.yml \
  /etc/grafana/provisioning/dashboards/dubhe.yml \
  /var/lib/grafana/dashboards/dubhe-home-nodes.json

sudo -n install -d -o root -g root -m 0755 /etc/systemd/system/grafana-server.service.d
sudo -n tee /etc/systemd/system/grafana-server.service.d/dubhe.conf >/dev/null <<EOF
[Service]
EnvironmentFile=/etc/grafana/dubhe.env
Environment=GF_SERVER_HTTP_ADDR=127.0.0.1
Environment=GF_SERVER_HTTP_PORT=13000
Environment=GF_SERVER_DOMAIN=telemetry.obelisk.build
Environment=GF_SERVER_ROOT_URL=$public_origin/ops/grafana/
Environment=GF_SERVER_SERVE_FROM_SUB_PATH=true
Environment=GF_AUTH_ANONYMOUS_ENABLED=true
Environment=GF_AUTH_ANONYMOUS_ORG_ROLE=Viewer
Environment=GF_AUTH_DISABLE_LOGIN_FORM=true
Environment=GF_USERS_VIEWERS_CAN_EDIT=false
Environment=GF_USERS_DEFAULT_THEME=dark
Environment=GF_SECURITY_COOKIE_SECURE=true
Environment=GF_SECURITY_COOKIE_SAMESITE=strict
Environment=GF_LOG_LEVEL=warn
MemoryMax=512M
CPUQuota=100%
EOF

sudo -n python3 - "$proxy_token" <<'PY'
from pathlib import Path
import sys

path = Path("/etc/caddy/Caddyfile")
token = sys.argv[1]
text = path.read_text()
begin = "\t# BEGIN DUBHE OBSERVABILITY\n"
end = "\t# END DUBHE OBSERVABILITY\n"
if begin in text and end in text:
    text = text[: text.index(begin)] + text[text.index(end) + len(end) :]
marker = "\thandle_path /home/telemetry/* {\n"
if marker not in text:
    raise SystemExit("Caddy Home telemetry marker was not found")
block = f"""{begin}\t@dubheGrafana {{
\t\tpath /home/ops/grafana /home/ops/grafana/*
\t\theader X-Dubhe-Observability-Token {token}
\t}}
\thandle @dubheGrafana {{
\t\turi strip_prefix /home
\t\treverse_proxy 127.0.0.1:13000
\t}}

\t@dubhePrometheus {{
\t\tpath /home/ops/prometheus /home/ops/prometheus/*
\t\theader X-Dubhe-Observability-Token {token}
\t}}
\thandle @dubhePrometheus {{
\t\turi strip_prefix /home/ops/prometheus
\t\treverse_proxy 127.0.0.1:19090
\t}}

\t@dubheObservabilityDenied path /home/ops/*
\trespond @dubheObservabilityDenied 404
{end}
"""
path.write_text(text.replace(marker, block + "\n" + marker))
PY

sudo -n systemctl daemon-reload
sudo -n systemctl enable --now dubhe-prometheus.service grafana-server.service
sudo -n caddy fmt --overwrite /etc/caddy/Caddyfile
sudo -n caddy validate --config /etc/caddy/Caddyfile
sudo -n systemctl reload caddy

wait_http() {
  local url="$1"
  for _ in $(seq 1 30); do
    if curl --fail --silent --show-error "$url" >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done
  echo "timed out waiting for $url" >&2
  return 1
}

wait_http http://127.0.0.1:19090/-/ready
wait_http http://127.0.0.1:13000/api/health
curl --fail --silent --show-error \
  -H "X-Dubhe-Observability-Token: $proxy_token" \
  https://relay-hk.obelisk.build/home/ops/grafana/api/health >/dev/null
curl --fail --silent --show-error \
  -H "X-Dubhe-Observability-Token: $proxy_token" \
  "https://relay-hk.obelisk.build/home/ops/prometheus/api/v1/query?query=up" >/dev/null

echo "DUBHE_OBSERVABILITY_INSTALLED prometheus=$prometheus_version grafana=$grafana_version"
