"""Offline artifact and independent rule-model checks; does not execute SQL.

The rule model is separate from SQL generation. A pass is deliberately not a
claim about PostgreSQL execution, Grafana import/render or notification delivery.
"""
from __future__ import annotations

import hashlib
import json
import re
from datetime import date, datetime, time, timedelta, timezone
from decimal import Decimal
from pathlib import Path

ROOT = Path(__file__).resolve().parent
SHANGHAI = timezone(timedelta(hours=8))
MUTATIONS = re.compile(r"\b(insert|update|delete|merge|truncate|alter|create|drop|grant|revoke|call|copy|do|set|reset|vacuum|refresh|pg_sleep|dblink|lo_import|nextval|setval)\b", re.I)
CORE = ["burn_daily_gold", "burn_light_pvp_gold", "burn_sabuk_gold", "permit_minted_atoms", "permit_consumed_atoms", "permit_decayed_atoms", "permit_closing_atoms", "bound_gold_stock", "unbound_gold_stock", "gross_converted_gold", "conversion_fee_gold", "net_unbound_inflow_gold"]
CHECKS = 0


def load(relative: str):
    return json.loads((ROOT / relative).read_text(encoding="utf-8"))


def check(condition: bool, detail: str):
    global CHECKS
    CHECKS += 1
    if not condition:
        raise AssertionError(detail)


def closed_date(now: datetime) -> date:
    return (now.astimezone(SHANGHAI) - timedelta(hours=4)).date() - timedelta(days=1)


def seed(day: date) -> dict:
    return {"economy_date": day, "period_end_hour": 4, "is_complete": True, "policy_version": "synthetic-v1",
            "burn_daily_gold": 50, "burn_light_pvp_gold": 30, "burn_sabuk_gold": 20,
            "permit_minted_atoms": 1000, "permit_consumed_atoms": 1000, "permit_decayed_atoms": 1, "permit_closing_atoms": 1000,
            "bound_gold_stock": 2000, "unbound_gold_stock": 1000, "gross_converted_gold": 100, "conversion_fee_gold": 2, "net_unbound_inflow_gold": 98,
            "unbound_stock_baseline_gold": 1000, "burn_baseline_gold": 100, "planned_daily_consumption_atoms": 1000,
            "active_casual_accounts": 6, "active_light_pvp_accounts": 3, "active_sabuk_accounts": 1, "active_account_total": 10,
            "monthly_card_gold_per_card": None, "raw_material_price_index": None, "heavy_producer_monthly_usd_equivalent": None}


def positive(value) -> bool:
    return value is not None and value > 0


def nonnegative(value) -> bool:
    return value is not None and value >= 0


def evaluate(rows: list[dict], end: date, days: int, required, predicate):
    expected_days = {end - timedelta(days=offset) for offset in range(days)}
    selected = [row for row in rows if row["economy_date"] in expected_days]
    if len(selected) != days or {r["economy_date"] for r in selected} != expected_days:
        return None
    if not all(row["is_complete"] is True and required(row) for row in selected):
        return None
    return int(all(predicate(row) for row in selected))


def model(rows: list[dict], end: date) -> dict:
    stock_required = lambda r: nonnegative(r["unbound_gold_stock"]) and positive(r["unbound_stock_baseline_gold"])
    result = {
        "inflation": evaluate(rows, end, 7, stock_required, lambda r: Decimal(r["unbound_gold_stock"]) > Decimal(r["unbound_stock_baseline_gold"]) * Decimal("1.30")),
        "deflation": evaluate(rows, end, 7, stock_required, lambda r: Decimal(r["unbound_gold_stock"]) < Decimal(r["unbound_stock_baseline_gold"]) * Decimal("0.70")),
        "permit_exhaustion": evaluate(rows, end, 10, lambda r: nonnegative(r["permit_closing_atoms"]) and positive(r["planned_daily_consumption_atoms"]), lambda r: r["permit_closing_atoms"] < r["planned_daily_consumption_atoms"]),
        "burn_pulse": evaluate(rows, end, 1, lambda r: all(nonnegative(r[f]) for f in CORE[:3]) and positive(r["burn_baseline_gold"]), lambda r: sum(Decimal(r[f]) for f in CORE[:3]) > Decimal(r["burn_baseline_gold"]) * Decimal("1.50")),
    }

    def healthy(r):
        counts = [r[f] for f in ("active_casual_accounts", "active_light_pvp_accounts", "active_sabuk_accounts")]
        return (all(nonnegative(r[f]) for f in CORE)
                and all(positive(r[f]) for f in ("unbound_stock_baseline_gold", "burn_baseline_gold", "planned_daily_consumption_atoms"))
                and bool(r["policy_version"]) and r["period_end_hour"] == 4
                and r["gross_converted_gold"] == r["conversion_fee_gold"] + r["net_unbound_inflow_gold"]
                and all(nonnegative(c) for c in counts) and r["active_account_total"] == sum(counts))

    result["data_integrity"] = 0 if evaluate(rows, end, 10, healthy, lambda r: True) == 1 else 1
    return result


def run_fixtures() -> tuple[int, int]:
    fixtures = load("fixtures/alert-cases.json")
    check(fixtures["classification"].startswith("synthetic"), "fixtures must be marked synthetic")
    assertions = 0
    for case in fixtures["cases"]:
        now = datetime.fromisoformat(case.get("now", fixtures["default_now"]))
        end = closed_date(now)
        if "expected_closed_date" in case:
            check(end.isoformat() == case["expected_closed_date"], f"{case['name']}: wrong 04:00 day")
            assertions += 1
        shifted = case.get("shift_history_days", 0)
        indexed = {i: seed(end - timedelta(days=i + shifted)) for i in range(10)}
        setting = case.get("set_last", {"days": 0, "fields": {}})
        for i in range(setting["days"]):
            indexed[i].update(setting["fields"])
        for override in case.get("overrides", []):
            indexed[override["offset"]].update(override["fields"])
        rows = [row for i, row in indexed.items() if i not in case.get("drop_offsets", [])]
        if "duplicate_offset" in case:
            rows.append(indexed[case["duplicate_offset"]].copy())
        actual = model(rows, end)
        for key, expected in case["expected"].items():
            check(actual[key] == expected, f"{case['name']}: {key} expected {expected!r}, got {actual[key]!r}")
            assertions += 1
    return len(fixtures["cases"]), assertions


def check_readonly(sql: str, name: str):
    # This is a conservative artifact check, not a SQL parser or replacement for
    # PostgreSQL privileges. Strip strings/comments before examining tokens.
    stripped = re.sub(r"--[^\n]*|/\*.*?\*/|'(?:''|[^'])*'", " ", sql, flags=re.S)
    match = MUTATIONS.search(stripped)
    check(match is None, f"{name}: disallowed SQL token {match.group(0) if match else ''}")
    check(re.match(r"\s*(SELECT|WITH)\b", stripped, re.I) is not None, f"{name}: not a read query")
    check("public.hunting_permit_" in sql, f"{name}: unexpected source")
    check(not re.search(r"\bCOALESCE\s*\(", sql, re.I), f"{name}: must not zero-fill missing observations")


def check_artifacts() -> int:
    start_checks = CHECKS
    count = 0
    datasource = load("grafana/provisioning/datasources/economy.yaml")["datasources"][0]
    check(datasource["uid"] == "mir2-economy-readonly", "wrong datasource UID")
    check(datasource["editable"] is False and datasource["isDefault"] is False, "datasource mutability/default")
    check(datasource["secureJsonData"]["password"] == "$MIR2_ECONOMY_DB_PASSWORD", "password must be an environment reference")
    check(datasource["jsonData"]["sslmode"] == "verify-full", "database TLS verification is required")
    check("password" not in datasource, "password cannot be stored as a plaintext datasource field")
    count += 5
    dashboard = load("grafana/dashboards/hunting-permit.json")
    check(dashboard["timezone"] == "Asia/Shanghai" and dashboard["editable"] is False, "dashboard timezone/read-only")
    check(len(dashboard["panels"]) == 14, "required dashboard panels")
    check(len({p["id"] for p in dashboard["panels"]}) == len(dashboard["panels"]), "duplicate panel ID")
    count += 3
    variable = dashboard["templating"]["list"][0]
    check_readonly(variable["query"], "realm-variable")
    count += 1
    for p in dashboard["panels"]:
        for t in p.get("targets", []):
            check_readonly(t["rawSql"], p["title"])
            check("${realm:sqlstring}" in t["rawSql"], f"{p['title']}: selected realm must apply to every data panel")
            count += 1
        if p["type"] in {"timeseries", "piechart"}:
            sql = p["targets"][0]["rawSql"]
            check("generate_series(" in sql and "LEFT JOIN" in sql, f"{p['title']}: absent-day gap rows")
            check("${realm:sqlstring}" in sql, f"{p['title']}: escaped realm selector")
            check(p["fieldConfig"]["defaults"]["custom"]["spanNulls"] is False, f"{p['title']}: must show gaps")
            count += 3
    card = next(p for p in dashboard["panels"] if p["id"] == 5)
    check(card["timeFrom"] == "90d", "monthly card panel must show 90 days")
    for p in dashboard["panels"]:
        if p["id"] in {3, 4}:
            check("/ 1000000.0" in p["targets"][0]["rawSql"], "permit atom display scale")
    count += 3

    group = load("grafana/provisioning/alerting/economy-rules.yaml")["groups"][0]
    check(len(group["rules"]) == 5, "four business rules plus data integrity rule required")
    count += 1
    for rule in group["rules"]:
        key = rule["labels"]["economy_rule"]
        sql = (ROOT / f"sql/alerts/{key}.sql").read_text(encoding="utf-8").strip()
        check(rule["data"][0]["model"]["rawSql"].strip() == sql, f"{key}: SQL copy drift")
        check_readonly(sql, key)
        check("now() AT TIME ZONE 'Asia/Shanghai' - interval '4 hours'" in sql, f"{key}: wrong economic-day clock")
        check("generate_series(" in sql and "LEFT JOIN" in sql, f"{key}: absent-day skeleton")
        check("max(" not in sql.lower(), f"{key}: cannot anchor to latest existing observation")
        check("bool_and(o.is_complete IS TRUE)" in sql, f"{key}: incomplete day accepted")
        check(rule["for"] == "10m" and rule["execErrState"] == "Error", f"{key}: missing pending/error behavior")
        check(rule["noDataState"] == ("Alerting" if key == "data_integrity" else "NoData"), f"{key}: missing-data handling")
        check(rule["labels"]["severity"] in {"red", "yellow"}, f"{key}: severity")
        check(rule["data"][2]["model"]["conditions"][0]["evaluator"] == {"type": "gt", "params": [0.5]}, f"{key}: incorrect boolean threshold")
        if key == "data_integrity":
            check("THEN 0.0 ELSE 1.0 END AS value" in sql and "ELSE NULL" not in sql, "health query must explicitly flag gaps")
        else:
            check("ELSE NULL::double precision END AS value" in sql, f"{key}: missing data is not normal")
        count += 11
    rule_sql = {r["labels"]["economy_rule"]: r["data"][0]["model"]["rawSql"] for r in group["rules"]}
    for key, days in (("inflation", 7), ("deflation", 7), ("permit_exhaustion", 10), ("burn_pulse", 1), ("data_integrity", 10)):
        check(f"count(o.economy_date) = {days}" in rule_sql[key], f"{key}: wrong required day count")
        check(f"b.closed_date - ({days} - 1)" in rule_sql[key], f"{key}: wrong calendar width")
        count += 2
    check("o.unbound_gold_stock::numeric > o.unbound_stock_baseline_gold::numeric * 1.30" in rule_sql["inflation"], "inflation strict threshold")
    check("o.unbound_gold_stock::numeric < o.unbound_stock_baseline_gold::numeric * 0.70" in rule_sql["deflation"], "deflation strict threshold")
    check("o.permit_closing_atoms < o.planned_daily_consumption_atoms" in rule_sql["permit_exhaustion"], "permit strict coverage threshold")
    check(") > o.burn_baseline_gold::numeric * 1.50" in rule_sql["burn_pulse"], "burn strict threshold")
    count += 4
    contacts = load("grafana/provisioning/alerting/economy-contact-points.yaml")["contactPoints"][0]
    settings = contacts["receivers"][0]["settings"]
    check(settings["url"] == "$MIR2_ECONOMY_ALERT_RELAY_URL", "webhook URL must be runtime configured")
    check(settings["authorization_credentials"] == "$MIR2_ECONOMY_ALERT_RELAY_TOKEN", "webhook secret must be runtime configured")
    check(contacts["receivers"][0]["disableResolveMessage"] is False, "recovery notification required")
    check(not list((ROOT / "grafana/provisioning").rglob("*polic*")), "must not automatically replace notification policy")
    count += 4
    # The entire generated provisioning tree must parse as JSON-compatible YAML.
    for path in (ROOT / "grafana/provisioning").rglob("*.yaml"):
        json.loads(path.read_text(encoding="utf-8"))
        count += 1
    migration = ROOT.parent / "postgres/migrations/0017_hunting_economy_observability.sql"
    if migration.exists():
        schema = migration.read_text(encoding="utf-8")
        check("full_realm_coverage_qualified BOOLEAN NOT NULL DEFAULT false" in schema, "global coverage qualification must default false")
        check("coverage_complete BOOLEAN NOT NULL DEFAULT false" in schema, "daily coverage must default false")
        check("CASE WHEN qualified.is_complete THEN metric.bound_gold_stock END AS bound_gold_stock" in schema, "global bound gold must require qualified explicit coverage")
        check("CASE WHEN qualified.is_complete THEN metric.unbound_gold_stock END AS unbound_gold_stock" in schema, "global unbound gold must require qualified explicit coverage")
        check("new_wallet_bound_gold_stock" in schema and "new_wallet_unbound_gold_stock" in schema, "source receipt wallet scope must remain explicit")
        check("ORDER BY m.metric_revision DESC LIMIT 1" in schema, "observation view must select latest immutable metric version")
        check("policy.policy_json -> 'enabled' = 'true'::jsonb" in schema, "realm enabled must use exact policy JSON boolean")
        check("settings.full_realm_coverage_qualified IS TRUE" in schema and "metric.source_period_end = boundary.period_end" in schema, "independent trusted coverage and daily closure required")
        check("hunting_economy_observation_settings_immutable" in schema and "hunting_economy_daily_metrics_immutable" in schema, "observation revisions must preserve immutable history")
        check("SUM(" not in schema.upper(), "must not promote wallet sums to full realm assets")
        check("DEFAULT 72000000" not in schema and "DEFAULT 720000000" not in schema, "baseline must not be guessed from flow")
    return CHECKS - start_checks


def main():
    cases, fixture_assertions = run_fixtures()
    artifact_checks = check_artifacts()
    files = sorted(p for p in ROOT.rglob("*") if p.is_file() and p.name != "validation-result.json" and "__pycache__" not in p.parts)
    result = {
        "status": "passed",
        "scope": "offline artifact checks and independent synthetic rule-model fixtures only",
        "fixture_cases": cases, "fixture_assertions": fixture_assertions, "artifact_checks": artifact_checks,
        "postgres_sql_executed": False, "grafana_import_verified": False, "grafana_render_verified": False,
        "notifications_sent": False, "remote_accessed": False,
        "preliminary_failures": load("verification-history.json"),
        "files_sha256": {str(p.relative_to(ROOT)).replace("\\", "/"): hashlib.sha256(p.read_bytes()).hexdigest() for p in files},
    }
    migration = ROOT.parent / "postgres/migrations/0017_hunting_economy_observability.sql"
    result["schema_migration_sha256"] = hashlib.sha256(migration.read_bytes()).hexdigest() if migration.exists() else None
    (ROOT / "validation-result.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({key: value for key, value in result.items() if key != "files_sha256"}, ensure_ascii=False))


if __name__ == "__main__":
    main()
