#!/usr/bin/env python3
"""Offline fixture/negative tests. example_manifest() is a synthetic example only.

All passwords/keys below are obvious test strings, never production defaults.
Fill private per-process env snapshots outside Git; this tool cannot export a
running service environment or prove a deployment. Run with Python unittest.
"""
import copy
import contextlib
import importlib.util
import io
import json
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location("cluster_check", Path(__file__).with_name("verify_zone_cluster_config.py"))
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


def example_manifest():
    """Complete valid isolated, two-physical-host example; never connects anywhere."""
    topology = json.dumps({"version": 1, "mode": "per_map", "defaultZoneId": "primary", "defaultTickMs": 100,
                          "zones": {"zone-a": {"maps": ["0"], "tickMs": 100}, "zone-d": {"maps": ["1"], "tickMs": 100}}, "hotMaps": {}}, separators=(",", ":"))
    pg = "postgres://fixture_user:ONLY_SYNTHETIC_TEST_PASSWORD@10.20.0.10:5432/mir2_capacity_fixture"
    components = []
    for role, num in (("gateway", 1), ("gateway", 2), ("zone_host", 1), ("zone_host", 2)):
        cid = ("gateway-" if role == "gateway" else "zone-") + str(num)
        host = "host-" + str(num)
        env = {
            "MIR2_ENV": "staging", "MIR2_IDENTITY_POLICY": "commercial",
            "MIR2_ACCOUNT_STORE_BACKEND": "postgres", "MIR2_ACCOUNT_STORE_REQUIRE_POSTGRES": "1",
            "MIR2_ACCOUNT_STORE_DATABASE_URL": pg, "MIR2_GATEWAY_ZONE_LEASE_DATABASE_URL": pg,
            "MIR2_ACCOUNT_STORE_PATH": "/var/lib/capacity/" + cid + "/state/accounts.json",
            "MIR2_GATEWAY_ZONE_LEASE_OWNER_ID": cid, "MIR2_GATEWAY_INSTANCE_ID": cid,
            "MIR2_ZONE_HOST_TOKEN": "ONLY_TEST_RPC_TOKEN_NOT_FOR_DEPLOYMENT_000",
            "MIR2_ZONE_TOPOLOGY_JSON": topology, "MIR2_CRYSTAL_MAP_PACK": "/opt/capacity/crystal-map-pack",
        }
        address = "10.20.0." + str(num)
        if role == "gateway":
            env.update({
                "MIR2_GATEWAY_ID": cid, "MIR2_GATEWAY_WEB_ADDR": address + ":17410",
                "MIR2_GATEWAY_TCP_ADDR": address + ":17400", "MIR2_CONTENT_PROFILE": "crystal_full",
                "MIR2_ZONE_HOST_ACTIVE_ADDRS": "10.20.0.1:17500,10.20.0.2:17500",
                "MIR2_CHANNEL_IDENTITY_DATABASE_URL": pg, "MIR2_REQUIRE_CHANNEL_IDENTITY_POSTGRES": "1",
                "MIR2_REQUIRE_CHANNEL_IDENTITY_STORE": "1", "MIR2_GATEWAY_REQUIRE_REDIS_CACHE": "1",
                "MIR2_GATEWAY_REDIS_CACHE_URL": "redis://10.20.0.10:16481",
                "MIR2_SAVE_RECOVERY_DIR": "/var/lib/capacity/" + cid + "/recovery",
                "MIR2_SAVE_RECOVERY_MAC_KEY": bytes(range(32)).hex(),
                "MIR2_ALLOWED_WEB_ORIGINS": "http://10.20.0.1:17410,http://10.20.0.2:17410",
            })
            for key in CHECK.SHARED_AUTH:
                env[key] = "ONLY_SYNTHETIC_TEST_SECRET_NEVER_DEPLOY_" + key
            for key in CHECK.OFF:
                env[key] = "0"
        else:
            env.update({"MIR2_ZONE_HOST_ID": cid, "MIR2_ZONE_HOST_ADDR": address + ":17500",
                        "MIR2_ZONE_HOST_METRICS_ADDR": "127.0.0.1:17501", "MIR2_ZONE_HOST_CRYSTAL_WORLD": "1"})
        components.append({"id": cid, "hostId": host, "role": role, "env": env})
    return {"schemaVersion": 1, "profile": CHECK.PROFILE, "components": components}


class ClusterConfigTests(unittest.TestCase):
    def setUp(self):
        self.plan = example_manifest()

    def env(self, index=0):
        return self.plan["components"][index]["env"]

    def bad(self, code, previous=None):
        result = CHECK.verify(self.plan, previous)
        self.assertFalse(result["passed"])
        self.assertIn(code, [i["code"] for i in result["issues"]], result)
        self.assertFalse(result["deployed"])
        self.assertFalse(result["capacityVerified"])
        return result

    def test_valid_fixture_is_config_only(self):
        r = CHECK.verify(self.plan)
        self.assertTrue(r["passed"], r)
        self.assertEqual(r["issues"], [])
        for key in ("deployed", "capacityVerified", "runtimeFencingVerified", "highAvailabilityVerified", "migrationRequired"):
            self.assertFalse(r[key])
        self.assertEqual(r["scope"], "offline-isolated-static-2gw-2host-config-only")

    def test_native_single_host_loopback_plan(self):
        for index, c in enumerate(self.plan["components"]):
            c["hostId"] = "host-local"
            e = c["env"]
            for key in ("MIR2_ACCOUNT_STORE_DATABASE_URL", "MIR2_GATEWAY_ZONE_LEASE_DATABASE_URL", "MIR2_CHANNEL_IDENTITY_DATABASE_URL", "MIR2_GATEWAY_REDIS_CACHE_URL"):
                if key in e: e[key] = e[key].replace("10.20.0.10", "127.0.0.1")
            if c["role"] == "gateway":
                e["MIR2_ZONE_HOST_ACTIVE_ADDRS"] = "127.0.0.1:17500,127.0.0.1:17502"
                e["MIR2_GATEWAY_WEB_ADDR"] = "127.0.0.1:" + str(17410 + index * 10)
                e["MIR2_GATEWAY_TCP_ADDR"] = "127.0.0.1:" + str(17400 + index * 30)
                e["MIR2_ALLOWED_WEB_ORIGINS"] = "http://127.0.0.1:17410,http://127.0.0.1:17420"
            else:
                e["MIR2_ZONE_HOST_ADDR"] = "127.0.0.1:" + str(17500 + (index - 2) * 2)
                e["MIR2_ZONE_HOST_METRICS_ADDR"] = "127.0.0.1:" + str(17501 + (index - 2) * 2)
        self.assertTrue(CHECK.verify(self.plan)["passed"])

    def test_loader_bounds_and_duplicate_file_json(self):
        for raw in (b" " * (CHECK.MAX_BYTES + 1), b'{"secret":"one","secret":"two"}'):
            with patch.object(CHECK.Path, "open", return_value=io.BytesIO(raw)):
                with self.assertRaises(CHECK.Invalid): CHECK.load("not-read-private.json")

    def test_browser_origins_consistent(self):
        self.env(1)["MIR2_ALLOWED_WEB_ORIGINS"] = "http://10.20.0.2:17410"
        self.bad("browserOriginPolicyMismatch")

    def test_no_inherited_environment(self):
        with patch.dict("os.environ", {"MIR2_ZONE_HOST_ADDRS": "public.invalid:6666", "MIR2_GATEWAY_QA_CONTROL_TOKEN": "external-secret"}):
            self.assertTrue(CHECK.verify(self.plan)["passed"])

    def test_component_order_is_irrelevant(self):
        previous = copy.deepcopy(self.plan)
        self.plan["components"].reverse()
        self.assertTrue(CHECK.verify(self.plan, previous)["passed"])

    def test_duplicate_json_keys_at_all_levels(self):
        for raw in ('{"schemaVersion":1,"schemaVersion":1}', '{"x":{"token":"secret1","token":"secret2"}}'):
            with self.assertRaises(CHECK.Invalid): CHECK.strict_json(raw)

    def test_duplicate_embedded_topology_key(self):
        self.env()["MIR2_ZONE_TOPOLOGY_JSON"] = self.env()["MIR2_ZONE_TOPOLOGY_JSON"].replace('"version":1', '"version":1,"version":1')
        self.bad("unsupportedOrInvalidTopology")

    def test_non_finite_json_rejected(self):
        for raw in ('NaN', 'Infinity', '-Infinity'):
            with self.assertRaises(CHECK.Invalid): CHECK.strict_json(raw)

    def test_no_executable_fields(self):
        self.plan["command"] = "echo NEVER_EXECUTE"
        self.bad("unsupportedManifest")

    def test_unknown_env_secret_is_not_reported(self):
        self.env()["MIR2_GATEWAY_QA_CONTROL_TOKEN"] = "VERY_PRIVATE_SENTINEL_DO_NOT_PRINT"
        r = self.bad("unsupportedOrIncompleteEnvironment")
        self.assertNotIn("VERY_PRIVATE", json.dumps(r))
        self.assertNotIn("QA_CONTROL", json.dumps(r))

    def test_incomplete_environment_rejected(self):
        del self.env()["MIR2_ZONE_HOST_ACTIVE_ADDRS"]
        self.bad("unsupportedOrIncompleteEnvironment")

    def test_non_string_env_rejected(self):
        self.env()["MIR2_ENV"] = ["staging"]
        self.bad("invalidEnvironmentValue")

    def test_duplicate_component(self):
        self.plan["components"][1]["id"] = "gateway-1"
        self.bad("duplicateComponentId")

    def test_unique_owner_and_instance(self):
        self.env(1)["MIR2_GATEWAY_ZONE_LEASE_OWNER_ID"] = "gateway-1"
        self.bad("nonUniqueRuntimeIdentity")

    def test_host_identity_matches_fence_owner(self):
        self.env(2)["MIR2_ZONE_HOST_ID"] = "someone-else"
        self.bad("invalidZoneOwnerIdentity")

    def test_commercial_staging_required(self):
        self.env()["MIR2_ENV"] = "development"
        self.bad("unsafeAuthOrPersistenceProfile")

    def test_debug_alias_and_gate15_are_unsupported(self):
        for key in ("MIR2_ALLOW_DEV_PASSKEY_SECRET", "MIR2_ALLOW_DEV_OPERATOR_TOKEN", "MIR2_GATE15_VALIDATOR_URLS", "MIR2_ZONE_OWNER_LEASE_BACKEND", "MIR2_GATEWAY_ALLOW_UNSAFE_LOCAL_PLAYER_COMMANDS"):
            p = example_manifest(); p["components"][0]["env"][key] = "1"
            self.assertFalse(CHECK.verify(p)["passed"], key)

    def test_pg_source_required(self):
        self.env()["MIR2_ACCOUNT_STORE_BACKEND"] = "file"
        self.bad("unsafeAuthOrPersistenceProfile")

    def test_original_or_system_database_rejected(self):
        for name in ("mir2_playtest", "postgres", "mir2", "mir2_capacity", "mir2_capital"):
            with self.assertRaises(CHECK.Invalid): CHECK.pg_authority("postgres://u:p@10.20.0.10:5432/" + name)

    def test_pg_fence_mismatch(self):
        self.env()["MIR2_GATEWAY_ZONE_LEASE_DATABASE_URL"] = self.env()["MIR2_GATEWAY_ZONE_LEASE_DATABASE_URL"].replace("fixture", "other")
        self.bad("unsupportedOrMismatchedPostgresAuthority")

    def test_cross_component_pg_mismatch(self):
        for key in ("MIR2_ACCOUNT_STORE_DATABASE_URL", "MIR2_GATEWAY_ZONE_LEASE_DATABASE_URL"):
            self.env(2)[key] = self.env(2)[key].replace("fixture", "other")
        self.bad("postgresAuthorityMismatch")

    def test_postgres_and_redis_port_zero_rejected(self):
        for parser, value in ((CHECK.pg_authority, "postgres://u:p@10.20.0.10:0/mir2_capacity_fixture"),
                              (CHECK.redis_authority, "redis://10.20.0.10:0")):
            with self.assertRaises(CHECK.Invalid): parser(value)
        self.env()["MIR2_ACCOUNT_STORE_DATABASE_URL"] = self.env()["MIR2_ACCOUNT_STORE_DATABASE_URL"].replace(":5432/", ":0/")
        self.bad("unsupportedOrMismatchedPostgresAuthority")
        self.env()["MIR2_GATEWAY_REDIS_CACHE_URL"] = "redis://10.20.0.10:0"
        self.bad("unsupportedRedisAuthority")

    def test_save_recovery_key_decoded_diversity_matches_runtime(self):
        for key in ("a" * 64, bytes([0, 1, 2, 3, 4, 5, 6] * 4 + [0] * 4).hex(), "z" * 64):
            self.env()["MIR2_SAVE_RECOVERY_MAC_KEY"] = key
            self.bad("unsafeOrUnsupportedGatewayProfile")
        self.env()["MIR2_SAVE_RECOVERY_MAC_KEY"] = bytes(range(8)).hex() * 4
        self.assertTrue(CHECK.verify(self.plan)["passed"])

    def test_pg_multihost_query_and_dns_unsupported(self):
        for value in ("postgres://u:p@db.internal:5432/mir2_cap_qa", "postgres://u:p@10.0.0.1:5432/mir2_cap_qa?sslmode=require", "postgres://u:p@10.0.0.1,10.0.0.2:5432/mir2_cap_qa"):
            with self.assertRaises((CHECK.Invalid, ValueError)): CHECK.pg_authority(value)

    def test_namespace_not_invented_or_db_selector_ignored(self):
        for suffix in ("/0", "/1", "?db=1"):
            self.env()["MIR2_GATEWAY_REDIS_CACHE_URL"] = "redis://10.20.0.10:16481" + suffix
            self.bad("unsupportedRedisAuthority")
        self.env()["MIR2_GATEWAY_REDIS_NAMESPACE"] = "invented"
        self.bad("unsupportedOrIncompleteEnvironment")

    def test_redis_auth_not_supported_by_actual_client(self):
        self.env()["MIR2_GATEWAY_REDIS_CACHE_URL"] = "redis://user:password@10.20.0.10:16481"
        self.bad("unsupportedRedisAuthority")

    def test_redis_actual_parser_case_is_not_guessed(self):
        for value in ("REDIS://10.20.0.10:16481", "redis://10.20.0.10:016481", " redis://10.20.0.10:16481"):
            with self.assertRaises(CHECK.Invalid): CHECK.redis_authority(value)

    def test_env_control_characters_fail_closed(self):
        for ch in ("\t", "\x00", "\r", "\n", "\x7f"):
            p = example_manifest(); p["components"][0]["env"]["MIR2_ACCOUNT_STORE_DATABASE_URL"] += ch
            self.assertFalse(CHECK.verify(p)["passed"])

    def test_redis_mismatch(self):
        self.env(1)["MIR2_GATEWAY_REDIS_CACHE_URL"] = "redis://10.20.0.10:16482"
        self.bad("redisAuthorityMismatch")

    def test_weak_or_inconsistent_tokens(self):
        self.env()["MIR2_ZONE_HOST_TOKEN"] = "short"
        self.bad("weakRpcToken")
        self.env()["MIR2_ZONE_HOST_TOKEN"] = "SOME_OTHER_TEST_TOKEN_" * 3
        self.bad("rpcTokenMismatch")

    def test_gateway_identity_secrets_shared(self):
        self.env(1)["MIR2_IDENTITY_SESSION_SECRET"] = "different-test-session-secret-" * 3
        self.bad("gatewayAuthSecretMismatch")

    def test_paid_features_not_enabled(self):
        for key in CHECK.OFF:
            p = example_manifest(); p["components"][0]["env"][key] = "1"
            self.assertFalse(CHECK.verify(p)["passed"], key)

    def test_content_profiles_are_explicit(self):
        self.env()["MIR2_CONTENT_PROFILE"] = "platinum_176"
        self.bad("unsafeOrUnsupportedGatewayProfile")
        self.env(2)["MIR2_ZONE_HOST_CRYSTAL_WORLD"] = "0"
        self.bad("unsupportedHostContentProfile")

    def test_ordered_active_lists_must_match(self):
        self.env(1)["MIR2_ZONE_HOST_ACTIVE_ADDRS"] = "10.20.0.2:17500,10.20.0.1:17500"
        self.bad("orderedActiveHostsMismatch")

    def test_duplicate_or_unmapped_endpoint(self):
        self.env()["MIR2_ZONE_HOST_ACTIVE_ADDRS"] = "10.20.0.1:17500,10.20.0.1:17500"
        self.bad("unsupportedActiveHosts")
        self.env()["MIR2_ZONE_HOST_ACTIVE_ADDRS"] = "10.20.0.1:17500,10.20.0.3:17500"
        self.bad("activeEndpointsDoNotMatchHosts")

    def test_physical_host_port_conflict(self):
        self.env()["MIR2_GATEWAY_TCP_ADDR"] = "10.20.0.1:17410"
        self.bad("physicalHostListenPortConflict")

    def test_per_host_paths_overlap_rejected(self):
        self.env()["MIR2_SAVE_RECOVERY_DIR"] = "/var/lib/capacity/gateway-1/state/subdir"
        self.bad("physicalHostStateRecoveryConflict")

    def test_different_hosts_can_reuse_ports_and_paths(self):
        self.env(1)["MIR2_ACCOUNT_STORE_PATH"] = self.env()["MIR2_ACCOUNT_STORE_PATH"]
        self.env(1)["MIR2_SAVE_RECOVERY_DIR"] = self.env()["MIR2_SAVE_RECOVERY_DIR"]
        self.assertTrue(CHECK.verify(self.plan)["passed"])

    def test_private_literal_addresses_only(self):
        for value in ("0.0.0.0:17410", "8.8.8.8:17410", "localhost:17410", "[::1]:17410"):
            self.env()["MIR2_GATEWAY_WEB_ADDR"] = value
            self.bad("unsupportedListenAddress")

    def test_nonloopback_operator_needs_signature_not_guessed(self):
        self.env(2)["MIR2_ZONE_HOST_METRICS_ADDR"] = "10.20.0.1:17501"
        self.bad("unsupportedListenAddress")

    def test_cross_physical_host_loopback_authority_rejected(self):
        for c in self.plan["components"]:
            for key in ("MIR2_ACCOUNT_STORE_DATABASE_URL", "MIR2_GATEWAY_ZONE_LEASE_DATABASE_URL", "MIR2_CHANNEL_IDENTITY_DATABASE_URL"):
                if key in c["env"]: c["env"][key] = c["env"][key].replace("10.20.0.10", "127.0.0.1")
        self.bad("crossHostLoopbackAuthority")

    def test_bad_topology_and_local_hotmaps_unsupported(self):
        for patch_value in ({"mode": "single"}, {"hotMaps": {"0": {}}}, {"defaultTickMs": 1}):
            t = json.loads(self.env()["MIR2_ZONE_TOPOLOGY_JSON"]); t.update(patch_value)
            self.env()["MIR2_ZONE_TOPOLOGY_JSON"] = json.dumps(t)
            self.bad("unsupportedOrInvalidTopology")

    def test_duplicate_map_and_topology_mismatch(self):
        t = json.loads(self.env()["MIR2_ZONE_TOPOLOGY_JSON"])
        t["zones"]["zone-d"]["maps"] = ["0"]
        self.env()["MIR2_ZONE_TOPOLOGY_JSON"] = json.dumps(t)
        self.bad("unsupportedOrInvalidTopology")
        t["zones"]["zone-d"]["maps"] = ["2"]
        self.env()["MIR2_ZONE_TOPOLOGY_JSON"] = json.dumps(t)
        self.bad("topologyMismatch")

    def test_source_shard_algorithm_requires_both_hosts_used(self):
        t = json.loads(self.env()["MIR2_ZONE_TOPOLOGY_JSON"])
        t["zones"]["zone-b"] = t["zones"].pop("zone-d")
        self.env()["MIR2_ZONE_TOPOLOGY_JSON"] = json.dumps(t)
        self.bad("unsupportedOrInvalidTopology")

    def test_new_host_length_change_with_four_components_is_flagged(self):
        previous = copy.deepcopy(self.plan)
        for i in (0, 1): self.env(i)["MIR2_ZONE_HOST_ACTIVE_ADDRS"] += ",10.20.0.3:17500"
        self.assertTrue(self.bad("unsupportedActiveHosts", previous)["migrationRequired"])

    def test_host_id_move_cannot_keep_another_hosts_ip(self):
        previous = copy.deepcopy(self.plan)
        self.plan["components"][0]["hostId"] = "host-3"
        result = self.bad("nonLoopbackAddressClaimedByMultipleHosts", previous)
        self.assertTrue(result["migrationRequired"])
        self.assertIn("migrationRequired", [item["code"] for item in result["issues"]])

    def test_one_physical_host_can_declare_multiple_private_ips(self):
        self.env()["MIR2_GATEWAY_WEB_ADDR"] = "10.20.1.1:17410"
        self.env()["MIR2_GATEWAY_TCP_ADDR"] = "10.20.1.1:17400"
        self.assertTrue(CHECK.verify(self.plan)["passed"])

    def test_valid_gateway_relocation_still_requires_migration(self):
        previous = copy.deepcopy(self.plan)
        self.plan["components"][0]["hostId"] = "host-3"
        self.env()["MIR2_GATEWAY_WEB_ADDR"] = "10.20.0.3:17410"
        self.env()["MIR2_GATEWAY_TCP_ADDR"] = "10.20.0.3:17400"
        self.assertTrue(CHECK.verify(self.plan)["passed"])
        self.assertTrue(self.bad("migrationRequired", previous)["migrationRequired"])

    def test_listen_and_state_recovery_location_changes_require_migration(self):
        for key, value in (("MIR2_GATEWAY_WEB_ADDR", "10.20.0.1:17411"),
                           ("MIR2_ACCOUNT_STORE_PATH", "/var/lib/capacity/gateway-1/new-state/accounts.json"),
                           ("MIR2_SAVE_RECOVERY_DIR", "/var/lib/capacity/gateway-1/new-recovery")):
            previous = example_manifest()
            self.plan = copy.deepcopy(previous)
            self.env()[key] = value
            self.assertTrue(CHECK.verify(self.plan)["passed"], key)
            self.assertTrue(self.bad("migrationRequired", previous)["migrationRequired"], key)

    def test_previous_active_reorder_requires_migration(self):
        previous = copy.deepcopy(self.plan)
        for i in (0, 1): self.env(i)["MIR2_ZONE_HOST_ACTIVE_ADDRS"] = "10.20.0.2:17500,10.20.0.1:17500"
        self.assertTrue(self.bad("migrationRequired", previous)["migrationRequired"])

    def test_previous_address_replacement_requires_migration(self):
        previous = copy.deepcopy(self.plan)
        for i in (0, 1): self.env(i)["MIR2_ZONE_HOST_ACTIVE_ADDRS"] = "10.20.0.1:17500,10.20.0.3:17500"
        self.env(3)["MIR2_ZONE_HOST_ADDR"] = "10.20.0.3:17500"
        self.assertTrue(self.bad("migrationRequired", previous)["migrationRequired"])

    def test_three_host_extension_still_reports_migration(self):
        previous = copy.deepcopy(self.plan)
        for i in (0, 1): self.env(i)["MIR2_ZONE_HOST_ACTIVE_ADDRS"] += ",10.20.0.3:17500"
        extra = copy.deepcopy(self.plan["components"][3]); extra["id"] = "zone-3"
        self.plan["components"].append(extra)
        r = self.bad("requiresTwoGatewaysTwoHosts", previous)
        self.assertTrue(r["migrationRequired"])
        self.assertIn("migrationRequired", [i["code"] for i in r["issues"]])

    def test_reformatting_topology_does_not_require_migration(self):
        previous = copy.deepcopy(self.plan)
        for c in self.plan["components"]:
            c["env"]["MIR2_ZONE_TOPOLOGY_JSON"] = json.dumps(json.loads(c["env"]["MIR2_ZONE_TOPOLOGY_JSON"]), indent=2).replace("\n", " ")
        self.assertTrue(CHECK.verify(self.plan, previous)["passed"])

    def test_invalid_previous_fails_closed(self):
        previous = copy.deepcopy(self.plan); previous["profile"] = "unknown"
        self.bad("invalidPreviousConfiguration", previous)

    def test_cli_prints_only_redacted_codes(self):
        sentinel = "SENTINEL_PRIVATE_PASSWORD_0123456789"
        self.env()["MIR2_ACCOUNT_STORE_DATABASE_URL"] = sentinel
        output = io.StringIO()
        with patch.object(CHECK, "load", return_value=self.plan), contextlib.redirect_stdout(output):
            status = CHECK.main(["not-read-private.json"])
        self.assertEqual(status, 1); self.assertNotIn(sentinel, output.getvalue())
        self.assertNotIn("not-read-private", output.getvalue())
        self.assertFalse(json.loads(output.getvalue())["passed"])

    def test_cli_duplicate_json_exception_has_no_payload(self):
        output = io.StringIO()
        with patch.object(CHECK, "load", side_effect=CHECK.Invalid("SECRET_MUST_NOT_ESCAPE")), contextlib.redirect_stdout(output):
            self.assertEqual(CHECK.main(["private.json"]), 1)
        self.assertNotIn("SECRET", output.getvalue())
        self.assertEqual(json.loads(output.getvalue())["issues"][0]["code"], "invalidPrivateManifest")


if __name__ == "__main__":
    unittest.main()
