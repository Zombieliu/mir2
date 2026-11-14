#!/usr/bin/env python3
"""Offline, fail-closed proposed-config checks; never deploys or contacts services.

Usage: python scripts/verify_zone_cluster_config.py PLAN.json [--previous OLD.json]
PLAN is private: explicit {schemaVersion:1, profile:PROFILE, components:[
  {id, hostId, role:"gateway"|"zone_host", env:{actual MIR2 key:string}}]}.
The unittest fixture is the runnable, synthetic example. No commands from JSON.
The env object lists exactly this profile's MIR2 application keys, not a full
process environment or proof of environment clearing by a future launcher.
Only static IPv4 2GW/2ZoneHost, inline per_map and direct PG/Redis are supported.
No inherited environment, env-file includes, DNS, wildcard binds, Gate15, Sentinel,
HA, TLS or dynamic Cells are inferred. PASS is config compatibility, not readiness.
PG database must match mir2_cap[0-9_]* or mir2_capacity_* (not a privilege check).
Direct unauthenticated private Redis is only this isolated test profile, not a
production security recommendation. Metrics are loopback-only. Host save recovery
is not claimed: zone_host does not consume Gateway MIR2_SAVE_RECOVERY_DIR.
No runtime content hashes, port ownership, path symlinks or identity are verified.
One nonloopback listen IP cannot be claimed by different declared physical hosts.
A host may declare multiple IPs; their actual ownership is not verified offline.

Source contracts: main.rs:25-60, session.rs:1633, topology.rs:55-145,
zone_rpc.rs:1499,7747; zone_lease.rs:151,679; cache.rs:1527,1574;
operator.rs:64-128; web.rs:9039. All are apps/gateway/src paths.
"""
import argparse
import hashlib
import ipaddress
import json
import re
import sys
from pathlib import Path, PurePosixPath
from urllib.parse import urlsplit, unquote

PROFILE = "static-2gw-2host-pg-v1"
MAX_BYTES = 1024 * 1024
SLUG = re.compile(r"[a-z][a-z0-9_-]{0,47}\Z")
PRIVATE = tuple(ipaddress.ip_network(n) for n in ("10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16", "127.0.0.0/8"))
COMMON = {
    "MIR2_ENV", "MIR2_IDENTITY_POLICY", "MIR2_ACCOUNT_STORE_BACKEND",
    "MIR2_ACCOUNT_STORE_REQUIRE_POSTGRES", "MIR2_ACCOUNT_STORE_DATABASE_URL",
    "MIR2_ACCOUNT_STORE_PATH", "MIR2_GATEWAY_ZONE_LEASE_DATABASE_URL",
    "MIR2_GATEWAY_ZONE_LEASE_OWNER_ID", "MIR2_GATEWAY_INSTANCE_ID",
    "MIR2_ZONE_HOST_TOKEN", "MIR2_ZONE_TOPOLOGY_JSON", "MIR2_CRYSTAL_MAP_PACK",
}
GATEWAY = {
    "MIR2_GATEWAY_WEB_ADDR", "MIR2_GATEWAY_TCP_ADDR", "MIR2_GATEWAY_ID",
    "MIR2_CONTENT_PROFILE", "MIR2_ZONE_HOST_ACTIVE_ADDRS",
    "MIR2_CHANNEL_IDENTITY_DATABASE_URL", "MIR2_REQUIRE_CHANNEL_IDENTITY_POSTGRES",
    "MIR2_REQUIRE_CHANNEL_IDENTITY_STORE", "MIR2_GATEWAY_REQUIRE_REDIS_CACHE",
    "MIR2_GATEWAY_REDIS_CACHE_URL", "MIR2_PASSKEY_AUTH_SECRET",
    "MIR2_IDENTITY_SESSION_SECRET", "MIR2_IDENTITY_RECOVERY_PEPPER",
    "MIR2_SAVE_RECOVERY_DIR", "MIR2_SAVE_RECOVERY_MAC_KEY", "MIR2_ALLOWED_WEB_ORIGINS",
    "MIR2_PROCUREMENT_ENABLED", "MIR2_PROCUREMENT_TEST_CREDITS", "MIR2_SUI_USDC_ENABLED",
    "MIR2_STRIPE_ENABLED", "MIR2_SPECTATOR_ENABLED", "MIR2_SPECTATOR_RECORDING_ENABLED",
    "MIR2_SPECTATOR_PUBLIC", "MIR2_AI_LIVE_ENABLED",
}
HOST = {"MIR2_ZONE_HOST_ID", "MIR2_ZONE_HOST_ADDR", "MIR2_ZONE_HOST_METRICS_ADDR", "MIR2_ZONE_HOST_CRYSTAL_WORLD"}
OFF = {k for k in GATEWAY if k.endswith("_ENABLED") or k in ("MIR2_PROCUREMENT_TEST_CREDITS", "MIR2_SPECTATOR_PUBLIC")}
SHARED_AUTH = ("MIR2_PASSKEY_AUTH_SECRET", "MIR2_IDENTITY_SESSION_SECRET", "MIR2_IDENTITY_RECOVERY_PEPPER")


class Invalid(Exception):
    """Never includes private input or parser errors."""


def require(ok):
    if not ok:
        raise Invalid()


def pairs(items):
    out = {}
    for key, value in items:
        require(key not in out)
        out[key] = value
    return out


def strict_json(text):
    return json.loads(text, object_pairs_hook=pairs, parse_constant=lambda _: (_ for _ in ()).throw(Invalid()))


def load(path):
    with Path(path).open("rb") as stream:
        data = stream.read(MAX_BYTES + 1)
    require(len(data) <= MAX_BYTES)
    return strict_json(data.decode("utf-8"))


def ipv4(value):
    ip = ipaddress.ip_address(value)
    require(ip.version == 4 and any(ip in n for n in PRIVATE) and str(ip) == value)
    return ip


def endpoint(value):
    require(isinstance(value, str) and value.count(":") == 1)
    address, port = value.split(":")
    ipv4(address)
    require(port.isdecimal() and str(int(port)) == port and 1024 <= int(port) <= 65535)
    return address, int(port)


def pg_authority(value):
    require(value.startswith(("postgres://", "postgresql://")))
    u = urlsplit(value)
    require(u.scheme in ("postgres", "postgresql") and u.hostname is not None and u.port is not None)
    ipv4(u.hostname)
    require(1024 <= u.port <= 65535)
    require(u.username and u.password and not u.query and not u.fragment)
    require(re.fullmatch(r"/[A-Za-z_][A-Za-z0-9_]{0,62}", unquote(u.path)) is not None)
    require(re.fullmatch(r"/(mir2_cap[0-9_][a-z0-9_]*|mir2_capacity_[a-z0-9_]+)", unquote(u.path)) is not None)
    return u.hostname, u.port, unquote(u.path)


def redis_authority(value):
    # cache.rs strips /suffix and does not implement AUTH/SELECT: don't pretend it does.
    u = urlsplit(value)
    require(u.scheme == "redis" and not u.username and not u.password and u.port is not None)
    require(not u.path and not u.query and not u.fragment)
    ipv4(u.hostname)
    require(value == "redis://" + u.hostname + ":" + str(u.port) and 1024 <= u.port <= 65535)
    return u.hostname, u.port


def directory(value):
    p = PurePosixPath(value)
    require(value.startswith("/") and not value.startswith("//") and str(p) == value)
    require(".." not in p.parts and len(p.parts) >= 4)
    return p


def topology(value):
    t = strict_json(value)
    require(isinstance(t, dict) and set(t) == {"version", "mode", "defaultZoneId", "defaultTickMs", "zones", "hotMaps"})
    require(type(t["version"]) is int and t["version"] == 1 and t["mode"] == "per_map" and t["hotMaps"] == {})
    require(type(t["defaultTickMs"]) is int and 10 <= t["defaultTickMs"] <= 5000)
    require(isinstance(t["defaultZoneId"], str) and SLUG.fullmatch(t["defaultZoneId"]))
    require(isinstance(t["zones"], dict) and len(t["zones"]) == 2)
    assigned = set()
    for name, group in t["zones"].items():
        require(SLUG.fullmatch(name) and isinstance(group, dict) and set(group) == {"maps", "tickMs"})
        require(type(group["tickMs"]) is int and 10 <= group["tickMs"] <= 5000)
        require(isinstance(group["maps"], list) and group["maps"])
        for m in group["maps"]:
            require(isinstance(m, str) and re.fullmatch(r"[A-Za-z0-9_-]{1,80}", m) and m not in assigned)
            assigned.add(m)
    # Exact static algorithm from zone_rpc.rs:7747, without :line: (unsupported here).
    require({int.from_bytes(hashlib.sha256(z.encode()).digest()[:8], "big") % 2 for z in t["zones"]} == {0, 1})
    return t


def _validate(plan):
    issues = []
    def issue(code, cid=None):
        item = {"componentId": cid, "code": code}
        if item not in issues:
            issues.append(item)
    if not isinstance(plan, dict) or set(plan) != {"schemaVersion", "profile", "components"}:
        return [{"componentId": None, "code": "unsupportedManifest"}], None
    if type(plan["schemaVersion"]) is not int or plan["schemaVersion"] != 1 or plan["profile"] != PROFILE:
        return [{"componentId": None, "code": "unsupportedProfile"}], None
    cs = plan["components"]
    if not isinstance(cs, list) or len(cs) != 4:
        return [{"componentId": None, "code": "requiresTwoGatewaysTwoHosts"}], None
    ids, instances, owners, host_ids, gateway_ids = set(), set(), set(), set(), set()
    listeners, paths, gateways, hosts = [], [], [], []
    pg, tops, auth, tokens, redis, origins_policy = [], [], [], [], [], []
    facts = {"active": [], "topology": [], "owners": [], "pg": [], "redis": [], "locations": {}}
    physical = set()
    for c in cs:
        if not isinstance(c, dict) or set(c) != {"id", "hostId", "role", "env"}:
            issue("unsupportedComponent"); continue
        cid = c["id"]
        if not isinstance(cid, str) or not SLUG.fullmatch(cid):
            issue("invalidComponentId"); continue
        if cid in ids: issue("duplicateComponentId", cid)
        ids.add(cid)
        if not isinstance(c["hostId"], str) or not SLUG.fullmatch(c["hostId"]):
            issue("invalidPhysicalHostId", cid); continue
        physical.add(c["hostId"])
        role, env = c["role"], c["env"]
        if role not in ("gateway", "zone_host") or not isinstance(env, dict):
            issue("unsupportedRole", cid); continue
        allowed = COMMON | (GATEWAY if role == "gateway" else HOST)
        if set(env) != allowed:
            # Never emit an unknown key/value: it may itself contain a credential.
            issue("unsupportedOrIncompleteEnvironment", cid); continue
        if not all(isinstance(v, str) and v and len(v) <= 32768 and not any(ord(ch) < 32 or ord(ch) == 127 for ch in v) for v in env.values()):
            issue("invalidEnvironmentValue", cid); continue
        try:
            require(env["MIR2_ENV"] == "staging" and env["MIR2_IDENTITY_POLICY"] == "commercial")
            require(env["MIR2_ACCOUNT_STORE_BACKEND"] == "postgres" and env["MIR2_ACCOUNT_STORE_REQUIRE_POSTGRES"] == "1")
        except Invalid: issue("unsafeAuthOrPersistenceProfile", cid)
        for key, seen in (("MIR2_GATEWAY_INSTANCE_ID", instances), ("MIR2_GATEWAY_ZONE_LEASE_OWNER_ID", owners)):
            val = env[key]
            if not SLUG.fullmatch(val) or val in seen: issue("nonUniqueRuntimeIdentity", cid)
            seen.add(val)
        try:
            a = pg_authority(env["MIR2_ACCOUNT_STORE_DATABASE_URL"])
            require(a == pg_authority(env["MIR2_GATEWAY_ZONE_LEASE_DATABASE_URL"]))
            if role == "gateway": require(a == pg_authority(env["MIR2_CHANNEL_IDENTITY_DATABASE_URL"]))
            pg.append(a); facts["pg"].append(a)
        except (Invalid, ValueError, TypeError): issue("unsupportedOrMismatchedPostgresAuthority", cid)
        try:
            t = topology(env["MIR2_ZONE_TOPOLOGY_JSON"]); tops.append(t); facts["topology"].append(t)
        except (Invalid, ValueError, TypeError, AttributeError): issue("unsupportedOrInvalidTopology", cid)
        try:
            token = env["MIR2_ZONE_HOST_TOKEN"]; require(len(token.encode()) >= 32); tokens.append(token)
        except Invalid: issue("weakRpcToken", cid)
        try:
            account = directory(env["MIR2_ACCOUNT_STORE_PATH"])
            require(account.suffix == ".json")
            paths.append((c["hostId"], account.parent, cid))
            directory(env["MIR2_CRYSTAL_MAP_PACK"])
        except (Invalid, TypeError): issue("unsupportedStateOrContentPath", cid)
        bind_keys = ("MIR2_GATEWAY_WEB_ADDR", "MIR2_GATEWAY_TCP_ADDR") if role == "gateway" else ("MIR2_ZONE_HOST_ADDR", "MIR2_ZONE_HOST_METRICS_ADDR")
        facts["locations"][cid] = {
            "role": role, "hostId": c["hostId"],
            "listen": {key: env[key] for key in bind_keys},
            "accountStorePath": env["MIR2_ACCOUNT_STORE_PATH"],
            "saveRecoveryDir": env.get("MIR2_SAVE_RECOVERY_DIR"),
        }
        for key in bind_keys:
            try:
                ep = endpoint(env[key])
                if key == "MIR2_ZONE_HOST_METRICS_ADDR": require(ipv4(ep[0]).is_loopback)
                listeners.append((c["hostId"], ep, cid, key))
            except (Invalid, ValueError, TypeError): issue("unsupportedListenAddress", cid)
        if role == "gateway":
            gateways.append(c)
            try:
                require(env["MIR2_CONTENT_PROFILE"] == "crystal_full")
                require(all(env[k] == "1" for k in ("MIR2_REQUIRE_CHANNEL_IDENTITY_POSTGRES", "MIR2_REQUIRE_CHANNEL_IDENTITY_STORE", "MIR2_GATEWAY_REQUIRE_REDIS_CACHE")))
                require(all(env[k] == "0" for k in OFF))
                require(all(len(env[k].encode()) >= 32 for k in (*SHARED_AUTH, "MIR2_SAVE_RECOVERY_MAC_KEY")))
                require(re.fullmatch(r"[0-9a-fA-F]{64}", env["MIR2_SAVE_RECOVERY_MAC_KEY"]))
                # SimulationConfig::with_save_recovery_mac_key:32 decoded bytes, >=8 distinct.
                require(len(set(bytes.fromhex(env["MIR2_SAVE_RECOVERY_MAC_KEY"]))) >= 8)
                auth.append(tuple(env[k] for k in SHARED_AUTH))
                paths.append((c["hostId"], directory(env["MIR2_SAVE_RECOVERY_DIR"]), cid))
                val = env["MIR2_GATEWAY_ID"]
                require(SLUG.fullmatch(val) and val not in gateway_ids); gateway_ids.add(val)
            except (Invalid, ValueError, TypeError): issue("unsafeOrUnsupportedGatewayProfile", cid)
            try:
                r = redis_authority(env["MIR2_GATEWAY_REDIS_CACHE_URL"]); redis.append(r); facts["redis"].append(r)
            except (Invalid, ValueError, TypeError): issue("unsupportedRedisAuthority", cid)
            try:
                parts = env["MIR2_ZONE_HOST_ACTIVE_ADDRS"].split(",")
                require(len(parts) == 2 and len(set(parts)) == 2 and all(p == p.strip() for p in parts))
                for part in parts: endpoint(part)
                facts["active"].append(parts)
            except (Invalid, ValueError, TypeError): issue("unsupportedActiveHosts", cid)
            try:
                origins = env["MIR2_ALLOWED_WEB_ORIGINS"].split(",")
                for origin in origins:
                    u = urlsplit(origin); require(u.scheme == "http" and not u.path and not u.query and not u.fragment and not u.username and not u.password)
                    endpoint(u.netloc)
                origins_policy.append(sorted(origins))
            except (Invalid, ValueError, TypeError): issue("unsupportedBrowserOrigin", cid)
        else:
            hosts.append(c)
            val = env["MIR2_ZONE_HOST_ID"]
            if not SLUG.fullmatch(val) or val in host_ids or val != env["MIR2_GATEWAY_ZONE_LEASE_OWNER_ID"]:
                issue("invalidZoneOwnerIdentity", cid)
            host_ids.add(val)
            facts["owners"].append((env["MIR2_ZONE_HOST_ADDR"], val, c["hostId"]))
            if env["MIR2_ZONE_HOST_CRYSTAL_WORLD"] != "1": issue("unsupportedHostContentProfile", cid)
    if len(gateways) != 2 or len(hosts) != 2: issue("requiresTwoGatewaysTwoHosts")
    for values, code in ((pg, "postgresAuthorityMismatch"), (tops, "topologyMismatch"), (tokens, "rpcTokenMismatch"), (auth, "gatewayAuthSecretMismatch"), (redis, "redisAuthorityMismatch"), (facts["active"], "orderedActiveHostsMismatch"), (origins_policy, "browserOriginPolicyMismatch")):
        if values and any(v != values[0] for v in values[1:]): issue(code)
    if facts["active"] and {h["env"]["MIR2_ZONE_HOST_ADDR"] for h in hosts} != set(facts["active"][0]): issue("activeEndpointsDoNotMatchHosts")
    ip_claims = {}
    for i, (host, ep, cid, key) in enumerate(listeners):
        if not ipv4(ep[0]).is_loopback:
            if ep[0] in ip_claims and ip_claims[ep[0]] != host:
                issue("nonLoopbackAddressClaimedByMultipleHosts", cid)
            else:
                ip_claims[ep[0]] = host
        if any(host == oh and ep[1] == oe[1] for oh, oe, _, _ in listeners[:i]): issue("physicalHostListenPortConflict", cid)
        if key != "MIR2_ZONE_HOST_METRICS_ADDR" and len(physical) > 1 and ipv4(ep[0]).is_loopback: issue("crossHostLoopbackRouting", cid)
    if len(physical) > 1 and any(ipv4(a[0]).is_loopback for a in pg + redis): issue("crossHostLoopbackAuthority")
    for i, (host, p, cid) in enumerate(paths):
        if any(host == oh and (p == op or p in op.parents or op in p.parents) for oh, op, _ in paths[:i]): issue("physicalHostStateRecoveryConflict", cid)
    return issues, facts


def raw_active_arrays(plan):
    # Detect re-sharding even when a new 3-host plan exceeds this 2-host profile.
    arrays = []
    if isinstance(plan, dict) and isinstance(plan.get("components"), list):
        for c in plan["components"]:
            if isinstance(c, dict) and c.get("role") == "gateway" and isinstance(c.get("env"), dict):
                raw = c["env"].get("MIR2_ZONE_HOST_ACTIVE_ADDRS")
                if isinstance(raw, str): arrays.append(raw.split(","))
    return sorted(arrays)


def verify(plan, previous=None):
    issues, facts = _validate(plan)
    migration = False
    if previous is not None:
        old_issues, old = _validate(previous)
        if old_issues: issues.append({"componentId": None, "code": "invalidPreviousConfiguration"})
        else:
            migration = raw_active_arrays(previous) != raw_active_arrays(plan)
        if not old_issues and facts is not None:
            if old["locations"] != facts["locations"]: migration = True
            for key in ("active", "topology", "owners", "pg", "redis"):
                # Component order itself is irrelevant; active endpoint order is not.
                before = sorted(json.dumps(v, sort_keys=True) for v in old[key])
                after = sorted(json.dumps(v, sort_keys=True) for v in facts[key])
                if before != after: migration = True
        if migration: issues.append({"componentId": None, "code": "migrationRequired"})
    return {"schemaVersion": 1, "profile": PROFILE, "scope": "offline-isolated-static-2gw-2host-config-only", "passed": not issues,
            "migrationRequired": migration, "deployed": False, "capacityVerified": False, "highAvailabilityVerified": False,
            "runtimeFencingVerified": False, "issues": issues}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("--previous", type=Path)
    args = parser.parse_args(argv)
    try:
        value = verify(load(args.manifest), load(args.previous) if args.previous else None)
    except (Invalid, ValueError, TypeError, OSError, RecursionError):
        value = {"schemaVersion": 1, "profile": PROFILE, "passed": False,
                 "scope": "offline-isolated-static-2gw-2host-config-only", "migrationRequired": False, "deployed": False, "capacityVerified": False, "highAvailabilityVerified": False,
                 "runtimeFencingVerified": False, "issues": [{"componentId": None, "code": "invalidPrivateManifest"}]}
    print(json.dumps(value, sort_keys=True))
    return 0 if value["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
