-- Observation only: preserves the existing daily view's column order/types and
-- coverage gates. Fixed material valuations are permit-denominated diagnostics;
-- they do not establish market-price coverage or mint new unbound coins.
-- Requires migrations 0016, 0017, 0019 and 0026. Historical JSON is not rewritten.
CREATE OR REPLACE VIEW hunting_permit_daily_observations AS
SELECT s.realm_id,
    date '1970-01-01' + s.business_day::integer AS economy_date,
    boundary.period_end,
    qualified.is_complete,
    s.policy_version,
    (s.receipt_json #>> '{burns,daily}')::numeric AS burn_daily_gold,
    (s.receipt_json #>> '{burns,light_pvp}')::numeric AS burn_light_pvp_gold,
    (s.receipt_json #>> '{burns,siege}')::numeric AS burn_sabuk_gold,
    (s.receipt_json #>> '{plan,minted_atoms}')::numeric AS permit_minted_atoms,
    (s.receipt_json ->> 'consumed_atoms')::numeric AS permit_consumed_atoms,
    (s.receipt_json #>> '{plan,decayed_atoms}')::numeric AS permit_decayed_atoms,
    (s.receipt_json #>> '{plan,next_pool,stock_atoms}')::numeric AS permit_closing_atoms,
    -- These are explicit full-realm observations. Never substitute new-wallet
    -- receipt sums when coverage has not been established.
    CASE WHEN qualified.is_complete THEN metric.bound_gold_stock END AS bound_gold_stock,
    CASE WHEN qualified.is_complete THEN metric.unbound_gold_stock END AS unbound_gold_stock,
    (s.receipt_json ->> 'gross_conversion_gold')::numeric AS gross_converted_gold,
    (s.receipt_json ->> 'gross_conversion_gold')::numeric
        - (s.receipt_json ->> 'net_unbound_gold_inflow')::numeric
        + material.fee_unbound_gold AS conversion_fee_gold,
    (s.receipt_json ->> 'net_unbound_gold_inflow')::numeric AS net_unbound_inflow_gold,
    CASE WHEN settings.policy_version = s.policy_version AND settings.effective_from_day <= s.business_day
        THEN settings.unbound_stock_baseline_gold END AS unbound_stock_baseline_gold,
    CASE WHEN settings.policy_version = s.policy_version AND settings.effective_from_day <= s.business_day
        THEN settings.burn_baseline_gold END AS burn_baseline_gold,
    CASE WHEN settings.policy_version = s.policy_version AND settings.effective_from_day <= s.business_day
        THEN settings.planned_daily_consumption_atoms END AS planned_daily_consumption_atoms,
    CASE WHEN metric.market_data_complete AND metric.monthly_card_completed_quantity > 0
        AND metric.monthly_card_completed_gold > 0
        THEN metric.monthly_card_completed_gold / metric.monthly_card_completed_quantity END AS monthly_card_gold_per_card,
    CASE WHEN metric.material_data_complete THEN metric.raw_material_price_index END AS raw_material_price_index,
    CASE WHEN metric.portrait_data_complete THEN metric.active_casual_accounts END AS active_casual_accounts,
    CASE WHEN metric.portrait_data_complete THEN metric.active_light_pvp_accounts END AS active_light_pvp_accounts,
    CASE WHEN metric.portrait_data_complete THEN metric.active_sabuk_accounts END AS active_sabuk_accounts,
    CASE WHEN metric.portrait_data_complete THEN metric.active_account_total END AS active_account_total,
    CASE WHEN metric.income_data_complete AND metric.income_month = boundary.income_month
        AND metric.heavy_producer_accounts > 0 AND metric.income_monthly_card_gold_per_card > 0
        AND metric.income_monthly_card_usd_reference > 0
        AND metric.income_price_reference_version IS NOT NULL AND length(btrim(metric.income_price_reference_version)) > 0
        THEN metric.heavy_producer_monthly_net_gold / metric.income_monthly_card_gold_per_card
            * metric.income_monthly_card_usd_reference END AS heavy_producer_monthly_usd_equivalent,
    CASE WHEN metric.income_data_complete AND metric.income_month = boundary.income_month
        AND metric.ordinary_producer_accounts > 0 AND metric.income_monthly_card_gold_per_card > 0
        AND metric.income_monthly_card_usd_reference > 0
        AND metric.income_price_reference_version IS NOT NULL AND length(btrim(metric.income_price_reference_version)) > 0
        THEN metric.ordinary_producer_monthly_net_gold / metric.income_monthly_card_gold_per_card
            * metric.income_monthly_card_usd_reference END AS ordinary_producer_monthly_usd_equivalent,
    CASE WHEN metric.income_data_complete AND metric.income_month = boundary.income_month
        AND metric.heavy_producer_accounts > 0 THEN metric.heavy_producer_monthly_net_gold END AS heavy_producer_monthly_net_gold,
    CASE WHEN metric.income_data_complete AND metric.income_month = boundary.income_month
        AND metric.ordinary_producer_accounts > 0 THEN metric.ordinary_producer_monthly_net_gold END AS ordinary_producer_monthly_net_gold,
    CASE WHEN metric.market_data_complete THEN metric.monthly_card_completed_quantity END AS monthly_card_completed_quantity,
    CASE WHEN metric.income_data_complete AND metric.income_month = boundary.income_month
        THEN metric.heavy_producer_accounts END AS heavy_producer_accounts,
    CASE WHEN metric.income_data_complete AND metric.income_month = boundary.income_month
        THEN metric.ordinary_producer_accounts END AS ordinary_producer_accounts,
    -- Diagnostics preserve the restricted scope of the source receipt. They are
    -- not used as complete realm stock in either dashboards or business alerts.
    (s.receipt_json ->> 'bound_gold_stock')::numeric AS new_wallet_bound_gold_stock,
    (s.receipt_json ->> 'unbound_gold_stock')::numeric AS new_wallet_unbound_gold_stock,
    metric.metric_revision,
    settings.settings_revision,
    metric.coverage_complete,
    metric.coverage_scope,
    metric.source_ledger_watermark,
    s.committed_at_ms AS source_settlement_committed_at_ms,
    -- Permit valuation is not coin inflow or destruction. Only the separately
    -- debited existing unbound fee contributes to daily coin destruction.
    material.gold_equivalent AS material_conversion_gold_equivalent,
    material.fee_unbound_gold AS material_conversion_fee_unbound_gold,
    (s.receipt_json ->> 'gross_conversion_gold')::numeric
        - (s.receipt_json ->> 'net_unbound_gold_inflow')::numeric AS gold_conversion_fee_gold
FROM hunting_economy_settlements s
CROSS JOIN LATERAL (
    -- Old receipts omitted material fields. Treat absence as zero only after a
    -- durable exporter receipt validated this exact immutable outbox payload.
    -- Consumer v2 appends its own receipt without replacing the original v1 ACK.
    SELECT EXISTS (
        SELECT 1 FROM hunting_economy_metric_exports x
        JOIN hunting_economy_outbox e
            ON e.realm_id=x.realm_id AND e.event_id=x.event_id
        WHERE x.realm_id=s.realm_id AND x.business_day=s.business_day
            AND x.consumer_version IN ('hunting-daily-new-wallet-diagnostics-v1',
                'hunting-daily-new-wallet-diagnostics-v2')
            AND e.event_kind='daily_settlement'
            AND e.payload_json=s.receipt_json
    ) AS source_verified
) exported
CROSS JOIN LATERAL (
    SELECT CASE WHEN s.receipt_json ? 'material_conversion_gold_equivalent'
        THEN (s.receipt_json ->> 'material_conversion_gold_equivalent')::numeric
        WHEN exported.source_verified THEN 0::numeric END AS gold_equivalent,
        CASE WHEN s.receipt_json ? 'material_conversion_fee_unbound_gold'
        THEN (s.receipt_json ->> 'material_conversion_fee_unbound_gold')::numeric
        WHEN exported.source_verified THEN 0::numeric END AS fee_unbound_gold
) material
CROSS JOIN LATERAL (
    SELECT ((date '1970-01-01' + s.business_day::integer + 1) + time '04:00')
        AT TIME ZONE 'Asia/Shanghai' AS period_end,
        date_trunc('month', (date '1970-01-01' + s.business_day::integer)::timestamp)::date AS income_month
) boundary
LEFT JOIN LATERAL (
    SELECT m.* FROM hunting_economy_daily_metric_versions m
    WHERE m.realm_id = s.realm_id AND m.business_day = s.business_day
    ORDER BY m.metric_revision DESC LIMIT 1
) metric ON true
LEFT JOIN LATERAL (
    SELECT cfg.* FROM hunting_economy_observation_settings cfg
    WHERE cfg.realm_id = s.realm_id AND (
        (metric.settings_revision IS NOT NULL AND cfg.settings_revision = metric.settings_revision)
        OR (metric.settings_revision IS NULL AND cfg.policy_version = s.policy_version
            AND cfg.effective_from_day <= s.business_day)
    )
    ORDER BY cfg.effective_from_day DESC, cfg.settings_revision DESC LIMIT 1
) settings ON true
CROSS JOIN LATERAL (
    SELECT (
        metric.financial_period_closed IS TRUE
        AND metric.coverage_complete IS TRUE
        AND metric.coverage_scope = 'all_realm_assets'
        AND metric.coverage_evidence IS NOT NULL AND length(btrim(metric.coverage_evidence)) > 0
        AND metric.source_ledger_watermark IS NOT NULL AND length(btrim(metric.source_ledger_watermark)) > 0
        AND metric.source_period_end = boundary.period_end
        AND metric.bound_gold_stock IS NOT NULL AND metric.unbound_gold_stock IS NOT NULL
        AND settings.full_realm_coverage_qualified IS TRUE
        AND settings.policy_version = s.policy_version
        AND settings.effective_from_day <= s.business_day
        AND s.committed_at_ms::numeric >= extract(epoch FROM boundary.period_end) * 1000
        AND (s.receipt_json ->> 'realm_id') = s.realm_id
        AND (s.receipt_json ->> 'business_day')::bigint = s.business_day
        AND (s.receipt_json ->> 'policy_version') = s.policy_version
    ) IS TRUE AS is_complete
) qualified;

COMMENT ON VIEW hunting_permit_daily_observations IS
    'Read-only latest metric revision linked to immutable settlement. Full-realm and market coverage remain explicitly qualified. Material valuation consumes permits without coin inflow; only its unbound fee burns coins. Missing historical material fields are zero only for the exact exporter-verified payload.';
REVOKE ALL ON hunting_permit_daily_observations FROM PUBLIC;
