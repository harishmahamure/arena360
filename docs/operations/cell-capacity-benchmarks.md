# Cell capacity benchmarks

`pnpm bench:cell-capacity` runs the M0 venue-day workload concurrently across
dedicated disposable tenants. It verifies their control-plane ownership before
and after every wave, checks the tenant selected by each login, and starts
traffic only after every participant has completed setup.

Use a release backend, the target hardware profile, and the normal replication,
outbox and analytics configuration. Keep the benchmark client off the measured
cell where possible. Create dedicated admin memberships for the fixture tenants;
the harness creates synthetic staff, players, PCs and financial transactions.

## Profile

Create a profile file, replacing every example value with the actual environment
and acceptance targets. The example latency and duration are not product SLOs:

```json
{
  "disposable": true,
  "backendBuild": "release",
  "backendRevision": "YOUR_DEPLOYED_REVISION",
  "cellId": "00000000-0000-0000-0000-000000000001",
  "hardwareProfile": "YOUR_CPU_RAM_STORAGE_OS_PROFILE",
  "apiUrl": "http://YOUR_CELL:3000",
  "maxP95Ms": 100,
  "minimumMeasuredSeconds": 60,
  "repetitions": 3,
  "maxWaves": 100,
  "sessions": 8,
  "sales": 6,
  "stages": [1, 2],
  "tenants": [
    {
      "tenantId": "00000000-0000-0000-0000-000000000002",
      "pcs": 20,
      "adminUsernameEnv": "BENCH_USER_ONE",
      "adminPasswordEnv": "BENCH_PASSWORD_ONE"
    },
    {
      "tenantId": "00000000-0000-0000-0000-000000000003",
      "pcs": 40,
      "adminUsernameEnv": "BENCH_USER_TWO",
      "adminPasswordEnv": "BENCH_PASSWORD_TWO"
    }
  ]
}
```

Set the named credential environment variables and `CONTROL_DATABASE_URL`;
do not put passwords in the profile. Enable legacy REST for this harness.

```sh
pnpm bench:cell-capacity --profile profile.json --json capacity-result.json
pnpm bench:test
```

Each stage repeats synchronized waves until both the repetition and measured
traffic-duration requirements are met. Setup is excluded from measured traffic
time. A latency or operation failure stops increasing load. A bounded wave limit
prevents an incomplete run from being recorded as passing. Interrupting the
runner stops its workload children.

## Interpreting evidence

The artifact records each tenant's operation percentiles, the worst tenant
operation p95, measured write TPS and read QPS, workload size, profile and
declared backend revision/build mode. Credentials and tokens are excluded.
One weighted venue is twenty PCs under the configured workload; this is an
observed benchmark unit, not a universal equivalence between customer tenants.

`observedWeightedVenues` is the largest passing tested stage. If the highest
tested stage passes, no capacity ceiling has been established. The runner does
not extrapolate beyond tested loads or turn a laptop result into a production
profile. Capture CPU/RAM/disk/WAL, outbox and replication lag, analytics load and
recovery measurements alongside this artifact before choosing rebalancer
resource budgets. Build mode/revision and hardware are explicit declarations;
verify them against the deployed binary and infrastructure record.

## Current status

Harness checks pass for exact sales distribution, profile validation and latency/
failure/throughput evaluation. Staging cells, production hardware and acceptance
targets have not been supplied, so TEST-0050's actual capacity measurements and
the M9 production launch gate remain pending.
