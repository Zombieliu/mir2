"""Execute isolated PostgreSQL view/alert/permission acceptance via psql.

Requires migrations 0016/0017 already applied. Authentication is inherited only
from PGPASSWORD in the process environment; no private credential file is read.
Every synthetic realm has a unique hunting-observer-test- prefix. All fixture
rows, role grants and test roles are created in ONE transaction and ROLLED BACK.
--render-only writes inspectable SQL without connecting or requiring credentials.
"""
from __future__ import annotations

import argparse
import ipaddress
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import uuid

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from verify import seed  # independent fixture defaults, never SQL generation
from datetime import date


def literal(value):
    if value is None:
        return "NULL"
    if value is True:
        return "true"
    if value is False:
        return "false"
    if isinstance(value, (int, float)):
        return str(value)
    return "'" + str(value).replace("'", "''") + "'"


def jsql(value):
    return literal(json.dumps(value, ensure_ascii=False, separators=(",", ":"))) + "::jsonb"


def build_script(prefix: str, role: str) -> tuple[str, dict]:
    vectors = json.loads((ROOT / "fixtures/alert-cases.json").read_text(encoding="utf-8"))
    statements = ["\\set ON_ERROR_STOP on", "BEGIN;", "SET LOCAL statement_timeout = '30s';",
                  "CREATE TEMP TABLE hunting_observer_assertions (name text PRIMARY KEY);",
                  "CREATE FUNCTION pg_temp.hunting_observer_assert(ok boolean, label text) RETURNS void LANGUAGE plpgsql AS $$ BEGIN IF ok IS NOT TRUE THEN RAISE EXCEPTION 'observer assertion failed: %', label; END IF; INSERT INTO hunting_observer_assertions(name) VALUES(label); END $$;",
                  f"SELECT set_config('hunting_observer_test.prefix', {literal(prefix)}, true);"]
    expected = []
    # Duplicate source days and zero baselines are impossible under the physical
    # keys/checks, so test their rejection separately rather than corrupt history.
    constraint_cases = {"duplicate_daily_row", "zero_stock_baseline_not_normal", "permit_zero_plan_undefined", "conversion_conservation_error"}
    fixture_cases = 0
    for case in vectors["cases"]:
        if case["name"] in constraint_cases:
            continue
        fixture_cases += 1
        realm = prefix + "-" + case["name"]
        policy_version = "synthetic-observer-v1"
        pjson = {"schema_version": 1, "policy_version": policy_version, "enabled": True,
                 "k": {"numerator": 1, "denominator": 10}, "conversion_fee": {"numerator": 1, "denominator": 50},
                 "daily_decay": {"numerator": 1, "denominator": 150}, "baseline_daily_mint_atoms": 72000000,
                 "daily_mint_cap_multiplier": {"numerator": 3, "denominator": 2}, "business_timezone": "Asia/Shanghai", "settlement_hour": 4}
        statements += [
            f"INSERT INTO hunting_economy_policies(realm_id,policy_version,policy_digest,policy_json,created_at_ms) VALUES({literal(realm)},{literal(policy_version)},repeat('0',64),{jsql(pjson)},(extract(epoch FROM now())*1000)::bigint);",
            f"INSERT INTO hunting_economy_pools(realm_id,policy_version,genesis_day,settled_through_day,updated_at_ms) SELECT {literal(realm)},{literal(policy_version)},(now() AT TIME ZONE 'Asia/Shanghai' - interval '4 hours')::date - date '1970-01-01' -100,(now() AT TIME ZONE 'Asia/Shanghai' - interval '4 hours')::date - date '1970-01-01' -1,(extract(epoch FROM now())*1000)::bigint;",
        ]
        drop = set(case.get("drop_offsets", []))
        shifted = case.get("shift_history_days", 0)
        for offset in range(10):
            if offset in drop:
                continue
            # Python's seed date is a placeholder only. SQL computes all real
            # fixture dates from the same transaction clock as the actual query.
            row = seed(date(2000, 1, 1))
            setting = case.get("set_last", {"days": 0, "fields": {}})
            if offset < setting["days"]:
                row.update(setting["fields"])
            for override in case.get("overrides", []):
                if override["offset"] == offset:
                    row.update(override["fields"])
            day_expr = f"(now() AT TIME ZONE 'Asia/Shanghai' - interval '4 hours')::date - date '1970-01-01' - 1 - {offset + shifted}"
            end_expr = f"((date '1970-01-01' + ({day_expr}) + 1) + time '{row['period_end_hour']:02d}:00') AT TIME ZONE 'Asia/Shanghai'"
            revision = offset + 1
            statements.append(
                "INSERT INTO hunting_economy_observation_settings(realm_id,settings_revision,policy_version,effective_from_day,unbound_stock_baseline_gold,burn_baseline_gold,planned_daily_consumption_atoms,full_realm_coverage_qualified,coverage_qualification_evidence,coverage_qualified_by,coverage_qualified_at_ms,created_at_ms) "
                f"SELECT {literal(realm)},{revision},{literal(policy_version)},{day_expr},{literal(row['unbound_stock_baseline_gold'])},{literal(row['burn_baseline_gold'])},{literal(row['planned_daily_consumption_atoms'])},true,'synthetic full-realm coverage, not real data','acceptance fixture',0,0;"
            )
            # This receipt has the exact Rust SettlementReceipt/SettlementPlan
            # field names; domain values are intentionally synthetic test input.
            receipt = {"realm_id": realm, "business_day": None, "policy_version": policy_version, "committed_at_ms": None,
                       "burns": {"daily": row["burn_daily_gold"], "light_pvp": row["burn_light_pvp_gold"], "siege": row["burn_sabuk_gold"]},
                       "plan": {"destruction_gold_equivalent": sum(row[k] for k in ("burn_daily_gold", "burn_light_pvp_gold", "burn_sabuk_gold")),
                                "opening_stock_atoms": 1000, "raw_mint_atoms": row["permit_minted_atoms"], "daily_mint_cap_atoms": 108000000,
                                "minted_atoms": row["permit_minted_atoms"], "capped_atoms": 0, "pre_decay_stock_atoms": row["permit_closing_atoms"] + row["permit_decayed_atoms"],
                                "decayed_atoms": row["permit_decayed_atoms"], "next_pool": {"stock_atoms": row["permit_closing_atoms"], "mint_remainder": 0, "decay_remainder": 0}},
                       "consumed_atoms": row["permit_consumed_atoms"], "gross_conversion_gold": row["gross_converted_gold"], "net_unbound_gold_inflow": row["net_unbound_inflow_gold"],
                       "bound_gold_stock": "123", "unbound_gold_stock": "456"}
            statements.append(
                "INSERT INTO hunting_economy_settlements(realm_id,business_day,policy_version,receipt_json,committed_at_ms) "
                f"SELECT {literal(realm)},{day_expr},{literal(policy_version)},{jsql(receipt)} || jsonb_build_object('business_day',{day_expr},'committed_at_ms',(extract(epoch FROM now())*1000)::bigint),(extract(epoch FROM now())*1000)::bigint;"
            )
            counts = [row[k] for k in ("active_casual_accounts", "active_light_pvp_accounts", "active_sabuk_accounts")]
            portrait_ok = all(n is not None for n in counts) and sum(counts) == row["active_account_total"]
            complete = row["is_complete"] is True
            columns = ["realm_id", "business_day", "metric_revision", "settings_revision", "metric_source", "created_at_ms",
                       "financial_period_closed", "source_period_end", "source_ledger_watermark", "coverage_complete", "coverage_scope", "coverage_evidence",
                       "bound_gold_stock", "unbound_gold_stock", "portrait_data_complete", "active_casual_accounts", "active_light_pvp_accounts", "active_sabuk_accounts", "active_account_total", "portrait_policy_version"]
            values = [literal(realm), day_expr, "1", str(revision), "'synthetic-acceptance'", "0", literal(complete), end_expr,
                      "'synthetic-closed-watermark'", literal(complete), "'all_realm_assets'", "'synthetic-full-coverage'",
                      literal(row["bound_gold_stock"]), literal(row["unbound_gold_stock"]), literal(portrait_ok),
                      *[literal(n) for n in counts], literal(row["active_account_total"]), "'synthetic-portrait-v1'"]
            statements.append(f"INSERT INTO hunting_economy_daily_metric_versions({','.join(columns)}) SELECT {','.join(values)};")
        expected.extend({"realm_id": realm, "rule": key, "value": value} for key, value in case["expected"].items())

    statements.append(f"CREATE TEMP TABLE hunting_observer_expected AS SELECT * FROM jsonb_to_recordset({jsql(expected)}) AS e(realm_id text, rule text, value double precision);")
    for key in ("inflation", "deflation", "permit_exhaustion", "burn_pulse", "data_integrity"):
        sql = (ROOT / f"sql/alerts/{key}.sql").read_text(encoding="utf-8").strip().rstrip(";")
        statements.append(f"CREATE TEMP TABLE hunting_observer_actual_{key} AS {sql};")
        statements.append(
            "SELECT pg_temp.hunting_observer_assert("
            f"NOT EXISTS(SELECT 1 FROM hunting_observer_expected e LEFT JOIN hunting_observer_actual_{key} a USING(realm_id) "
            f"WHERE e.rule={literal(key)} AND (a.realm_id IS NULL OR a.value IS DISTINCT FROM e.value)),{literal('actual-SQL-' + key)});"
        )
    # Apply the generic alert SQL to actual physical views; no fake tables shadow
    # public views. Existing unrelated realms may appear in results, but assertions
    # inspect only unique synthetic realm IDs created by this transaction.
    statements.append((ROOT / "tests/postgres_extra_assertions.sql").read_text(encoding="utf-8"))
    statements += [
        f'CREATE ROLE "{role}" NOLOGIN;',
        f'GRANT USAGE ON SCHEMA public TO "{role}";',
        f'GRANT SELECT ON hunting_permit_observability_realms,hunting_permit_daily_observations TO "{role}";',
        f'SET LOCAL ROLE "{role}";',
        "SELECT count(*) FROM hunting_permit_observability_realms;",
        "SELECT count(*) FROM hunting_permit_daily_observations;",
        (ROOT / "tests/postgres_readonly_assertions.sql").read_text(encoding="utf-8"),
        "RESET ROLE;",
        "SELECT pg_temp.hunting_observer_assert(true,'readonly-role-select-and-DML-denial');",
        "SELECT json_build_object('assertions',count(*),'synthetic_realms',(SELECT count(*) FROM hunting_permit_observability_realms WHERE realm_id LIKE current_setting('hunting_observer_test.prefix') || '%'),'rolled_back',true) AS acceptance_summary FROM hunting_observer_assertions;",
        "ROLLBACK;",
    ]
    return "\n".join(statements) + "\n", {"synthetic_fixture_cases": fixture_cases, "expected_rule_values": len(expected)}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--render-only", type=Path, help="write SQL only; no database or credentials required")
    args = parser.parse_args()
    suffix = uuid.uuid4().hex[:16]
    prefix, role = "hunting-observer-test-" + suffix, "hunting_observer_test_" + suffix
    script, stats = build_script(prefix, role)
    if args.render_only:
        args.render_only.parent.mkdir(parents=True, exist_ok=True)
        args.render_only.write_text(script, encoding="utf-8")
        print(json.dumps({"status": "rendered-only", "postgres_executed": False, **stats}))
        return

    env = os.environ.copy()
    host = env.get("PGHOST", "127.0.0.1")
    try:
        loopback = ipaddress.ip_address(host).is_loopback
    except ValueError:
        loopback = False
    database = env.get("PGDATABASE", "mir2_hunting_acceptance_20261019")
    if not loopback or not database.startswith("mir2_hunting_acceptance_"):
        raise SystemExit("Refusing acceptance outside an explicit loopback hunting acceptance database.")
    if not env.get("PGPASSWORD"):
        raise SystemExit("Set PGPASSWORD in the invoking process; this script never reads a credential file.")
    env.update({"PGHOST": host, "PGPORT": env.get("PGPORT", "55439"), "PGDATABASE": database, "PGUSER": env.get("PGUSER", "hunting_test")})
    psql = env.get("MIR2_PSQL_BIN") or shutil.which("psql")
    if not psql:
        raise SystemExit("psql is unavailable; set MIR2_PSQL_BIN to the local psql executable.")
    with tempfile.TemporaryDirectory(prefix="hunting-observer-test-") as tempdir:
        sql_path = Path(tempdir) / "acceptance.sql"
        sql_path.write_text(script, encoding="utf-8")
        result = subprocess.run([psql, "-X", "-q", "-v", "ON_ERROR_STOP=1", "-f", str(sql_path)], env=env, capture_output=True, text=True, timeout=180)
    secret = env["PGPASSWORD"]
    output = (result.stdout + result.stderr).replace(secret, "[redacted]")
    report = {"status": "passed" if result.returncode == 0 else "failed", "postgres_executed": True,
              "exit_code": result.returncode, "synthetic_realm_prefix": prefix, "all_fixture_changes_rolled_back": result.returncode == 0,
              "grafana_import_verified": False, "notifications_sent": False, **stats, "psql_output": output}
    (ROOT / "postgres-validation-result.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({k: v for k, v in report.items() if k != "psql_output"}, ensure_ascii=False))
    if result.returncode:
        print(output, file=sys.stderr)
        raise SystemExit(result.returncode)


if __name__ == "__main__":
    main()
