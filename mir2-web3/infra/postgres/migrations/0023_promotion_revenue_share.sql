-- User-approved alternative: commissions funded by actual season revenue,
-- without a prepaid campaign budget or artificial admission reservations.
ALTER TABLE promotion_campaigns ADD COLUMN IF NOT EXISTS funding_mode TEXT NOT NULL DEFAULT 'budget_capped'
 CHECK(funding_mode IN ('budget_capped','revenue_share'));
DO $$ DECLARE c record; BEGIN
 FOR c IN SELECT conname FROM pg_constraint WHERE conrelid='promotion_campaigns'::regclass AND contype='c'
 AND pg_get_constraintdef(oid) LIKE '%recruitment_enabled%'
 AND pg_get_constraintdef(oid) LIKE '%per_member_revenue_cap_micro_usd%'
 LOOP EXECUTE format('ALTER TABLE promotion_campaigns DROP CONSTRAINT %I',c.conname); END LOOP;
END $$;
ALTER TABLE promotion_campaigns ADD CONSTRAINT promotion_recruitment_funding CHECK (
 NOT recruitment_enabled OR (rules_approved AND status='active' AND (
  (funding_mode='budget_capped' AND budget_micro_usd>0 AND member_limit>0 AND leader_limit>0 AND per_leader_member_limit>0 AND per_member_revenue_cap_micro_usd>0)
  OR (funding_mode='revenue_share' AND budget_micro_usd=0 AND reserved_micro_usd=0)
 )));
