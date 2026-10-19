-- Runs only inside the acceptance runner's single rollback transaction.
-- Prefix comes from set_config in that transaction, never a production realm.
CREATE TEMP TABLE hunting_observer_source_witness AS
SELECT s.realm_id, s.business_day, md5(s.receipt_json::text) AS receipt_digest,
    (date '1970-01-01' + s.business_day::integer + 1 + time '04:00')
        AT TIME ZONE 'Asia/Shanghai' AS period_end
FROM hunting_economy_settlements s
WHERE s.realm_id = current_setting('hunting_observer_test.prefix') || '-steady_complete'
ORDER BY s.business_day DESC LIMIT 1;

SELECT pg_temp.hunting_observer_assert(
    ((t AT TIME ZONE 'Asia/Shanghai' - interval '4 hours')::date - 1) = expected,
    label)
FROM (VALUES
    ('2026-10-19T19:59:59Z'::timestamptz, date '2026-10-18', 'SQL-clock-before-0400'),
    ('2026-10-19T20:00:00Z'::timestamptz, date '2026-10-19', 'SQL-clock-at-0400'),
    ('2028-03-02T00:00:00Z'::timestamptz, date '2028-03-01', 'SQL-clock-leap-month')) AS x(t,expected,label);

-- Default metric publication cannot promote new-wallet receipt sums to global
-- realm assets. The correction is an append, leaving version1 in history.
INSERT INTO hunting_economy_daily_metric_versions(
    realm_id,business_day,metric_revision,metric_source,created_at_ms)
SELECT realm_id,business_day,2,'synthetic-default-partial',0
FROM hunting_observer_source_witness;
SELECT pg_temp.hunting_observer_assert(
    (SELECT metric_revision=2 AND is_complete IS FALSE AND bound_gold_stock IS NULL
        AND unbound_gold_stock IS NULL AND coverage_complete IS FALSE
        AND new_wallet_bound_gold_stock=123 AND new_wallet_unbound_gold_stock=456
     FROM hunting_permit_daily_observations o JOIN hunting_observer_source_witness w
        ON o.realm_id=w.realm_id AND o.economy_date=date '1970-01-01'+w.business_day::integer),
    'partial-metrics-never-promote-new-wallet-stock');

INSERT INTO hunting_economy_observation_settings(
    realm_id,settings_revision,policy_version,effective_from_day,
    unbound_stock_baseline_gold,burn_baseline_gold,planned_daily_consumption_atoms,created_at_ms)
SELECT w.realm_id,10000,s.policy_version,w.business_day,1000,100,1000,0
FROM hunting_observer_source_witness w JOIN hunting_economy_settlements s USING(realm_id,business_day);
INSERT INTO hunting_economy_daily_metric_versions(
    realm_id,business_day,metric_revision,settings_revision,metric_source,created_at_ms,
    financial_period_closed,source_period_end,source_ledger_watermark,coverage_complete,
    coverage_scope,coverage_evidence,bound_gold_stock,unbound_gold_stock)
SELECT realm_id,business_day,3,10000,'synthetic-unqualified-coverage',0,true,period_end,
    'synthetic-watermark',true,'all_realm_assets','synthetic-explicit-claim',2000,3000
FROM hunting_observer_source_witness;
SELECT pg_temp.hunting_observer_assert(
    (SELECT is_complete IS FALSE AND bound_gold_stock IS NULL AND unbound_gold_stock IS NULL
     FROM hunting_permit_daily_observations o JOIN hunting_observer_source_witness w
        ON o.realm_id=w.realm_id AND o.economy_date=date '1970-01-01'+w.business_day::integer),
    'coverage-claim-requires-separate-trusted-qualification');

-- No baseline is inferred when the approved settings leave baseline fields NULL.
INSERT INTO hunting_economy_observation_settings(
    realm_id,settings_revision,policy_version,effective_from_day,
    full_realm_coverage_qualified,coverage_qualification_evidence,
    coverage_qualified_by,coverage_qualified_at_ms,created_at_ms)
SELECT w.realm_id,10001,s.policy_version,w.business_day,true,'synthetic-qualification',
    'synthetic-trusted-aggregator',0,0
FROM hunting_observer_source_witness w JOIN hunting_economy_settlements s USING(realm_id,business_day);
INSERT INTO hunting_economy_daily_metric_versions(
    realm_id,business_day,metric_revision,settings_revision,metric_source,created_at_ms,
    financial_period_closed,source_period_end,source_ledger_watermark,coverage_complete,
    coverage_scope,coverage_evidence,bound_gold_stock,unbound_gold_stock,
    portrait_data_complete,active_casual_accounts,active_light_pvp_accounts,
    active_sabuk_accounts,active_account_total,portrait_policy_version)
SELECT realm_id,business_day,4,10001,'synthetic-qualified-coverage',0,true,period_end,
    'synthetic-watermark',true,'all_realm_assets','synthetic-explicit-coverage',2000,3000,
    true,6,3,1,10,'synthetic-portrait-v1'
FROM hunting_observer_source_witness;
SELECT pg_temp.hunting_observer_assert(
    (SELECT metric_revision=4 AND is_complete AND bound_gold_stock=2000 AND unbound_gold_stock=3000
        AND unbound_stock_baseline_gold IS NULL AND burn_baseline_gold IS NULL
        AND planned_daily_consumption_atoms IS NULL
        AND monthly_card_gold_per_card IS NULL AND raw_material_price_index IS NULL
        AND monthly_card_completed_quantity IS NULL AND heavy_producer_monthly_usd_equivalent IS NULL
     FROM hunting_permit_daily_observations o JOIN hunting_observer_source_witness w
        ON o.realm_id=w.realm_id AND o.economy_date=date '1970-01-01'+w.business_day::integer),
    'qualified-full-stock-is-explicit-and-unconnected-metrics-remain-null');

-- Future aggregate publication stores actual totals/coverage metadata, and the
-- view computes VWAP and an explicitly monthly conversion basis for USD value.
INSERT INTO hunting_economy_daily_metric_versions(
    realm_id,business_day,metric_revision,settings_revision,metric_source,created_at_ms,
    financial_period_closed,source_period_end,source_ledger_watermark,coverage_complete,
    coverage_scope,coverage_evidence,bound_gold_stock,unbound_gold_stock,
    market_data_complete,monthly_card_completed_gold,monthly_card_completed_quantity,market_source_evidence,
    material_data_complete,raw_material_price_index,material_index_version,material_priced_weight_fraction,
    portrait_data_complete,active_casual_accounts,active_light_pvp_accounts,active_sabuk_accounts,active_account_total,portrait_policy_version,
    income_data_complete,income_month,heavy_producer_monthly_net_gold,ordinary_producer_monthly_net_gold,
    heavy_producer_accounts,ordinary_producer_accounts,income_monthly_card_gold_per_card,
    income_monthly_card_usd_reference,income_price_reference_version,income_source_evidence)
SELECT realm_id,business_day,5,10001,'synthetic-complete-market-portrait-income',0,true,period_end,
    'synthetic-watermark',true,'all_realm_assets','synthetic-explicit-coverage',2000,3000,
    true,12000000,4,'synthetic-valid-completed-trades',true,101,'synthetic-fixed-basket-v1',1,
    true,6,3,1,10,'synthetic-portrait-v1',true,
    date_trunc('month',(date '1970-01-01'+business_day::integer)::timestamp)::date,
    -10000,2000,2,8,500000,10,'synthetic-monthly-price-v1','synthetic-full-cost-and-population'
FROM hunting_observer_source_witness;
SELECT pg_temp.hunting_observer_assert(
    (SELECT metric_revision=5 AND monthly_card_gold_per_card=3000000
        AND raw_material_price_index=101 AND heavy_producer_monthly_net_gold=-10000
        AND heavy_producer_monthly_usd_equivalent=-0.2
        AND ordinary_producer_monthly_usd_equivalent=0.04
        AND active_account_total=10 AND heavy_producer_accounts=2 AND ordinary_producer_accounts=8
     FROM hunting_permit_daily_observations o JOIN hunting_observer_source_witness w
        ON o.realm_id=w.realm_id AND o.economy_date=date '1970-01-01'+w.business_day::integer),
    'real-shaped-market-and-income-version-keeps-negative-income-and-monthly-basis');
SELECT pg_temp.hunting_observer_assert(
    (SELECT count(*)=5 FROM hunting_economy_daily_metric_versions m
     JOIN hunting_observer_source_witness w USING(realm_id,business_day))
    AND (SELECT bool_and(md5(s.receipt_json::text)=w.receipt_digest)
         FROM hunting_economy_settlements s JOIN hunting_observer_source_witness w USING(realm_id,business_day)),
    'metric-revisions-retained-without-changing-source-settlement');
SELECT pg_temp.hunting_observer_assert(
    NOT EXISTS(SELECT 1 FROM hunting_permit_daily_observations o
        WHERE o.realm_id LIKE current_setting('hunting_observer_test.prefix') || '%'
        AND o.gross_converted_gold IS DISTINCT FROM o.conversion_fee_gold+o.net_unbound_inflow_gold),
    'fee-derived-from-authoritative-gross-and-net');

-- New, disabled realm is still discoverable but excluded by enabled filter.
INSERT INTO hunting_economy_policies(realm_id,policy_version,policy_digest,policy_json,created_at_ms)
SELECT current_setting('hunting_observer_test.prefix')||'-disabled','synthetic-disabled',repeat('0',64),
    jsonb_build_object('policy_version','synthetic-disabled','enabled',false),0;
INSERT INTO hunting_economy_pools(realm_id,policy_version,genesis_day,settled_through_day,updated_at_ms)
SELECT current_setting('hunting_observer_test.prefix')||'-disabled','synthetic-disabled',0,0,0;
SELECT pg_temp.hunting_observer_assert(
    (SELECT enabled IS FALSE FROM hunting_permit_observability_realms
     WHERE realm_id=current_setting('hunting_observer_test.prefix')||'-disabled'),
    'realm-enabled-reads-current-policy-json');

-- Physical integrity constraints and immutable history are tested by intentional
-- rejected writes to our own synthetic rows only. No DELETE or cleanup mutation.
DO $$
DECLARE rejected boolean; witness record; cfg record;
BEGIN
    SELECT * INTO witness FROM hunting_observer_source_witness;
    SELECT * INTO cfg FROM hunting_economy_observation_settings
        WHERE realm_id=witness.realm_id AND settings_revision=10001;
    rejected := false;
    BEGIN
        INSERT INTO hunting_economy_settlements SELECT * FROM hunting_economy_settlements
        WHERE realm_id=witness.realm_id AND business_day=witness.business_day;
    EXCEPTION WHEN unique_violation THEN rejected := true; END;
    PERFORM pg_temp.hunting_observer_assert(rejected,'duplicate-source-day-rejected');

    rejected := false;
    BEGIN
        INSERT INTO hunting_economy_observation_settings(realm_id,settings_revision,policy_version,
            effective_from_day,unbound_stock_baseline_gold,created_at_ms)
        VALUES(witness.realm_id,20001,cfg.policy_version,witness.business_day,0,0);
    EXCEPTION WHEN check_violation THEN rejected := true; END;
    PERFORM pg_temp.hunting_observer_assert(rejected,'zero-stock-baseline-rejected');

    rejected := false;
    BEGIN
        INSERT INTO hunting_economy_observation_settings(realm_id,settings_revision,policy_version,
            effective_from_day,planned_daily_consumption_atoms,created_at_ms)
        VALUES(witness.realm_id,20002,cfg.policy_version,witness.business_day,0,0);
    EXCEPTION WHEN check_violation THEN rejected := true; END;
    PERFORM pg_temp.hunting_observer_assert(rejected,'zero-planned-demand-rejected');

    rejected := false;
    BEGIN
        UPDATE hunting_economy_observation_settings SET burn_baseline_gold=1
        WHERE realm_id=witness.realm_id AND settings_revision=10001;
    EXCEPTION WHEN raise_exception THEN
        IF SQLERRM='hunting economy history is immutable' THEN rejected:=true; ELSE RAISE; END IF;
    END;
    PERFORM pg_temp.hunting_observer_assert(rejected,'settings-history-mutation-rejected');

    rejected := false;
    BEGIN
        UPDATE hunting_economy_daily_metric_versions SET metric_source='changed'
        WHERE realm_id=witness.realm_id AND business_day=witness.business_day AND metric_revision=1;
    EXCEPTION WHEN raise_exception THEN
        IF SQLERRM='hunting economy history is immutable' THEN rejected:=true; ELSE RAISE; END IF;
    END;
    PERFORM pg_temp.hunting_observer_assert(rejected,'metric-history-mutation-rejected');

    rejected := false;
    BEGIN
        INSERT INTO hunting_economy_daily_metric_versions(realm_id,business_day,metric_revision,metric_source,created_at_ms,
            portrait_data_complete,active_casual_accounts,active_light_pvp_accounts,active_sabuk_accounts,
            active_account_total,portrait_policy_version)
        VALUES(witness.realm_id,witness.business_day,20003,'synthetic-overlap',0,true,6,3,1,9,'test-v1');
    EXCEPTION WHEN check_violation THEN rejected:=true; END;
    PERFORM pg_temp.hunting_observer_assert(rejected,'claimed-complete-overlapping-portraits-rejected');

    rejected := false;
    BEGIN
        INSERT INTO hunting_economy_daily_metric_versions(realm_id,business_day,metric_revision,metric_source,created_at_ms,
            material_data_complete,raw_material_price_index,material_index_version)
        VALUES(witness.realm_id,witness.business_day,20004,'synthetic-missing-basket-coverage',0,true,100,'test-v1');
    EXCEPTION WHEN check_violation THEN rejected:=true; END;
    PERFORM pg_temp.hunting_observer_assert(rejected,'claimed-complete-material-index-requires-full-fixed-basket');
END $$;
