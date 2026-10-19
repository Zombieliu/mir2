"""Offline integer comparison model; never an authoritative game asset service.

Reads the server policy verbatim. `enabled` is not bypass permission: calculations
remain available while disabled, but a production server must enforce that flag.
Daily destruction supplied to this model is the ALL-IN ledger aggregate, including
fees; fees from requested conversions are reported but are not added to it twice.
"""

from __future__ import annotations

import argparse
import copy
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
U64_MAX = (1 << 64) - 1
U128_MAX = (1 << 128) - 1
I64_MIN = -(1 << 63)
I64_MAX = (1 << 63) - 1
LICENSE_ATOMS_PER_UNIT = 1_000_000
GOLD_PER_LICENSE = 1_000_000


class PolicyError(ValueError):
    def __init__(self, code: str, field: str = ""):
        self.code = code
        self.field = field
        super().__init__(f"{code}: {field}")


def _integer(value: int, low: int, high: int, field: str) -> int:
    if type(value) is not int or not low <= value <= high:
        raise PolicyError("overflow", field)
    return value


def _u64(value: int, field: str) -> int:
    return _integer(value, 0, U64_MAX, field)


def _i64(value: int, field: str) -> int:
    return _integer(value, I64_MIN, I64_MAX, field)


def _u128(value: int, field: str) -> int:
    return _integer(value, 0, U128_MAX, field)


def _ratio(value: dict, field: str) -> None:
    if not isinstance(value, dict) or set(value) != {"numerator", "denominator"}:
        raise PolicyError("invalid_policy", field)
    _u64(value["numerator"], field)
    _u64(value["denominator"], field)
    if value["numerator"] == 0 or value["denominator"] == 0:
        raise PolicyError("invalid_policy", field)


def _cap_unchecked(policy: dict) -> int:
    ratio = policy["daily_mint_cap_multiplier"]
    product = _u128(policy["baseline_daily_mint_atoms"] * ratio["numerator"], "mint_cap_product")
    return _u64(product // ratio["denominator"], "mint_cap")


def validate_policy(policy: dict) -> None:
    expected = {
        "schema_version", "policy_version", "enabled", "k", "conversion_fee",
        "daily_decay", "baseline_daily_mint_atoms", "daily_mint_cap_multiplier",
        "business_timezone", "settlement_hour",
    }
    if not isinstance(policy, dict) or set(policy) != expected:
        raise PolicyError("invalid_policy", "policy_fields")
    if type(policy["schema_version"]) is not int or policy["schema_version"] != 1:
        raise PolicyError("invalid_policy", "schema_version")
    version = policy["policy_version"]
    if not isinstance(version, str) or not version.strip() or len(version.encode("utf-8")) > 128:
        raise PolicyError("invalid_policy", "policy_version")
    if type(policy["enabled"]) is not bool:
        raise PolicyError("invalid_policy", "enabled")
    for field in ("k", "conversion_fee", "daily_decay", "daily_mint_cap_multiplier"):
        _ratio(policy[field], field)
    for field in ("conversion_fee", "daily_decay"):
        ratio = policy[field]
        if ratio["numerator"] >= ratio["denominator"]:
            raise PolicyError("invalid_policy", field)
    baseline = _u64(policy["baseline_daily_mint_atoms"], "baseline_daily_mint_atoms")
    if baseline == 0:
        raise PolicyError("invalid_policy", "baseline_daily_mint_atoms")
    ratio = policy["daily_mint_cap_multiplier"]
    if ratio["numerator"] < ratio["denominator"]:
        raise PolicyError("invalid_policy", "daily_mint_cap_multiplier")
    if policy["business_timezone"] != "Asia/Shanghai" or type(policy["settlement_hour"]) is not int or policy["settlement_hour"] != 4:
        raise PolicyError("invalid_policy", "business_clock")
    _cap_unchecked(policy)


def daily_mint_cap_atoms(policy: dict) -> int:
    validate_policy(policy)
    return _cap_unchecked(policy)


def plan_settlement(policy: dict, pool: dict, destruction_gold_equivalent: int) -> dict:
    validate_policy(policy)
    if not isinstance(pool, dict) or set(pool) != {"stock_atoms", "mint_remainder", "decay_remainder"}:
        raise PolicyError("invalid_remainder", "pool_fields")
    for field, value in pool.items():
        _u64(value, field)
    destruction = _u64(destruction_gold_equivalent, "destruction_gold_equivalent")
    if pool["mint_remainder"] >= policy["k"]["denominator"]:
        raise PolicyError("invalid_remainder", "mint_remainder")
    if pool["decay_remainder"] >= policy["daily_decay"]["denominator"]:
        raise PolicyError("invalid_remainder", "decay_remainder")
    numerator = _u128(destruction * policy["k"]["numerator"] + pool["mint_remainder"], "mint_numerator")
    raw, mint_remainder = divmod(numerator, policy["k"]["denominator"])
    raw = _u64(raw, "raw_mint_atoms")
    cap = _cap_unchecked(policy)
    minted = min(raw, cap)
    before_decay = _u64(pool["stock_atoms"] + minted, "pre_decay_stock_atoms")
    numerator = _u128(before_decay * policy["daily_decay"]["numerator"] + pool["decay_remainder"], "decay_numerator")
    decayed, decay_remainder = divmod(numerator, policy["daily_decay"]["denominator"])
    return {
        "destruction_gold_equivalent": destruction,
        "opening_stock_atoms": pool["stock_atoms"],
        "raw_mint_atoms": raw,
        "daily_mint_cap_atoms": cap,
        "minted_atoms": minted,
        "capped_atoms": raw - minted,
        "pre_decay_stock_atoms": before_decay,
        "decayed_atoms": _u64(decayed, "decayed_atoms"),
        "next_pool": {
            "stock_atoms": _u64(before_decay - decayed, "closing_stock_atoms"),
            "mint_remainder": mint_remainder,
            "decay_remainder": decay_remainder,
        },
    }


def quote_conversion(policy: dict, gross_gold: int) -> dict:
    validate_policy(policy)
    gross_gold = _u64(gross_gold, "gross_gold")
    if gross_gold == 0:
        raise PolicyError("invalid_amount", "gross_gold")
    ratio = policy["conversion_fee"]
    numerator = _u128(gross_gold * ratio["numerator"], "fee_numerator")
    fee, remainder = divmod(numerator, ratio["denominator"])
    fee = _u64(fee + int(remainder != 0), "fee_unbound_gold")
    return {
        "bound_gold_debit": gross_gold,
        "license_atoms_debit": gross_gold,
        "fee_unbound_gold": fee,
        "net_unbound_gold_credit": _u64(gross_gold - fee, "net_unbound_gold_credit"),
    }


def business_day_key(unix_seconds: int) -> int:
    value = _i64(unix_seconds, "business_day_key")
    value = _i64(value + 28_800, "business_day_key")
    value = _i64(value - 14_400, "business_day_key")
    return value // 86_400


def settlement_window(day_key: int) -> tuple[int, int]:
    start = _i64(_i64(day_key, "day_key") * 86_400, "settlement_window_start")
    start = _i64(start - 28_800, "settlement_window_start")
    start = _i64(start + 14_400, "settlement_window_start")
    return start, _i64(start + 86_400, "settlement_window_end")


def load_policy(path: Path | str | None = None) -> dict:
    policy = json.loads(Path(path or ROOT / "config/hunting-economy.json").read_text(encoding="utf-8"))
    validate_policy(policy)
    return policy


def simulate(policy: dict, days: int, daily_destruction_gold: int, daily_conversion_gold: int = 0, initial_stock_atoms: int = 0) -> list[dict]:
    """Replay aggregate days; insufficient requests are wholly rejected.

    These arguments are supplied observations/scenarios, not invented game events.
    The caller supplies all-in destruction so reported fees are not counted twice.
    """
    if type(days) is not int or not 0 <= days <= 36_600:
        raise PolicyError("invalid_amount", "days")
    _u64(daily_conversion_gold, "daily_conversion_gold")
    pool = {"stock_atoms": _u64(initial_stock_atoms, "initial_stock_atoms"), "mint_remainder": 0, "decay_remainder": 0}
    results = []
    for day in range(days):
        plan = plan_settlement(policy, pool, daily_destruction_gold)
        pool = copy.deepcopy(plan["next_pool"])
        conversion = None
        status = "not_requested"
        if daily_conversion_gold:
            if pool["stock_atoms"] < daily_conversion_gold:
                status = "insufficient_global_quota"
            else:
                conversion = quote_conversion(policy, daily_conversion_gold)
                pool["stock_atoms"] -= conversion["license_atoms_debit"]
                status = "accepted_offline_scenario"
        results.append({"day_index": day, "settlement": plan, "conversion_status": status, "conversion": conversion, "closing_pool": copy.deepcopy(pool)})
    return results


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--policy", type=Path, default=ROOT / "config/hunting-economy.json")
    parser.add_argument("--days", type=int, default=30)
    parser.add_argument("--daily-destruction-gold", type=int, default=7_200_000)
    parser.add_argument("--daily-conversion-gold", type=int, default=0)
    parser.add_argument("--initial-stock-atoms", type=int, default=0)
    args = parser.parse_args()
    policy = load_policy(args.policy)
    result = simulate(policy, args.days, args.daily_destruction_gold, args.daily_conversion_gold, args.initial_stock_atoms)
    print(json.dumps({"offline_only": True, "policy_version": policy["policy_version"], "policy_enabled": policy["enabled"], "days": result}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
