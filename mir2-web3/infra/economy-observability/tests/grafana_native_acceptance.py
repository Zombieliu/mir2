"""Isolated Windows Grafana OSS acceptance; never touches an existing instance.

Download is pinned to the official 13.2.4 archive/checksum observed 2026-10-19.
Only a self-owned directory, loopback HTTP and explicit local read-only PG config
are accepted. Passwords stay in the private runtime, never in evidence/output.
"""
from __future__ import annotations

import argparse
import base64
import configparser
from concurrent.futures import ThreadPoolExecutor, as_completed
import hashlib
import json
import os
from pathlib import Path
import secrets
import socket
import ssl
import subprocess
import sys
import tarfile
import time
from urllib import request, error
from urllib.parse import urlsplit

VERSION = "13.2.4"
URL = "https://dl.grafana.com/grafana/release/13.2.4/grafana_13.2.4_38252365904_windows_amd64.tar.gz"
SHA256 = "3e005ff50fe3bb6b8cb428ef127636fac4ee676bf9b6d819c6a2297d21f4bcbd"
ARCHIVE_BYTES = 466144621
ROOT = Path(__file__).resolve().parents[1]
MARKER = "hunting-grafana-owned.json"


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def owned(runtime):
    runtime = runtime.resolve()
    marker = runtime / MARKER
    if runtime.exists() and not marker.exists() and any(runtime.iterdir()):
        raise RuntimeError("Refusing a nonempty directory without our ownership marker")
    runtime.mkdir(parents=True, exist_ok=True)
    if marker.exists():
        data = json.loads(marker.read_text(encoding="utf-8"))
        if data != {"kind": "hunting-grafana-native-acceptance", "root": str(runtime), "version": VERSION}:
            raise RuntimeError("Runtime ownership marker mismatch")
    else:
        save(marker, {"kind": "hunting-grafana-native-acceptance", "root": str(runtime), "version": VERSION})
        if os.name == "nt":
            acl = subprocess.run(["icacls", str(runtime), "/inheritance:r", "/grant:r", "*S-1-5-18:(OI)(CI)F", "*S-1-5-32-544:(OI)(CI)F"], capture_output=True)
            if acl.returncode:
                raise RuntimeError("Unable to restrict our private runtime directory")
    return runtime


def download_opener(direct, tls12):
    context = ssl.create_default_context()
    if tls12:
        context.maximum_version = ssl.TLSVersion.TLSv1_2
    handlers = [request.HTTPSHandler(context=context)]
    if direct:
        handlers.append(request.ProxyHandler({}))
    return request.build_opener(*handlers)


def download_ranges(runtime, direct, workers, tls12):
    block = (ARCHIVE_BYTES + workers - 1) // workers
    def fetch(index):
        first, last = index * block, min((index + 1) * block, ARCHIVE_BYTES) - 1
        part = runtime / f"official-range-{index:02d}.bin"
        if part.exists() and part.stat().st_size == last - first + 1:
            return index, part
        for attempt in range(3):
            opener = download_opener(direct, tls12)
            req = request.Request(URL, headers={"Range": f"bytes={first}-{last}", "User-Agent": "Mir2LocalAcceptance/1.0"})
            try:
                with opener.open(req, timeout=45) as response, part.open("wb") as output:
                    if response.status != 206 or response.headers.get("Content-Range") != f"bytes {first}-{last}/{ARCHIVE_BYTES}":
                        raise RuntimeError("Official range response boundary mismatch")
                    while chunk := response.read(256 * 1024):
                        output.write(chunk)
                break
            except (error.URLError, ConnectionError, TimeoutError, ssl.SSLError):
                if attempt == 2:
                    raise RuntimeError("Public archive range failed after three bounded transport attempts") from None
                time.sleep(attempt + 1)
        if part.stat().st_size != last - first + 1:
            raise RuntimeError("Official range response length mismatch")
        return index, part
    parts = {}
    # A proxy may reject many simultaneous TLS tunnels; range count does not
    # require the same number of connections. Four concurrent tunnels are ample.
    with ThreadPoolExecutor(max_workers=min(workers, 4)) as executor:
        for future in as_completed([executor.submit(fetch, index) for index in range(workers)]):
            index, part = future.result()
            parts[index] = part
            print(json.dumps({"download_ranges_complete": len(parts), "total_ranges": workers}), flush=True)
    merged = runtime / "grafana-official.parallel.partial"
    with merged.open("wb") as output:
        for index in range(workers):
            with parts[index].open("rb") as source:
                while chunk := source.read(1024 * 1024):
                    output.write(chunk)
    if hashlib.file_digest(merged.open("rb"), "sha256").hexdigest() != SHA256:
        raise RuntimeError("Merged official archive checksum mismatch")
    merged.replace(runtime / "grafana-official.tar.gz")


def download(runtime, direct=False, workers=1, tls12=False):
    archive = runtime / "grafana-official.tar.gz"
    if workers < 1 or workers > 32:
        raise RuntimeError("Download worker count must be between 1 and 32")
    if not archive.exists() and workers > 1:
        download_ranges(runtime, direct, workers, tls12)
    if not archive.exists():
        partial = runtime / "grafana-official.tar.gz.partial"
        digest = hashlib.sha256()
        size = 0
        req = request.Request(URL, headers={"User-Agent": "Mir2LocalAcceptance/1.0"})
        # Opt-in direct transport changes routing only, never TLS verification.
        opener = download_opener(direct, tls12)
        with opener.open(req, timeout=45) as response, partial.open("wb") as out:
            while chunk := response.read(1024 * 1024):
                out.write(chunk)
                digest.update(chunk)
                size += len(chunk)
                if size % (32 * 1024 * 1024) == 0:
                    print(json.dumps({"downloaded_mib": size // 1024 // 1024}), flush=True)
        if digest.hexdigest() != SHA256:
            raise RuntimeError("Official archive checksum mismatch; retained partial for inspection")
        partial.replace(archive)
    digest = hashlib.file_digest(archive.open("rb"), "sha256").hexdigest()
    if digest != SHA256:
        raise RuntimeError("Cached archive checksum mismatch")
    distribution = runtime / "distribution"
    ready = runtime / "distribution-ready.json"
    if not ready.exists():
        distribution.mkdir(exist_ok=True)
        with tarfile.open(archive, "r:gz") as tar:
            for member in tar.getmembers():
                target = (distribution / member.name).resolve()
                if not target.is_relative_to(distribution.resolve()) or not (member.isfile() or member.isdir()):
                    raise RuntimeError("Refusing archive traversal, link or special file")
            tar.extractall(distribution, filter="data")
        binaries = list(distribution.glob("*/bin/grafana.exe"))
        if not binaries:
            binaries = list(distribution.glob("*/bin/grafana-server.exe"))
        if len(binaries) != 1:
            raise RuntimeError("Official archive has an unexpected executable layout")
        save(ready, {"home": str(binaries[0].parent.parent), "binary": str(binaries[0]), "archive_sha256": SHA256})
    return {"version": VERSION, "archive_bytes": archive.stat().st_size, "sha256_verified": digest == SHA256,
            "download_transport": "direct_https" if direct else "environment_proxy_https", "download_range_count": workers,
            "download_concurrency": min(workers, 4),
            "download_tls_maximum": "1.2" if tls12 else "system_default", "tls_verification_disabled": False}


def reserve_port():
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


def read_pg(path):
    data = json.loads(path.read_text(encoding="utf-8"))
    def field(*names):
        for name in names:
            if name in data:
                return data[name]
        raise RuntimeError("Read-only PG config is missing a required connection field")
    raw_host = field("host")
    if raw_host in ("localhost", "127.0.0.1", "::1"):
        host, port = raw_host, data.get("port", 5432)
    else:
        endpoint = urlsplit("//" + raw_host)
        if endpoint.username or endpoint.password or endpoint.path or endpoint.query or endpoint.fragment:
            raise RuntimeError("PostgreSQL host must be a plain loopback endpoint")
        host, port = endpoint.hostname, endpoint.port
    database = field("database", "dbname", "db")
    if host not in ("localhost", "127.0.0.1", "::1") or not database.startswith("mir2_hunting_acceptance_"):
        raise RuntimeError("Refusing nonlocal/nonacceptance PostgreSQL")
    if data.get("sslmode", "verify-full") != "verify-full":
        raise RuntimeError("PostgreSQL acceptance must retain verify-full")
    ca = Path(field("ca_certificate_path", "ca_path", "caPath", "sslrootcert", "ca", "root_ca_path"))
    if ca.suffix.lower() not in (".crt", ".pem") or "key" in ca.name.lower():
        raise RuntimeError("Only the explicitly supplied public CA certificate may be used")
    if not port or not 1 <= int(port) <= 65535:
        raise RuntimeError("PostgreSQL acceptance endpoint is missing a valid port")
    return {"host": host, "port": int(port), "database": database, "user": field("user", "username"), "password": field("password"), "ca": str(ca.resolve())}


def start(runtime, pg_json=None, identity=None):
    if (runtime / "instance-private.json").exists():
        raise RuntimeError("Instance state already exists; inspect/stop it explicitly before a new instance")
    ready = json.loads((runtime / "distribution-ready.json").read_text(encoding="utf-8"))
    home = Path(ready["home"])
    binary = Path(ready["binary"])
    for name in ("data", "logs", "plugins"):
        (runtime / name).mkdir(exist_ok=True)
    port = reserve_port()
    identity = identity or {}
    private = {"admin_user": identity.get("admin_user", "hunting_acceptance"), "admin_password": identity.get("admin_password", secrets.token_urlsafe(30)),
               "secret_key": identity.get("secret_key", secrets.token_urlsafe(40)), "port": port, "pg_json": str(pg_json.resolve()) if pg_json else None}
    provision = runtime / "provisioning"
    for source in (ROOT / "grafana/provisioning").rglob("*.yaml"):
        value = json.loads(source.read_text(encoding="utf-8"))
        if "providers" in value:
            value["providers"][0]["options"]["path"] = str((ROOT / "grafana/dashboards").resolve()).replace("\\", "/")
        if "datasources" in value and pg_json:
            pg = read_pg(pg_json)
            value["datasources"][0]["jsonData"].update({"tlsConfigurationMethod": "file-path", "sslRootCertFile": pg["ca"].replace("\\", "/")})
        save(provision / source.relative_to(ROOT / "grafana/provisioning"), value)
    config = runtime / "grafana-local.ini"
    config.write_text(f"""[paths]
data = {runtime / 'data'}
logs = {runtime / 'logs'}
plugins = {runtime / 'plugins'}
provisioning = {provision}
[server]
http_addr = 127.0.0.1
http_port = {port}
domain = localhost
[security]
admin_user = {private['admin_user']}
admin_password = {private['admin_password']}
secret_key = {private['secret_key']}
[auth.anonymous]
enabled = true
org_role = Viewer
[analytics]
reporting_enabled = false
check_for_updates = false
check_for_plugin_updates = false
[users]
allow_sign_up = false
[smtp]
enabled = false
[plugins]
preinstall_disabled = true
preinstall_auto_update = false
public_key_retrieval_disabled = true
[unified_alerting]
enabled = true
execute_alerts = false
""", encoding="utf-8")
    allowed = {"PATH", "SYSTEMROOT", "WINDIR", "TEMP", "TMP", "COMSPEC", "PATHEXT", "USERPROFILE", "LOCALAPPDATA", "APPDATA", "PROGRAMDATA"}
    env = {key: value for key, value in os.environ.items() if key.upper() in allowed}
    env.update({"MIR2_ECONOMY_DB_HOST": "127.0.0.1:1", "MIR2_ECONOMY_DB_USER": "unwired_acceptance_only", "MIR2_ECONOMY_DB_NAME": "unwired_acceptance_only", "MIR2_ECONOMY_DB_PASSWORD": secrets.token_urlsafe(30), "MIR2_ECONOMY_ALERT_RELAY_URL": "http://127.0.0.1:1/never-send", "MIR2_ECONOMY_ALERT_RELAY_TOKEN": secrets.token_urlsafe(30)})
    if pg_json:
        pg = read_pg(pg_json)
        endpoint = f"[{pg['host']}]:{pg['port']}" if pg["host"] == "::1" else f"{pg['host']}:{pg['port']}"
        env.update({"MIR2_ECONOMY_DB_HOST": endpoint, "MIR2_ECONOMY_DB_USER": pg["user"], "MIR2_ECONOMY_DB_NAME": pg["database"], "MIR2_ECONOMY_DB_PASSWORD": pg["password"]})
    args = [str(binary)] + (["server"] if binary.name.lower() == "grafana.exe" else []) + ["--homepath", str(home), "--config", str(config)]
    with (runtime / "server.stdout.log").open("wb") as stdout, (runtime / "server.stderr.log").open("wb") as stderr:
        process = subprocess.Popen(args, cwd=home, env=env, stdout=stdout, stderr=stderr, creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
    private["pid"] = process.pid
    private["binary"] = str(binary)
    private["config"] = str(config)
    save(runtime / "instance-private.json", private)
    return {"pid": process.pid, "loopback_url": f"http://127.0.0.1:{port}", "pg_connected_configuration": bool(pg_json), "notifications_disabled": True}


def api(runtime, path, body=None, method=None):
    private = json.loads((runtime / "instance-private.json").read_text(encoding="utf-8"))
    credentials = base64.b64encode(f"{private['admin_user']}:{private['admin_password']}".encode()).decode()
    req = request.Request(f"http://127.0.0.1:{private['port']}" + path, data=json.dumps(body).encode() if body is not None else None,
                          method=method or ("POST" if body is not None else "GET"), headers={"Authorization": "Basic " + credentials, "Content-Type": "application/json"})
    try:
        # Loopback admin credentials must never be sent to an environment proxy.
        with request.build_opener(request.ProxyHandler({})).open(req, timeout=30) as response:
            return json.load(response)
    except error.HTTPError as failure:
        # Never echo returned connection details or secret-bearing payloads.
        payload = failure.read().decode("utf-8", errors="replace")
        save(runtime / "last-api-error-private.json", {"status": failure.code, "body": payload})
        raise RuntimeError(f"Owned Grafana API returned HTTP {failure.code}; inspect its private error evidence") from None


def verify(runtime):
    deadline = time.monotonic() + 45
    while True:
        try:
            health = api(runtime, "/api/health")
            break
        except (error.URLError, ConnectionError):
            if time.monotonic() > deadline:
                raise RuntimeError("Owned Grafana did not become healthy; inspect its private logs")
            time.sleep(1)
    dashboard = api(runtime, "/api/dashboards/uid/mir2-hunting-permit")
    rules = api(runtime, "/api/v1/provisioning/alert-rules")
    contact_points = api(runtime, "/api/v1/provisioning/contact-points")
    datasource = api(runtime, "/api/datasources/uid/mir2-economy-readonly")
    source_rules = json.loads((ROOT / "grafana/provisioning/alerting/economy-rules.yaml").read_text(encoding="utf-8"))["groups"][0]["rules"]
    if health.get("version") != VERSION or len(dashboard["dashboard"]["panels"]) != 14 or {r["uid"] for r in rules} != {r["uid"] for r in source_rules}:
        raise RuntimeError("Grafana import/version/rule set mismatch")
    if not any(p["uid"] == "mir2-economy-webhook" and p["type"] == "webhook" for p in contact_points):
        raise RuntimeError("Grafana webhook schema did not import")
    for actual in rules:
        expected = next(r for r in source_rules if r["uid"] == actual["uid"])
        if actual["data"][0]["model"]["rawSql"] != expected["data"][0]["model"]["rawSql"]:
            raise RuntimeError("Alert query changed during actual provisioning")
        for position in (1, 2):
            expected_model = expected["data"][position]["model"]
            actual_model = actual["data"][position]["model"]
            if any(actual_model.get(key) != value for key, value in expected_model.items()) or "settings" in actual_model:
                raise RuntimeError("Provisioned expression chain differs from the supported source model")
    return {"grafana_version": health["version"], "health_database": health["database"], "dashboard_panels_imported": 14,
            "alert_rules_imported": len(rules), "alert_sql_preserved": True, "contact_point_imported": True,
            "datasource_plugin": datasource["type"], "sslmode": datasource["jsonData"]["sslmode"],
            "native_schema_import_passed": True, "datasource_queries_executed": False, "notifications_sent": False}


def fixture_sql(prefix):
    """Render append-only synthetic fixtures for the acceptance DB owner to run.

    This process never receives owner credentials and never executes fixture DML.
    Unlike postgres_acceptance's transaction-only test, rows must be committed
    before a separately connected Grafana can see them.
    """
    if not prefix.startswith("hunting-observer-test-grafana-") or any(c not in "abcdefghijklmnopqrstuvwxyz0123456789-" for c in prefix):
        raise RuntimeError("Synthetic fixture prefix must use the dedicated Grafana namespace")
    sys.path.insert(0, str(ROOT / "tests"))
    from postgres_acceptance import build_script
    script, _ = build_script(prefix, "unused_grafana_fixture_role")
    inserts = script.split("CREATE TEMP TABLE hunting_observer_expected AS", 1)[0]
    vectors = json.loads((ROOT / "fixtures/alert-cases.json").read_text(encoding="utf-8"))
    rejected = {"duplicate_daily_row", "zero_stock_baseline_not_normal", "permit_zero_plan_undefined", "conversion_conservation_error"}
    expected = {prefix + "-" + case["name"]: case["expected"] for case in vectors["cases"] if case["name"] not in rejected}
    script = inserts + "COMMIT;\n"
    return script, {"fixture_prefix": prefix, "synthetic_only": True,
                    "fixture_sql_sha256": hashlib.sha256(script.encode("utf-8")).hexdigest(), "expected": expected}


def query(runtime, queries):
    finish = int(time.time() * 1000)
    response = api(runtime, "/api/ds/query", {"from": str(finish - 91 * 86400000), "to": str(finish), "queries": queries})
    for result in response.get("results", {}).values():
        if result.get("error") or result.get("status", 200) >= 400:
            save(runtime / "last-query-error-private.json", response)
            raise RuntimeError("Grafana datasource/expression returned a query error; inspect private evidence")
    return response


def values_by_realm(response, ref):
    values = {}
    for frame in response["results"][ref].get("frames", []):
        fields = frame["schema"]["fields"]
        columns = frame["data"]["values"]
        realm_column = next((i for i, field in enumerate(fields) if field["name"] == "realm_id"), None)
        for index, field in enumerate(fields):
            if field.get("type") != "number":
                continue
            label = field.get("labels", {}).get("realm_id")
            for row_index, value in enumerate(columns[index]):
                realm = label if label is not None else columns[realm_column][row_index] if realm_column is not None else None
                if realm is not None:
                    values[realm] = value
    return values


def query_acceptance(runtime, expected_path):
    private = json.loads((runtime / "instance-private.json").read_text(encoding="utf-8"))
    if not private["pg_json"]:
        raise RuntimeError("Read-only PostgreSQL was not explicitly wired into this owned instance")
    connection_health = api(runtime, "/api/datasources/uid/mir2-economy-readonly/health")
    if connection_health.get("status") != "OK":
        save(runtime / "datasource-health-private.json", connection_health)
        raise RuntimeError("PostgreSQL datasource health did not pass with verify-full")
    expected = json.loads(expected_path.read_text(encoding="utf-8"))
    prefix = expected["fixture_prefix"]
    if not prefix.startswith("hunting-observer-test-grafana-") or not expected.get("synthetic_only"):
        raise RuntimeError("Expected data is not a synthetic Grafana acceptance fixture")
    dashboard = api(runtime, "/api/dashboards/uid/mir2-hunting-permit")["dashboard"]
    chosen_realm = next(iter(expected["expected"]))
    literal_realm = "'" + chosen_realm.replace("'", "''") + "'"
    panel_queries = 0
    for panel in dashboard["panels"]:
        for target in panel.get("targets", []):
            target = json.loads(json.dumps(target))
            target["rawSql"] = target["rawSql"].replace("${realm:sqlstring}", literal_realm)
            target.update({"intervalMs": 60000, "maxDataPoints": 10000})
            query(runtime, [target])
            panel_queries += 1
    # Evaluate the models actually stored by native provisioning, rather than
    # merely sending a corrected local copy that could hide stale live rules.
    rules = api(runtime, "/api/v1/provisioning/alert-rules")
    raw_assertions = 0
    expression_assertions = 0
    null_assertions = 0
    for rule in rules:
        name = rule["labels"]["economy_rule"]
        queries = []
        for entry in rule["data"]:
            model = json.loads(json.dumps(entry["model"]))
            if entry["datasourceUid"] == "__expr__":
                model["datasource"] = {"uid": "__expr__", "type": "__expr__"}
            queries.append(model)
        response = query(runtime, queries)
        # Keep detailed native frames private. Repository evidence contains only
        # synthetic fixture namespace/counts and validation conclusions.
        save(runtime / ("query-" + name + "-private.json"), response)
        raw_values = values_by_realm(response, "A")
        threshold_values = values_by_realm(response, "C")
        for realm, cases in expected["expected"].items():
            if name not in cases:
                continue
            target = cases[name]
            if realm not in raw_values or raw_values[realm] != target:
                raise RuntimeError("Actual Grafana SQL result differs from its synthetic expected value")
            raw_assertions += 1
            if realm not in threshold_values or threshold_values[realm] != target:
                raise RuntimeError("Actual Grafana threshold does not preserve the expected synthetic value/NULL")
            expression_assertions += 1
            null_assertions += target is None
    datasource = api(runtime, "/api/datasources/uid/mir2-economy-readonly")
    return {"grafana_version": VERSION, "synthetic_only": True, "fixture_prefix": prefix,
            "fixture_realms": len(expected["expected"]), "panel_queries_executed": panel_queries,
            "alert_sql_assertions": raw_assertions, "alert_native_threshold_assertions": expression_assertions,
            "explicit_null_preservation_assertions": null_assertions,
            "fixture_sql_sha256": expected.get("fixture_sql_sha256"),
            "evaluated_models_from_native_provisioning_api": True,
            "economic_window": "current complete economic days from server now(), Asia/Shanghai 04:00; no historical leap-day claim",
            "sslmode": datasource["jsonData"]["sslmode"], "tls_configuration_method": datasource["jsonData"].get("tlsConfigurationMethod"),
            "datasource_queries_executed": True, "notifications_sent": False,
            "production_coverage_claimed": False}


def stop(runtime):
    """Stop exactly this owned executable/config, retaining all private evidence."""
    private = json.loads((runtime / "instance-private.json").read_text(encoding="utf-8"))
    pid = int(private["pid"])
    if os.name != "nt":
        raise RuntimeError("This Windows acceptance helper does not stop non-Windows processes")
    # Trusted script on stdin avoids shell construction from paths/passwords.
    script = f"$p=Get-CimInstance Win32_Process -Filter 'ProcessId = {pid}'; if ($null -eq $p) {{ exit 3 }}; [Console]::Write((@{{path=$p.ExecutablePath; command=$p.CommandLine}} | ConvertTo-Json -Compress))"
    result = subprocess.run(["powershell.exe", "-NoProfile", "-Command", "-"], input=script, text=True, capture_output=True)
    if result.returncode == 3:
        return {"owned_pid": pid, "already_stopped": True}
    if result.returncode:
        raise RuntimeError("Unable to verify owned Grafana process before stopping")
    process = json.loads(result.stdout)
    if Path(process["path"]).resolve() != Path(private["binary"]).resolve() or private["config"] not in process["command"]:
        raise RuntimeError("Refusing to stop PID whose executable/config do not match our private state")
    # Native datasource executables inherit the log handle on Windows. Stop only
    # verified direct children inside our private distribution before the server.
    child_script = f"$items=@(Get-CimInstance Win32_Process -Filter 'ParentProcessId = {pid}' | ForEach-Object {{ @{{pid=$_.ProcessId; path=$_.ExecutablePath}} }}); [Console]::Write((ConvertTo-Json -InputObject $items -Compress))"
    children_result = subprocess.run(["powershell.exe", "-NoProfile", "-Command", "-"], input=child_script, text=True, capture_output=True)
    if children_result.returncode:
        raise RuntimeError("Unable to inspect owned backend children before stopping")
    children = json.loads(children_result.stdout)
    stopped_backends = 0
    for child in children:
        if not child["path"] or not Path(child["path"]).resolve().is_relative_to(runtime.resolve()):
            continue
        child_id = int(child["pid"])
        path_literal = "'" + child["path"].replace("'", "''") + "'"
        child_stop = f"$p=Get-CimInstance Win32_Process -Filter 'ProcessId = {child_id}'; if ($null -eq $p) {{ exit 0 }}; if ($p.ParentProcessId -ne {pid} -or $p.ExecutablePath -ne {path_literal}) {{ exit 4 }}; Stop-Process -Id {child_id} -ErrorAction Stop"
        stopped_child = subprocess.run(["powershell.exe", "-NoProfile", "-Command", "-"], input=child_stop, text=True, capture_output=True)
        if stopped_child.returncode:
            raise RuntimeError("Backend child identity changed; refusing further shutdown")
        stopped_backends += 1
    stopped = subprocess.run(["powershell.exe", "-NoProfile", "-Command", f"Stop-Process -Id {pid} -ErrorAction Stop"], capture_output=True)
    if stopped.returncode:
        raise RuntimeError("Unable to stop verified owned Grafana process")
    return {"owned_pid": pid, "owned_backend_processes_stopped": stopped_backends, "stopped": True, "private_runtime_retained": True}


def restart(runtime, pg_json):
    identity = json.loads((runtime / "instance-private.json").read_text(encoding="utf-8"))
    if "secret_key" not in identity:
        configuration = configparser.ConfigParser(interpolation=None)
        configuration.read(identity["config"], encoding="utf-8")
        identity["secret_key"] = configuration["security"]["secret_key"]
    stop(runtime)
    history = (runtime / "private-history" / str(time.time_ns())).resolve()
    if not history.is_relative_to(runtime.resolve()):
        raise RuntimeError("Refusing private history path outside our runtime")
    history.mkdir(parents=True)
    for name in ("instance-private.json", "server.stdout.log", "server.stderr.log"):
        source = runtime / name
        if source.exists():
            source.replace(history / name)
    return start(runtime, pg_json, identity)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("action", choices=["download", "start", "restart", "verify", "fixture-sql", "query", "stop"])
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--pg-json", type=Path)
    parser.add_argument("--direct-download", action="store_true", help="Bypass environment proxies for the pinned public download only; TLS verification stays enabled")
    parser.add_argument("--download-workers", type=int, default=1)
    parser.add_argument("--download-tls12", action="store_true", help="Use TLS 1.2 for the pinned public archive when a proxy breaks TLS 1.3; certificate verification remains required")
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--fixture-prefix")
    parser.add_argument("--sql-output", type=Path)
    parser.add_argument("--expected", type=Path)
    args = parser.parse_args()
    runtime = owned(args.runtime)
    if args.action == "fixture-sql":
        if not args.fixture_prefix or not args.sql_output or not args.expected:
            raise RuntimeError("Fixture rendering requires prefix, SQL output and expected output paths")
        script, result = fixture_sql(args.fixture_prefix)
        args.sql_output.parent.mkdir(parents=True, exist_ok=True)
        args.sql_output.write_text(script, encoding="utf-8")
        # Hash the bytes actually written, including native Windows newlines.
        result["fixture_sql_sha256"] = hashlib.file_digest(args.sql_output.open("rb"), "sha256").hexdigest()
        save(args.expected, result)
        result = {"fixture_prefix": args.fixture_prefix, "synthetic_only": True, "fixture_realms": len(result["expected"]), "sql_executed": False}
    elif args.action == "download":
        result = download(runtime, args.direct_download, args.download_workers, args.download_tls12)
    elif args.action == "start":
        result = start(runtime, args.pg_json)
    elif args.action == "restart":
        result = restart(runtime, args.pg_json)
    elif args.action == "verify":
        result = verify(runtime)
    elif args.action == "query":
        if args.expected is None:
            raise RuntimeError("Native query acceptance requires a synthetic expected-values file")
        result = query_acceptance(runtime, args.expected)
    else:
        result = stop(runtime)
    if args.evidence:
        save(args.evidence, result)
    print(json.dumps(result, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    main()
