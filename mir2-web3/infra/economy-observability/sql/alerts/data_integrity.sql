WITH bounds AS (
  SELECT ((now() AT TIME ZONE 'Asia/Shanghai' - interval '4 hours')::date - 1) AS closed_date
), calendar AS (
  SELECT r.realm_id, d::date AS economy_date
  FROM public.hunting_permit_observability_realms r CROSS JOIN bounds b
  CROSS JOIN LATERAL generate_series(b.closed_date - (10 - 1), b.closed_date, interval '1 day') d
  WHERE r.enabled IS TRUE
)
SELECT now() AS time, c.realm_id,
  CASE WHEN count(o.economy_date) = 10
    AND bool_and(o.is_complete IS TRUE)
    AND bool_and((o.burn_daily_gold IS NOT NULL AND o.burn_daily_gold >= 0 AND
      o.burn_light_pvp_gold IS NOT NULL AND o.burn_light_pvp_gold >= 0 AND
      o.burn_sabuk_gold IS NOT NULL AND o.burn_sabuk_gold >= 0 AND
      o.permit_minted_atoms IS NOT NULL AND o.permit_minted_atoms >= 0 AND
      o.permit_consumed_atoms IS NOT NULL AND o.permit_consumed_atoms >= 0 AND
      o.permit_decayed_atoms IS NOT NULL AND o.permit_decayed_atoms >= 0 AND
      o.permit_closing_atoms IS NOT NULL AND o.permit_closing_atoms >= 0 AND
      o.bound_gold_stock IS NOT NULL AND o.bound_gold_stock >= 0 AND
      o.unbound_gold_stock IS NOT NULL AND o.unbound_gold_stock >= 0 AND
      o.gross_converted_gold IS NOT NULL AND o.gross_converted_gold >= 0 AND
      o.conversion_fee_gold IS NOT NULL AND o.conversion_fee_gold >= 0 AND
      o.net_unbound_inflow_gold IS NOT NULL AND o.net_unbound_inflow_gold >= 0 AND
      o.gold_conversion_fee_gold IS NOT NULL AND o.gold_conversion_fee_gold >= 0 AND
      o.material_conversion_gold_equivalent IS NOT NULL AND o.material_conversion_gold_equivalent >= 0 AND
      o.material_conversion_fee_unbound_gold IS NOT NULL AND o.material_conversion_fee_unbound_gold >= 0 AND
      o.unbound_stock_baseline_gold > 0 AND o.burn_baseline_gold > 0
      AND o.planned_daily_consumption_atoms > 0
      AND o.policy_version IS NOT NULL AND length(o.policy_version) > 0
      AND o.period_end = ((o.economy_date + 1) + time '04:00') AT TIME ZONE 'Asia/Shanghai'
      AND o.gross_converted_gold::numeric = o.gold_conversion_fee_gold::numeric + o.net_unbound_inflow_gold::numeric
      AND o.conversion_fee_gold::numeric = o.gold_conversion_fee_gold::numeric + o.material_conversion_fee_unbound_gold::numeric
      AND o.permit_consumed_atoms::numeric = o.gross_converted_gold::numeric + o.material_conversion_gold_equivalent::numeric
      AND (o.material_conversion_gold_equivalent = 0) = (o.material_conversion_fee_unbound_gold = 0)
      AND o.material_conversion_fee_unbound_gold <= o.material_conversion_gold_equivalent
      AND o.conversion_fee_gold <= o.burn_daily_gold
      AND o.active_casual_accounts IS NOT NULL AND o.active_casual_accounts >= 0
      AND o.active_light_pvp_accounts IS NOT NULL AND o.active_light_pvp_accounts >= 0
      AND o.active_sabuk_accounts IS NOT NULL AND o.active_sabuk_accounts >= 0
      AND o.active_account_total = o.active_casual_accounts + o.active_light_pvp_accounts + o.active_sabuk_accounts) IS TRUE)
  THEN 0.0 ELSE 1.0 END AS value
FROM calendar c LEFT JOIN public.hunting_permit_daily_observations o
  ON o.realm_id = c.realm_id AND o.economy_date = c.economy_date
GROUP BY c.realm_id ORDER BY c.realm_id;
