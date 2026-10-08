-- -1 allows the first control projection to reconcile existing tenant metadata.
ALTER TABLE tenant_runtime ADD COLUMN timezone_revision INTEGER NOT NULL DEFAULT -1 CHECK(timezone_revision>=-1);
