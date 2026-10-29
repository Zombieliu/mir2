#!/usr/bin/env python3
"""Run real Gateway async_main with an empty isolated native PG/Redis fixture.

No mock startup, file-backend fallback, player seed, chain driver or public route.
The database URL is read only from the CI-created environment, never argv/logs.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import signal
import socket
import subprocess
import sys
import tempfile
import time
from urllib.parse import unquote, urlsplit
from urllib.request import ProxyHandler, build_opener


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def port():
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


def file_sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def psql(binary, url, statement):
    value = urlsplit(url)
    env = {"PATH": os.environ["PATH"], "LANG": "C.UTF-8",
           "PGHOST": value.hostname, "PGPORT": str(value.port),
           "PGUSER": unquote(value.username or ""),
           "PGDATABASE": unquote(value.path.removeprefix("/"))}
    if value.password is not None:
        env["PGPASSWORD"] = unquote(value.password)
    result = subprocess.run([str(binary), "-X", "-qAt", "-v", "ON_ERROR_STOP=1"],
                            input=statement.encode(), stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, env=env, timeout=20)
    require(result.returncode == 0, "isolated PostgreSQL assertion failed")
    return result.stdout.decode().strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("binary", "maps", "psql", "redis-binary", "evidence"):
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--revision", required=True)
    args = parser.parse_args()
    require(sys.platform == "linux" and os.geteuid() != 0,
            "native startup regression requires the non-root Linux CI publisher")
    require(re.fullmatch(r"[0-9a-f]{40}", args.revision), "exact source revision required")
    for binary in (args.binary, args.psql, args.redis_binary):
        require(binary.is_absolute() and binary.is_file() and os.access(binary, os.X_OK),
                "reviewed native executable missing")
    require(args.maps.is_dir() and (args.maps / "0.map.gz").is_file(), "real source maps required")
    require(args.evidence.is_dir(), "existing evidence directory required")
    database_url = os.environ.get("MIR2_STARTUP_TEST_DATABASE_URL", "")
    database = urlsplit(database_url)
    require(database.scheme in ("postgres", "postgresql")
            and database.hostname == "127.0.0.1" and database.port is not None
            and database.username == "startup_qa" and not database.query and not database.fragment
            and re.fullmatch(r"/mir2_startup_qa_[0-9_]+", database.path),
            "only the fresh private CI startup database is allowed")
    require(psql(args.psql, database_url,
                 "SELECT NOT rolsuper AND NOT rolcreatedb AND NOT rolcreaterole AND NOT rolreplication FROM pg_roles WHERE rolname=current_user;") == "t",
            "startup fixture must use its low-privilege owner")
    require(psql(args.psql, database_url,
                 "SELECT count(*) FROM pg_tables WHERE schemaname='public';") == "0",
            "startup fixture database must be fresh and empty")
    receipt = args.evidence / "gateway-postgres-startup.json"
    require(not receipt.exists(), "fresh startup receipt required")
    local_http = build_opener(ProxyHandler({}))

    with tempfile.TemporaryDirectory(prefix="mir2-gateway-startup-") as owned:
        root = Path(owned)
        state, recovery, redis_dir = (root / leaf for leaf in ("state", "recovery", "redis"))
        for directory in (state, recovery, redis_dir):
            directory.mkdir(mode=0o700)
        web_port, tcp_port, redis_port = port(), port(), port()
        require(len({web_port, tcp_port, redis_port}) == 3, "isolated ports collided")
        base_env = {"PATH": os.environ["PATH"], "LANG": "C.UTF-8", "HOME": str(root)}
        env = dict(base_env, MIR2_ENV="staging", MIR2_PLAYTEST_TESTNET="1",
                   MIR2_DEPLOY_REVISION=args.revision,
                   MIR2_GATEWAY_WEB_ADDR=f"127.0.0.1:{web_port}",
                   MIR2_GATEWAY_TCP_ADDR=f"127.0.0.1:{tcp_port}",
                   MIR2_ACCOUNT_STORE_BACKEND="postgres", MIR2_ACCOUNT_STORE_REQUIRE_POSTGRES="1",
                   MIR2_ACCOUNT_STORE_DATABASE_URL=database_url,
                   MIR2_ACCOUNT_STORE_PATH=str(state / "accounts.json"),
                   MIR2_SAVE_RECOVERY_DIR=str(recovery), MIR2_SAVE_RECOVERY_MAC_KEY=secrets.token_hex(32),
                   MIR2_IDENTITY_POLICY="commercial", MIR2_PASSKEY_AUTH_SECRET=secrets.token_hex(32),
                   MIR2_IDENTITY_SESSION_SECRET=secrets.token_hex(32),
                   MIR2_IDENTITY_RECOVERY_PEPPER=secrets.token_hex(32),
                   MIR2_CHANNEL_IDENTITY_DATABASE_URL=database_url,
                   MIR2_REQUIRE_CHANNEL_IDENTITY_POSTGRES="1", MIR2_REQUIRE_CHANNEL_IDENTITY_STORE="1",
                   MIR2_GATEWAY_REQUIRE_REDIS_CACHE="1",
                   MIR2_GATEWAY_REDIS_CACHE_URL=f"redis://127.0.0.1:{redis_port}",
                   MIR2_GATEWAY_ID="isolated-native-startup", MIR2_REALM_ID="isolated-native-startup",
                   MIR2_ALLOWED_WEB_ORIGINS="https://startup.invalid",
                   MIR2_GATEWAY_TOKIO_WORKER_THREADS="2", MIR2_CONTENT_PROFILE="platinum_176",
                   MIR2_CRYSTAL_MAP_PACK=str(args.maps), MIR2_QUEST_CADENCE="newcomer-v2",
                   MIR2_SIMULATION_FIXED_LIGHT_SETTING="day", MIR2_PROCUREMENT_ENABLED="0",
                   MIR2_PROCUREMENT_TEST_CREDITS="0", MIR2_SUI_USDC_ENABLED="0", MIR2_STRIPE_ENABLED="0",
                   MIR2_SPECTATOR_ENABLED="0", MIR2_SPECTATOR_RECORDING_ENABLED="0",
                   MIR2_SPECTATOR_PUBLIC="0", MIR2_AI_LIVE_ENABLED="0")
        gateway = redis = None
        started = time.monotonic()
        try:
            with (args.evidence / "startup-redis.log").open("xb") as redis_log, \
                    (args.evidence / "startup-gateway.log").open("xb") as gateway_log:
                redis = subprocess.Popen([str(args.redis_binary), "--bind", "127.0.0.1",
                    "--port", str(redis_port), "--protected-mode", "yes", "--save", "",
                    "--appendonly", "no", "--dir", str(redis_dir)],
                    env=base_env, stdout=redis_log, stderr=subprocess.STDOUT)
                for _ in range(100):
                    require(redis.poll() is None, "owned isolated Redis exited before startup")
                    try:
                        with socket.create_connection(("127.0.0.1", redis_port), timeout=0.2) as connection:
                            connection.sendall(b"*1\r\n$4\r\nPING\r\n")
                            if connection.recv(64) == b"+PONG\r\n":
                                break
                    except OSError:
                        time.sleep(0.05)
                else:
                    raise RuntimeError("owned isolated Redis did not become ready")
                gateway = subprocess.Popen([str(args.binary)], cwd=state, env=env,
                    stdin=subprocess.DEVNULL, stdout=gateway_log, stderr=subprocess.STDOUT)
                health = None
                for _ in range(300):
                    require(gateway.poll() is None, "real Gateway exited during async_main startup; inspect startup-gateway.log")
                    try:
                        with local_http.open(f"http://127.0.0.1:{web_port}/health", timeout=1) as response:
                            health = json.load(response)
                        break
                    except (OSError, ValueError):
                        time.sleep(0.1)
                require(health is not None, "real Gateway never became ready")
                cache, channel = health.get("session_cache", {}), health.get("channel_identity", {})
                require(health.get("ok") is True and health.get("revision") == args.revision
                        and health.get("http") == "ready" and health.get("ws") == "ready"
                        and health.get("identity_backend") == "postgres"
                        and cache.get("backend") == "redis" and cache.get("healthy") is True
                        and cache.get("configured") is True and channel.get("backend") == "postgres"
                        and channel.get("durable") is True and health.get("gate15") is None,
                        "full staging PostgreSQL/Redis health contract failed")
                gateway.send_signal(signal.SIGINT)
                require(gateway.wait(timeout=60) == 0, "Gateway did not exit cleanly after real startup")
                redis.terminate()
                require(redis.wait(timeout=10) == 0, "isolated Redis did not stop cleanly")
            require(psql(args.psql, database_url, "SELECT count(*) FROM accounts;") == "0",
                    "startup must not invent a demo/player account")
            value = {"schema": "mir2.playtest.postgres-async-startup.v1", "passed": True,
                     "sourceRevision": args.revision, "binarySha256": file_sha256(args.binary),
                     "actualReleaseSubprocess": True, "procurementNewSalesEnabled": False,
                     "usdcNewSalesEnabled": False, "chainDriversConfigured": False,
                     "identityBackend": health["identity_backend"], "channelIdentity": channel,
                     "sessionCacheBackend": cache["backend"], "sessionCacheHealthy": cache["healthy"],
                     "gatewayExitCode": gateway.returncode, "redisExitCode": redis.returncode,
                     "playerAccountsCreated": 0, "elapsedMs": round((time.monotonic()-started)*1000),
                     "containersStarted": False, "publicServiceContacted": False,
                     "credentialsRecorded": False}
            with receipt.open("x", encoding="utf-8") as stream:
                json.dump(value, stream, indent=2)
                stream.write("\n")
            print("PASS: real release async_main with isolated low-privilege PostgreSQL/Redis, both recovery features off, clean SIGINT shutdown")
        finally:
            # Only child process objects created above are eligible for cleanup.
            for child in (gateway, redis):
                if child is not None and child.poll() is None:
                    child.terminate()
                    try:
                        child.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        child.kill()
                        child.wait(timeout=10)


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, OSError, ValueError, subprocess.SubprocessError) as error:
        print("FAIL: native PostgreSQL async startup: " + str(error), file=sys.stderr)
        sys.exit(1)
