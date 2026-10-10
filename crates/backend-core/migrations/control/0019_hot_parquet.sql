-- Rebuildable copies. These manifests never authorize a SQLite purge.
CREATE TABLE hot_month_manifests (
 tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
 period_start DATE NOT NULL CHECK(extract(day FROM period_start)=1),
 period_end DATE NOT NULL CHECK(period_end=(period_start+INTERVAL '1 month')::date),
 timezone TEXT NOT NULL,
 ownership_generation BIGINT NOT NULL CHECK(ownership_generation>0),
 source_watermark BIGINT NOT NULL CHECK(source_watermark>=0),
 projection_version BIGINT NOT NULL CHECK(projection_version>0),
 objects JSONB NOT NULL CHECK(jsonb_typeof(objects)='array'),
 state TEXT NOT NULL DEFAULT 'READY' CHECK(state IN ('READY','RETIRED')),
 verified_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 retired_at TIMESTAMPTZ,
 PRIMARY KEY(tenant_id,period_start),
 CHECK(state<>'RETIRED' OR retired_at IS NOT NULL)
);
CREATE TABLE hot_tenant_state (
 tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
 ownership_generation BIGINT NOT NULL CHECK(ownership_generation>0),
 source_watermark BIGINT NOT NULL CHECK(source_watermark>=0),
 timezone TEXT NOT NULL,
 hot_window_start DATE NOT NULL,
 next_due TIMESTAMPTZ NOT NULL,
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
