"""Build deterministic, data-free Grafana provisioning from the view contract.

Standard library only. This writes configuration, never accesses a database.
The JSON syntax used for .yaml files is valid YAML 1.2 and permits dependency-
free artifact verification. Do not run against an existing Grafana deployment.
"""
from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
DS = {"type": "grafana-postgresql-datasource", "uid": "mir2-economy-readonly"}
DAILY = "public.hunting_permit_daily_observations"
REALMS = "public.hunting_permit_observability_realms"


def save(relative: str, value: object) -> None:
    path = ROOT / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def window_sql(days: int, predicate: str, required: str) -> str:
    # Anchoring on the database clock instead of max(observed_date) makes missing
    # recent dates visible. LEFT JOIN preserves a missing day as a missing day.
    return f"""WITH bounds AS (
  SELECT ((now() AT TIME ZONE 'Asia/Shanghai' - interval '4 hours')::date - 1) AS closed_date
), calendar AS (
  SELECT r.realm_id, d::date AS economy_date
  FROM {REALMS} r CROSS JOIN bounds b
  CROSS JOIN LATERAL generate_series(b.closed_date - ({days} - 1), b.closed_date, interval '1 day') d
  WHERE r.enabled IS TRUE
)
SELECT now() AS time, c.realm_id,
  CASE WHEN count(o.economy_date) = {days}
    AND bool_and(o.is_complete IS TRUE)
    AND bool_and(({required}) IS TRUE)
  THEN CASE WHEN bool_and(({predicate}) IS TRUE) THEN 1.0 ELSE 0.0 END
  ELSE NULL::double precision END AS value
FROM calendar c LEFT JOIN {DAILY} o
  ON o.realm_id = c.realm_id AND o.economy_date = c.economy_date
GROUP BY c.realm_id ORDER BY c.realm_id;"""


CORE_FIELDS = [
    "burn_daily_gold", "burn_light_pvp_gold", "burn_sabuk_gold",
    "permit_minted_atoms", "permit_consumed_atoms", "permit_decayed_atoms", "permit_closing_atoms",
    "bound_gold_stock", "unbound_gold_stock", "gross_converted_gold", "conversion_fee_gold", "net_unbound_inflow_gold",
]
HEALTH_REQUIRED = " AND\n      ".join(f"o.{field} IS NOT NULL AND o.{field} >= 0" for field in CORE_FIELDS)
HEALTH_REQUIRED += """ AND
      o.unbound_stock_baseline_gold > 0 AND o.burn_baseline_gold > 0
      AND o.planned_daily_consumption_atoms > 0
      AND o.policy_version IS NOT NULL AND length(o.policy_version) > 0
      AND o.period_end = ((o.economy_date + 1) + time '04:00') AT TIME ZONE 'Asia/Shanghai'
      AND o.gross_converted_gold::numeric = o.conversion_fee_gold::numeric + o.net_unbound_inflow_gold::numeric
      AND o.active_casual_accounts IS NOT NULL AND o.active_casual_accounts >= 0
      AND o.active_light_pvp_accounts IS NOT NULL AND o.active_light_pvp_accounts >= 0
      AND o.active_sabuk_accounts IS NOT NULL AND o.active_sabuk_accounts >= 0
      AND o.active_account_total = o.active_casual_accounts + o.active_light_pvp_accounts + o.active_sabuk_accounts"""

ALERTS = {
    "inflation": {
        "title": "可交易金币通胀：连续7完整日高于存量基准130%", "days": 7, "severity": "red",
        "predicate": "o.unbound_gold_stock::numeric > o.unbound_stock_baseline_gold::numeric * 1.30",
        "required": "o.unbound_gold_stock >= 0 AND o.unbound_stock_baseline_gold > 0",
    },
    "deflation": {
        "title": "可交易金币通缩：连续7完整日低于存量基准70%", "days": 7, "severity": "yellow",
        "predicate": "o.unbound_gold_stock::numeric < o.unbound_stock_baseline_gold::numeric * 0.70",
        "required": "o.unbound_gold_stock >= 0 AND o.unbound_stock_baseline_gold > 0",
    },
    "permit_exhaustion": {
        "title": "许可库存：连续10完整日不足1天计划需求", "days": 10, "severity": "yellow",
        "predicate": "o.permit_closing_atoms < o.planned_daily_consumption_atoms",
        "required": "o.permit_closing_atoms >= 0 AND o.planned_daily_consumption_atoms > 0",
    },
    "burn_pulse": {
        "title": "销毁脉冲：完整经济日超稳态150%", "days": 1, "severity": "red",
        "predicate": "(o.burn_daily_gold::numeric + o.burn_light_pvp_gold::numeric + o.burn_sabuk_gold::numeric) > o.burn_baseline_gold::numeric * 1.50",
        "required": "o.burn_daily_gold >= 0 AND o.burn_light_pvp_gold >= 0 AND o.burn_sabuk_gold >= 0 AND o.burn_baseline_gold > 0",
    },
}

SQL = {key: window_sql(spec["days"], spec["predicate"], spec["required"]) for key, spec in ALERTS.items()}
SQL["data_integrity"] = window_sql(10, "TRUE", HEALTH_REQUIRED).replace(
    "THEN CASE WHEN bool_and((TRUE) IS TRUE) THEN 1.0 ELSE 0.0 END\n  ELSE NULL::double precision END AS value",
    "THEN 0.0 ELSE 1.0 END AS value",
)


def chart_sql(expressions: dict[str, str], latest: bool = False) -> str:
    # Emit an actual NULL row for every absent date to prevent a chart drawing
    # a continuous line through an ingestion gap. Daily charts use period_end.
    values = ",\n  ".join(
        f'CASE WHEN o.is_complete IS TRUE THEN {expr} ELSE NULL END AS "{name}"'
        for name, expr in expressions.items()
    )
    sql = f"""WITH calendar AS (
  SELECT d::date AS economy_date,
    ((d::date + 1) + time '04:00') AT TIME ZONE 'Asia/Shanghai' AS period_end
  FROM generate_series(
    (to_timestamp($__unixEpochFrom()) AT TIME ZONE 'Asia/Shanghai')::date - 1,
    (to_timestamp($__unixEpochTo()) AT TIME ZONE 'Asia/Shanghai')::date,
    interval '1 day') d
)
SELECT c.period_end AS time,
  {values}
FROM calendar c LEFT JOIN {DAILY} o
  ON o.economy_date = c.economy_date AND o.realm_id IN (${{realm:sqlstring}})
WHERE $__timeFilter(c.period_end)
  AND c.period_end <= (date_trunc('day', now() AT TIME ZONE 'Asia/Shanghai' - interval '4 hours') AT TIME ZONE 'Asia/Shanghai') + interval '4 hours'
ORDER BY c.period_end {'DESC LIMIT 1' if latest else 'ASC'};"""
    return sql


def panel(panel_id: int, title: str, expressions: dict[str, str], x: int, y: int,
          w: int = 12, h: int = 9, kind: str = "timeseries", unit: str = "short",
          description: str = "", stacked: bool = False, latest: bool = False) -> dict:
    custom = {"spanNulls": False, "drawStyle": "line", "lineWidth": 2, "fillOpacity": 10}
    if stacked:
        custom.update({"stacking": {"mode": "normal", "group": "A"}, "fillOpacity": 45})
    result = {
        "id": panel_id, "type": kind, "title": title, "description": description,
        "gridPos": {"h": h, "w": w, "x": x, "y": y}, "datasource": DS,
        "targets": [{"refId": "A", "datasource": DS, "rawQuery": True,
                     "format": "time_series", "rawSql": chart_sql(expressions, latest=latest)}],
        "fieldConfig": {"defaults": {"unit": unit, "noValue": "暂无有效数据", "custom": custom}, "overrides": []},
        "options": {"legend": {"displayMode": "list", "placement": "bottom"}, "tooltip": {"mode": "multi", "sort": "none"}},
    }
    if kind == "piechart":
        result["options"] = {"pieType": "pie", "displayLabels": ["name", "percent"],
                             "legend": {"displayMode": "table", "placement": "right", "values": ["value", "percent"]},
                             "reduceOptions": {"calcs": ["last"], "fields": "", "values": False}}
    return result


def main() -> None:
    for key, sql in SQL.items():
        path = ROOT / f"sql/alerts/{key}.sql"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(sql + "\n", encoding="utf-8")

    save("grafana/provisioning/datasources/economy.yaml", {
        "apiVersion": 1, "datasources": [{
            "name": "Mir2 Economy Readonly", "uid": DS["uid"], "type": DS["type"],
            "access": "proxy", "url": "$MIR2_ECONOMY_DB_HOST", "user": "$MIR2_ECONOMY_DB_USER",
            "isDefault": False, "editable": False,
            "jsonData": {"database": "$MIR2_ECONOMY_DB_NAME", "sslmode": "verify-full", "postgresVersion": 1500,
                         "maxOpenConns": 5, "maxIdleConns": 2, "connMaxLifetime": 14400, "timescaledb": False},
            "secureJsonData": {"password": "$MIR2_ECONOMY_DB_PASSWORD"},
        }],
    })
    save("grafana/provisioning/dashboards/economy.yaml", {
        "apiVersion": 1, "providers": [{"name": "Mir2 Economy", "orgId": 1, "folder": "Mir2 Economy",
        "type": "file", "disableDeletion": True, "editable": False, "updateIntervalSeconds": 60,
        "options": {"path": "/var/lib/grafana/economy-dashboards"}}],
    })

    panels = [{"id": 1, "type": "text", "title": "经济监控口径", "gridPos": {"h": 4, "w": 24, "x": 0, "y": 0},
               "options": {"mode": "markdown", "content": "北京时间04:00切日；金额单位为金币，许可单位为1许可=100万金币额度。图表显示完整经济日快照；空值表示缺数据。月卡美元值为等价估值，不是提现收入。此面板只读，不控制游戏资产。"}}]
    panels += [
        panel(2, "每日三层销毁金币当量", {"日常保底": "o.burn_daily_gold", "轻PVP": "o.burn_light_pvp_gold", "沙巴克脉冲": "o.burn_sabuk_gold"}, 0, 4, description="只计实际永久销毁；转移给其他玩家的货物不计销毁。", stacked=True),
        panel(3, "许可每日新增、消耗与衰减", {"实际新增许可": "o.permit_minted_atoms / 1000000.0", "玩家消耗许可": "o.permit_consumed_atoms / 1000000.0", "衰减许可": "o.permit_decayed_atoms / 1000000.0"}, 12, 4, description="许可原子在查询时除以1,000,000；不使用金币数充当许可数量。"),
        panel(4, "全局许可日结库存", {"日结库存许可": "o.permit_closing_atoms / 1000000.0", "计划每日需求许可": "o.planned_daily_consumption_atoms / 1000000.0"}, 0, 13, description="最新完整日结快照；不代表此刻可转换额度。实时请求始终由主服务端校验。"),
        panel(5, "月卡凭证市场汇率 · 90天", {"非绑定金币/张月卡": "o.monthly_card_gold_per_card"}, 12, 13, description="有效真实成交金币总额÷成交月卡张数；无成交时为空。排除无效、撤销、自成交及已确认操纵成交。"),
        panel(6, "原材料价格指数 · 固定篮子基期100", {"价格指数": "o.raw_material_price_index"}, 0, 22, description="固定权重与基期的版本化篮子。无有效覆盖时为空，不能通过动态删掉贵材料制造稳定价格。"),
        panel(7, "绑定与可交易金币日结存量", {"绑定金币": "o.bound_gold_stock", "非绑定金币": "o.unbound_gold_stock", "非绑定稳态基准": "o.unbound_stock_baseline_gold", "通胀130%": "o.unbound_stock_baseline_gold * 1.30", "通缩70%": "o.unbound_stock_baseline_gold * 0.70"}, 12, 22, description="包括在线和离线钱包、行会、托管、邮件未领取金币；所有权唯一计数。存量基准不能使用7200万/天流量。"),
        panel(8, "绑定转换：毛额、费用与净流入", {"转换毛额": "o.gross_converted_gold", "销毁手续费": "o.conversion_fee_gold", "净非绑定流入": "o.net_unbound_inflow_gold"}, 0, 31, description="每笔毛额=手续费+净到账。这里仅展示转换净流入；其他产币来源需独立记账。"),
        panel(9, "每日互斥活跃账号", {"休闲生产": "o.active_casual_accounts", "轻度PVP": "o.active_light_pvp_accounts", "硬核沙巴克": "o.active_sabuk_accounts"}, 12, 31, stacked=True, description="账号去重；同日按沙巴克＞轻PVP＞休闲优先级互斥归类，三类和=总DAU。"),
        panel(10, "所选范围最后完整日玩家构成", {"休闲生产": "o.active_casual_accounts", "轻度PVP": "o.active_light_pvp_accounts", "硬核沙巴克": "o.active_sabuk_accounts"}, 0, 40, kind="piechart", latest=True, description="按所选时间范围最后一个已闭合经济日取值；该日缺失时显示无数据，不回退旧日冒充。"),
        panel(11, "生产玩家月度人均等价美元价值", {"重度种田": "o.heavy_producer_monthly_usd_equivalent", "普通休闲": "o.ordinary_producer_monthly_usd_equivalent"}, 12, 40, unit="currencyUSD", description="当月截至该日人均净非绑定收益÷月卡金币成交价×月卡美元参考价；不得标记实际提现。价格、玩家样本或成本覆盖不足时为空。"),
        panel(12, "生产玩家月度人均净金币收益", {"重度种田": "o.heavy_producer_monthly_net_gold", "普通休闲": "o.ordinary_producer_monthly_net_gold"}, 0, 49, description="按账号和政策版本归类，收入减已覆盖成本；不能只取正收益玩家。与美元估值共享人口及月份。"),
        panel(13, "月卡成交量与生产画像样本", {"月卡成交张数": "o.monthly_card_completed_quantity", "重度种田账号": "o.heavy_producer_accounts", "普通休闲账号": "o.ordinary_producer_accounts"}, 12, 49, description="成交量为日量，画像为截至该经济日所属月份的样本账号数。真实零与未接入NULL明确区分。"),
    ]
    for p in panels:
        if p["id"] == 5:
            p["timeFrom"] = "90d"
        if p["id"] == 3:
            p["fieldConfig"]["defaults"]["custom"].update({"drawStyle": "bars", "barAlignment": 0, "fillOpacity": 70, "lineWidth": 0})
    panels.append({"id": 14, "type": "stat", "title": "经济数据完整性（0正常 / 1异常）",
                   "description": "覆盖最近10个闭合经济日、资金字段、画像互斥和配置基准。缺失的价格保持NULL；无成交价格不是零。",
                   "gridPos": {"h": 5, "w": 24, "x": 0, "y": 58}, "datasource": DS,
                   "targets": [{"refId": "A", "datasource": DS, "rawQuery": True, "format": "time_series", "rawSql": "SELECT * FROM (\n" + SQL["data_integrity"].rstrip(";") + "\n) health WHERE realm_id IN (${realm:sqlstring});"}],
                   "fieldConfig": {"defaults": {"noValue": "无数据", "thresholds": {"mode": "absolute", "steps": [{"color": "green", "value": None}, {"color": "red", "value": 1}]}}, "overrides": []},
                   "options": {"reduceOptions": {"calcs": ["last"], "fields": "", "values": False}, "colorMode": "background"}})
    save("grafana/dashboards/hunting-permit.json", {
        "uid": "mir2-hunting-permit", "title": "Mir2 狩猎许可与资产经济", "version": 1, "schemaVersion": 39,
        "editable": False, "graphTooltip": 1, "refresh": "1m", "timezone": "Asia/Shanghai",
        "time": {"from": "now-90d", "to": "now"}, "tags": ["mir2", "economy", "read-only"], "annotations": {"list": []},
        "templating": {"list": [{"name": "realm", "label": "经济服", "type": "query", "datasource": DS,
                                   "query": f"SELECT realm_id AS __text, realm_id AS __value FROM {REALMS} WHERE enabled IS TRUE ORDER BY realm_id",
                                   "refresh": 1, "multi": False, "includeAll": False, "current": {"selected": False, "text": "", "value": ""}, "options": []}]},
        "panels": panels,
    })

    rules = []
    for key, sql in SQL.items():
        spec = ALERTS.get(key, {"title": "经济日结数据、配置或画像完整性异常", "severity": "red"})
        rules.append({"uid": f"mir2-econ-{key.replace('_', '-')}", "title": spec["title"], "condition": "C", "for": "10m",
                      "noDataState": "Alerting" if key == "data_integrity" else "NoData", "execErrState": "Error", "isPaused": False,
                      "labels": {"service": "mir2-economy", "severity": spec["severity"], "economy_rule": key},
                      "annotations": {"summary": spec["title"], "description": "按北京时间04:00闭合的经济日和版本化基准计算；缺日期或缺基准产生数据异常，不代表业务阈值已触发。此告警只通知，不变更游戏资产。"},
                      "data": [
                          {"refId": "A", "relativeTimeRange": {"from": 864000, "to": 0}, "datasourceUid": DS["uid"],
                           "model": {"refId": "A", "datasource": DS, "rawQuery": True, "format": "time_series", "rawSql": sql, "intervalMs": 60000, "maxDataPoints": 1000}},
                          {"refId": "B", "relativeTimeRange": {"from": 0, "to": 0}, "datasourceUid": "__expr__",
                           # Strict is the native default. An explicit mode named
                           # "strict" is rejected by Grafana's expression parser.
                           "model": {"refId": "B", "type": "reduce", "expression": "A", "reducer": "last"}},
                          {"refId": "C", "relativeTimeRange": {"from": 0, "to": 0}, "datasourceUid": "__expr__",
                           "model": {"refId": "C", "type": "threshold", "expression": "B",
                                     "conditions": [{"evaluator": {"type": "gt", "params": [0.5]}, "operator": {"type": "and"}, "reducer": {"type": "last", "params": []}, "query": {"params": ["C"]}, "type": "query"}]}},
                      ]})
    save("grafana/provisioning/alerting/economy-rules.yaml", {"apiVersion": 1, "groups": [{"orgId": 1, "name": "Mir2 daily economy", "folder": "Mir2 Economy", "interval": "1m", "rules": rules}]})
    save("grafana/provisioning/alerting/economy-contact-points.yaml", {"apiVersion": 1, "contactPoints": [{"orgId": 1, "name": "mir2-economy-chatops", "receivers": [{"uid": "mir2-economy-webhook", "type": "webhook", "disableResolveMessage": False, "settings": {"url": "$MIR2_ECONOMY_ALERT_RELAY_URL", "httpMethod": "POST", "authorization_scheme": "Bearer", "authorization_credentials": "$MIR2_ECONOMY_ALERT_RELAY_TOKEN"}}]}]})
    # Do not load this automatically: provisioning a policy replaces the complete
    # existing notification policy tree. An operator must merge this route.
    save("grafana/economy-notification-route.example", {"apiVersion": 1, "policies": [{"orgId": 1, "receiver": "EXISTING_DEFAULT_RECEIVER", "routes": [{"receiver": "mir2-economy-chatops", "object_matchers": [["service", "=", "mir2-economy"]], "group_by": ["alertname", "realm_id", "severity"], "group_wait": "30s", "group_interval": "5m", "repeat_interval": "4h"}]}]})


if __name__ == "__main__":
    main()
