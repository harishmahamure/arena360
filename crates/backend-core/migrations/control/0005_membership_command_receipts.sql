CREATE TABLE staff_membership_command_receipts (
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID NOT NULL,
    access_revision BIGINT NOT NULL,
    ownership_generation BIGINT NOT NULL,
    control_revision BIGINT NOT NULL,
    conflicted BOOLEAN NOT NULL,
    PRIMARY KEY(tenant_id,user_id,access_revision,ownership_generation)
);
