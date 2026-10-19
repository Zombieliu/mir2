WITH bounds AS (
  SELECT ((now() AT TIME ZONE 'Asia/Shanghai' - interval '4 hours')::date - 1) AS closed_date
), calendar AS (
  SELECT r.realm_id, d::date AS economy_date
  FROM public.hunting_permit_observability_realms r CROSS JOIN bounds b
  CROSS JOIN LATERAL generate_series(b.closed_date - (1 - 1), b.closed_date, interval '1 day') d
  WHERE r.enabled IS TRUE
)
SELECT now() AS time, c.realm_id,
  CASE WHEN count(o.economy_date) = 1
    AND bool_and(o.is_complete IS TRUE)
    AND bool_and((o.burn_daily_gold >= 0 AND o.burn_light_pvp_gold >= 0 AND o.burn_sabuk_gold >= 0 AND o.burn_baseline_gold > 0) IS TRUE)
  THEN CASE WHEN bool_and(((o.burn_daily_gold::numeric + o.burn_light_pvp_gold::numeric + o.burn_sabuk_gold::numeric) > o.burn_baseline_gold::numeric * 1.50) IS TRUE) THEN 1.0 ELSE 0.0 END
  ELSE NULL::double precision END AS value
FROM calendar c LEFT JOIN public.hunting_permit_daily_observations o
  ON o.realm_id = c.realm_id AND o.economy_date = c.economy_date
GROUP BY c.realm_id ORDER BY c.realm_id;
