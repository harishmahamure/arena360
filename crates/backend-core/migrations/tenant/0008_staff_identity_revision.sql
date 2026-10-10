-- Independent from access_revision and the exact grant projection's member_revision.
ALTER TABLE users ADD COLUMN identity_revision INTEGER NOT NULL DEFAULT 0 CHECK(identity_revision>=0);
