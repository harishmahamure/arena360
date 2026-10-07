-- Control metadata cached during provisioning; business reads never consult PostgreSQL.
CREATE TABLE tenant_runtime (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    timezone TEXT NOT NULL CHECK (length(timezone) BETWEEN 1 AND 80)
) STRICT;
