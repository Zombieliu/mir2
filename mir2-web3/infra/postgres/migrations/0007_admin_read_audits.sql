-- Read-only operations are audited without creating a synthetic command.
-- Command-backed audit rows continue to reference admin_commands; PostgreSQL
-- foreign keys permit NULL for independent read audit records.
ALTER TABLE admin_audit_records
    ALTER COLUMN command_id DROP NOT NULL;
