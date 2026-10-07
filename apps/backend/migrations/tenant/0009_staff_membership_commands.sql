-- Tenant access edits request global activation through a durable local command.
-- Activation stays fail-closed until the control-plane commit is acknowledged.
CREATE TABLE staff_membership_commands (
    user_id TEXT PRIMARY KEY REFERENCES users(id),
    desired_active INTEGER NOT NULL CHECK(desired_active IN(0,1)),
    identity_revision INTEGER NOT NULL CHECK(identity_revision>=0),
    access_revision INTEGER NOT NULL CHECK(access_revision>0)
) STRICT;
