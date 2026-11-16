-- Explicit, independently reviewed launch evidence. No defaults or campaigns.
CREATE TABLE IF NOT EXISTS promotion_production_approvals (
 environment TEXT NOT NULL,
 campaign_id TEXT NOT NULL,
 configuration_sha256 TEXT NOT NULL CHECK(configuration_sha256 ~ '^[0-9a-f]{64}$'),
 manifest_sha256 TEXT NOT NULL CHECK(manifest_sha256 ~ '^[0-9a-f]{64}$'),
 manifest_json JSONB NOT NULL,
 proposed_by TEXT NOT NULL REFERENCES accounts(account_id),
 proposed_at_ms BIGINT NOT NULL,
 approved_by TEXT REFERENCES accounts(account_id),
 approved_at_ms BIGINT,
 PRIMARY KEY(environment,campaign_id),
 FOREIGN KEY(environment,campaign_id) REFERENCES promotion_campaigns(environment,campaign_id),
 CHECK(approved_by IS NULL OR approved_by<>proposed_by),
 CHECK((approved_by IS NULL)=(approved_at_ms IS NULL))
);
CREATE OR REPLACE FUNCTION promotion_guard_production_approval() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
 IF TG_OP IN ('DELETE','TRUNCATE') THEN RAISE EXCEPTION 'promotionProductionApprovalImmutable'; END IF;
 IF OLD.environment IS DISTINCT FROM NEW.environment OR OLD.campaign_id IS DISTINCT FROM NEW.campaign_id
 OR OLD.configuration_sha256 IS DISTINCT FROM NEW.configuration_sha256 OR OLD.manifest_sha256 IS DISTINCT FROM NEW.manifest_sha256
 OR OLD.manifest_json IS DISTINCT FROM NEW.manifest_json OR OLD.proposed_by IS DISTINCT FROM NEW.proposed_by
 OR OLD.proposed_at_ms IS DISTINCT FROM NEW.proposed_at_ms OR OLD.approved_by IS NOT NULL
 OR NEW.approved_by IS NULL OR NEW.approved_at_ms IS NULL THEN RAISE EXCEPTION 'promotionProductionApprovalImmutable'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER promotion_production_approval_immutable BEFORE UPDATE OR DELETE ON promotion_production_approvals
 FOR EACH ROW EXECUTE FUNCTION promotion_guard_production_approval();
CREATE TRIGGER promotion_production_approval_no_truncate BEFORE TRUNCATE ON promotion_production_approvals
 FOR EACH STATEMENT EXECUTE FUNCTION promotion_guard_production_approval();
-- Replace the unconditional historical disable with an evidence gate.
DO $$ DECLARE c record; BEGIN
 FOR c IN SELECT conname FROM pg_constraint WHERE conrelid='promotion_campaigns'::regclass AND contype='c'
 AND pg_get_constraintdef(oid) ~ '^CHECK \(\(NOT payout_enabled\)\)$'
 LOOP EXECUTE format('ALTER TABLE promotion_campaigns DROP CONSTRAINT %I',c.conname); END LOOP;
END $$;
CREATE OR REPLACE FUNCTION promotion_guard_launch_gates() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
 IF (NEW.environment='production' AND (NEW.recruitment_enabled OR NEW.commission_enabled OR NEW.payout_enabled)) OR NEW.payout_enabled THEN
  IF NOT NEW.rules_approved OR NOT EXISTS(SELECT 1 FROM promotion_production_approvals a
   WHERE a.environment=NEW.environment AND a.campaign_id=NEW.campaign_id AND a.approved_by IS NOT NULL)
  THEN RAISE EXCEPTION 'promotionProductionApprovalRequired'; END IF;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER promotion_launch_gates BEFORE INSERT OR UPDATE ON promotion_campaigns
 FOR EACH ROW EXECUTE FUNCTION promotion_guard_launch_gates();
