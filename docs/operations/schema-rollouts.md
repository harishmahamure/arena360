# Staged tenant schema rollout

Replication-enabled cells execute only enrolled rollout members. There is no
automatic fleet-wide migration merely because a new backend starts. Deploy
application code compatible with both schema versions, then enroll canaries.

```sh
cargo run -p arena360-tools --bin schema_rollout -- \
  --canary TENANT_UUID,TENANT_UUID --soak-seconds 300
cargo run -p arena360-tools --bin schema_rollout -- \
  --status ROLLOUT_UUID
cargo run -p arena360-tools --bin schema_rollout -- \
  --resume ROLLOUT_UUID
```

Commands use `CONTROL_DATABASE_URL` and the binary's latest embedded tenant
schema version. One durable rollout is allowed per version. Choose internal
or otherwise suitable canary tenants on cells running that binary. The initial
cohort is frozen; new provisioning and cold-tenant recovery install the current
schema through their existing paths.

Stages are **canary → 1% → 10% → 25% → 100%**, with cumulative membership.
Each percentage is rounded up and includes at least all canaries. Stable hashed
tenant ordering selects the remaining cohort. Every admitted member in a stage
must succeed before the soak timer starts. Each stage has its own soak interval;
small fleets can have stages with no additional tenants. Zero seconds is
supported for local tests; use an observation interval appropriate to the
release in staging and production.

Cells poll every ten seconds, run one migration at a time, and use P6 admission.
Only currently owned ACTIVE tenants with fresh leases are admitted. A
PostgreSQL advisory lock prevents duplicate execution across processes. The
writer gate and verified pre/post migration backups use the existing migration
runner. Success atomically records the tenant schema version and rollout result.

A migration or backup hook failure records the member error and globally sets
the rollout to `HALTED`. New admission checks the same durable rollout row.
Already admitted work may finish; running work is not preempted. No subsequent
stage opens while a member remains incomplete. Process restart can retry an
interrupted member after its advisory lock is released. SQLx migration history
reconciles a schema already applied before a control-plane completion failure.

Inspect status and the failed member's error before resuming. `--resume` retries
failed members within the current stage and restarts its observation interval;
it never moves the rollout forward directly. Contract migrations and large
backfills still need the expand/backfill/switch/contract release discipline.
The original low-level migration state remains available for provisioning and
isolated tests; the production scheduler uses rollout admission.

## Local verification

An isolated 100-tenant control test checks all five cumulative cohorts,
duplicate admission locking, durable halt/resume and stage observation timing.
Its canary uses a real SQLite database and one background slot: an injected
runner hook failure persists the halt, and explicit resume migrates the database
to the current version. Existing 50-tenant migration and 16 replication checks
also pass, including verified pre/post migration snapshots. These are functional
checks; release-specific canary health still requires staging observation.
