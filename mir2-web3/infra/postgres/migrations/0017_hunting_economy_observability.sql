-- Observation only: never modifies wallets, pools, settlements or legacy gold.
-- NULL is deliberately different from an observed zero. No default baseline is
-- inferred from daily monetary flow, and new-wallet totals are not realm totals.
CREATE TABLE IF NOT EXISTS hunting_economy_observation_settings (
    realm_id TEXT NOT NULL REFERENCES hunting_economy_pools(realm_id),
    settings_revision BIGINT NOT NULL CHECK (settings_revision > 0),
    policy_version TEXT NOT NULL,
    effective_from_day BIGINT NOT NULL,
    unbound_stock_baseline_gold NUMERIC(38,0)
        CHECK (unbound_stock_baseline_gold > 0),
    burn_baseline_gold NUMERIC(38,0) CHECK (burn_baseline_gold > 0),
    planned_daily_consumption_atoms NUMERIC(38,0)
        CHECK (planned_daily_consumption_atoms > 0),
    full_realm_coverage_qualified BOOLEAN NOT NULL DEFAULT false,
    coverage_qualification_evidence TEXT,
    coverage_qualified_by TEXT,
    coverage_qualified_at_ms BIGINT CHECK (coverage_qualified_at_ms >= 0),
    created_at_ms BIGINT NOT NULL CHECK (created_at_ms >= 0),
    PRIMARY KEY (realm_id, settings_revision),
    FOREIGN KEY (realm_id, policy_version)
        REFERENCES hunting_economy_policies(realm_id, policy_version),
    CHECK (NOT full_realm_coverage_qualified OR (
        coverage_qualification_evidence IS NOT NULL
        AND length(btrim(coverage_qualification_evidence)) > 0
        AND coverage_qualified_by IS NOT NULL
        AND length(btrim(coverage_qualified_by)) > 0
        AND coverage_qualified_at_ms IS NOT NULL
    ))
);
CREATE INDEX IF NOT EXISTS hunting_economy_observation_settings_day_idx
    ON hunting_economy_observation_settings(realm_id, policy_version, effective_from_day, settings_revision);

-- Append a higher metric_revision to publish a correction. Every previous
-- revision and its source settlement/settings link remains available forever.
-- Only a trusted server aggregator/approved operator may write these tables.
CREATE TABLE IF NOT EXISTS hunting_economy_daily_metric_versions (
    realm_id TEXT NOT NULL,
    business_day BIGINT NOT NULL,
    metric_revision BIGINT NOT NULL CHECK (metric_revision > 0),
    settings_revision BIGINT,
    metric_source TEXT NOT NULL CHECK (length(btrim(metric_source)) > 0),
    metric_evidence JSONB NOT NULL DEFAULT '{}'::jsonb
        CHECK (jsonb_typeof(metric_evidence) = 'object'),
    created_at_ms BIGINT NOT NULL CHECK (created_at_ms >= 0),
    financial_period_closed BOOLEAN NOT NULL DEFAULT false,
    source_period_end TIMESTAMPTZ,
    source_ledger_watermark TEXT,
    coverage_complete BOOLEAN NOT NULL DEFAULT false,
    coverage_scope TEXT NOT NULL DEFAULT 'new_wallets_only'
        CHECK (coverage_scope IN ('new_wallets_only', 'all_realm_assets')),
    coverage_evidence TEXT,
    bound_gold_stock NUMERIC(38,0) CHECK (bound_gold_stock >= 0),
    unbound_gold_stock NUMERIC(38,0) CHECK (unbound_gold_stock >= 0),
    market_data_complete BOOLEAN NOT NULL DEFAULT false,
    monthly_card_completed_gold NUMERIC(38,0) CHECK (monthly_card_completed_gold >= 0),
    monthly_card_completed_quantity BIGINT CHECK (monthly_card_completed_quantity >= 0),
    market_source_evidence TEXT,
    material_data_complete BOOLEAN NOT NULL DEFAULT false,
    raw_material_price_index NUMERIC(38,12) CHECK (raw_material_price_index > 0),
    material_index_version TEXT,
    material_priced_weight_fraction NUMERIC(18,12)
        CHECK (material_priced_weight_fraction >= 0 AND material_priced_weight_fraction <= 1),
    portrait_data_complete BOOLEAN NOT NULL DEFAULT false,
    active_casual_accounts BIGINT CHECK (active_casual_accounts >= 0),
    active_light_pvp_accounts BIGINT CHECK (active_light_pvp_accounts >= 0),
    active_sabuk_accounts BIGINT CHECK (active_sabuk_accounts >= 0),
    active_account_total BIGINT CHECK (active_account_total >= 0),
    portrait_policy_version TEXT,
    income_data_complete BOOLEAN NOT NULL DEFAULT false,
    income_month DATE CHECK (extract(day FROM income_month) = 1),
    heavy_producer_monthly_net_gold NUMERIC(38,12),
    ordinary_producer_monthly_net_gold NUMERIC(38,12),
    heavy_producer_accounts BIGINT CHECK (heavy_producer_accounts >= 0),
    ordinary_producer_accounts BIGINT CHECK (ordinary_producer_accounts >= 0),
    income_monthly_card_gold_per_card NUMERIC(38,12)
        CHECK (income_monthly_card_gold_per_card > 0),
    income_monthly_card_usd_reference NUMERIC(24,12)
        CHECK (income_monthly_card_usd_reference > 0),
    income_price_reference_version TEXT,
    income_source_evidence TEXT,
    PRIMARY KEY (realm_id, business_day, metric_revision),
    FOREIGN KEY (realm_id, business_day)
        REFERENCES hunting_economy_settlements(realm_id, business_day),
    FOREIGN KEY (realm_id, settings_revision)
        REFERENCES hunting_economy_observation_settings(realm_id, settings_revision),
    CHECK (NOT coverage_complete OR (
        coverage_scope = 'all_realm_assets'
        AND coverage_evidence IS NOT NULL AND length(btrim(coverage_evidence)) > 0
        AND financial_period_closed
        AND source_period_end IS NOT NULL
        AND source_ledger_watermark IS NOT NULL AND length(btrim(source_ledger_watermark)) > 0
        AND bound_gold_stock IS NOT NULL AND unbound_gold_stock IS NOT NULL
        AND settings_revision IS NOT NULL
    )),
    CHECK (NOT market_data_complete OR (
        monthly_card_completed_gold IS NOT NULL AND monthly_card_completed_quantity IS NOT NULL
        AND market_source_evidence IS NOT NULL AND length(btrim(market_source_evidence)) > 0
        AND (monthly_card_completed_quantity > 0 OR monthly_card_completed_gold = 0)
    )),
    CHECK (NOT material_data_complete OR (
        raw_material_price_index IS NOT NULL AND material_priced_weight_fraction IS NOT NULL
        AND material_priced_weight_fraction = 1
        AND material_index_version IS NOT NULL AND length(btrim(material_index_version)) > 0
    )),
    CHECK (NOT portrait_data_complete OR (
        active_casual_accounts IS NOT NULL AND active_light_pvp_accounts IS NOT NULL
        AND active_sabuk_accounts IS NOT NULL AND active_account_total IS NOT NULL
        AND active_account_total::numeric = active_casual_accounts::numeric
            + active_light_pvp_accounts::numeric + active_sabuk_accounts::numeric
        AND portrait_policy_version IS NOT NULL AND length(btrim(portrait_policy_version)) > 0
    )),
    CHECK (NOT income_data_complete OR (
        income_month IS NOT NULL
        AND income_source_evidence IS NOT NULL AND length(btrim(income_source_evidence)) > 0
    ))
);
CREATE INDEX IF NOT EXISTS hunting_economy_daily_metric_latest_idx
    ON hunting_economy_daily_metric_versions(realm_id, business_day, metric_revision DESC);

DROP TRIGGER IF EXISTS hunting_economy_observation_settings_immutable ON hunting_economy_observation_settings;
CREATE TRIGGER hunting_economy_observation_settings_immutable
    BEFORE UPDATE OR DELETE ON hunting_economy_observation_settings
    FOR EACH ROW EXECUTE FUNCTION hunting_economy_reject_history_mutation();
DROP TRIGGER IF EXISTS hunting_economy_daily_metrics_immutable ON hunting_economy_daily_metric_versions;
CREATE TRIGGER hunting_economy_daily_metrics_immutable
    BEFORE UPDATE OR DELETE ON hunting_economy_daily_metric_versions
    FOR EACH ROW EXECUTE FUNCTION hunting_economy_reject_history_mutation();

CREATE OR REPLACE VIEW hunting_permit_observability_realms AS
SELECT pool.realm_id,
    (policy.policy_json -> 'enabled' = 'true'::jsonb) IS TRUE AS enabled
FROM hunting_economy_pools pool
JOIN hunting_economy_policies policy
    ON policy.realm_id = pool.realm_id AND policy.policy_version = pool.policy_version;

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
        - (s.receipt_json ->> 'net_unbound_gold_inflow')::numeric AS conversion_fee_gold,
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
    s.committed_at_ms AS source_settlement_committed_at_ms
FROM hunting_economy_settlements s
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
    'Read-only latest metric revision linked to immutable settlement. is_complete requires explicit qualified full-realm asset coverage; new-wallet sums never establish global completeness.';
COMMENT ON VIEW hunting_permit_observability_realms IS
    'Configured realms, including those with no daily observations. enabled reads the current immutable policy JSON boolean.';

REVOKE ALL ON hunting_economy_observation_settings, hunting_economy_daily_metric_versions FROM PUBLIC;
REVOKE ALL ON hunting_permit_observability_realms, hunting_permit_daily_observations FROM PUBLIC;
