-- A later correction/backfill can produce another immutable monthly revision.
ALTER TABLE archive_manifests DROP CONSTRAINT archive_manifests_tenant_id_period_start_key;
ALTER TABLE archive_manifests ADD COLUMN revision BIGINT GENERATED ALWAYS AS IDENTITY;
ALTER TABLE archive_manifests ADD COLUMN superseded_at TIMESTAMPTZ;
ALTER TABLE archive_manifests ADD COLUMN oltp_p99_target_milliseconds BIGINT NOT NULL DEFAULT 25 CHECK(oltp_p99_target_milliseconds BETWEEN 1 AND 60000);
ALTER TABLE archive_manifests ADD COLUMN batch_rows INTEGER NOT NULL DEFAULT 100 CHECK(batch_rows BETWEEN 1 AND 1000);
ALTER TABLE archive_manifests ADD COLUMN retry_after TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp();
CREATE UNIQUE INDEX archive_one_running_month ON archive_manifests(tenant_id,period_start) WHERE state<>'COMPLETE' AND superseded_at IS NULL;
CREATE INDEX archive_verified_period ON archive_manifests(tenant_id,period_start,revision) WHERE verified_at IS NOT NULL;
-- Source evidence stays tied to its original capture. Execution follows current ownership.
CREATE INDEX archive_resume_queue ON archive_manifests(tenant_id,retry_after) WHERE state<>'COMPLETE' AND superseded_at IS NULL;
