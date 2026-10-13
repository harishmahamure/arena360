# Deployment

This guide covers the implemented split gateway/storage deployment and the repository's existing production workflows. For responsibilities and data flow, see [architecture](architecture.md). For local development, see [start](start.md) and [setup guidelines](setup-guidelines.md).

Run build commands from the repository root. Replace example image names, hosts, namespace, and cell UUID with the values for your environment. The commands below are instructions; creating this guide does not deploy anything.

## Deployment units

| Unit | Image build | Helm chart | Storage |
| --- | --- | --- | --- |
| Gateway | `infra/docker/rust.Dockerfile`, target `gateway` | `infra/helm/tenant-services` | Stateless |
| Storage | Same Dockerfile, target `storage` | `infra/helm/tenant-services` | Persistent tenant PVC and optional separate backup-key PVC |
| Platform portal | `apps/tenant/Dockerfile` | `infra/helm/platform-portal` | Static Nginx frontend |
| Staff panel | `apps/admin/Dockerfile` | `infra/helm/frontend` | Static Nginx frontend |
| Combined backend | Rust Dockerfile, target `backend` | `infra/helm/backend` | Persistent tenant storage |

PostgreSQL must be reachable from both Rust services. Default split-service images use SQLite reports and need no DuckDB SDK. The PostgreSQL chart is under `infra/helm/` if you host the control database in the cluster.

## 1. Build and publish images

Use versioned tags and build for the cluster's CPU architecture. The helper uses the local Docker builder; use an architecture-compatible runner or an explicit Docker Buildx platform when the build machine and cluster differ.

```sh
pnpm rust:image --target gateway --tag REGISTRY/arena360-gateway:VERSION
pnpm rust:image --target storage --tag REGISTRY/arena360-storage:VERSION
docker build -f apps/tenant/Dockerfile -t REGISTRY/arena360-portal:VERSION .
docker build -f apps/admin/Dockerfile -t REGISTRY/arena360-staff:VERSION .

docker push REGISTRY/arena360-gateway:VERSION
docker push REGISTRY/arena360-storage:VERSION
docker push REGISTRY/arena360-portal:VERSION
docker push REGISTRY/arena360-staff:VERSION
```

The Rust helper sends an allowlisted build context that excludes tenant data and environment secrets. Default Rust runtime images run as UID/GID 10001. The storage image includes the operator binaries, including `platform_operator`, `schema_rollout`, `tenant_recover`, and `tenant_move`.

The staff image's default build uses API/WebSocket placeholders that its startup script fills from runtime environment variables. Images built with explicit `VITE_API_URL` or `VITE_GATEWAY_URL` build arguments retain those compiled URLs; Helm values do not replace already compiled concrete URLs.

## 2. Prepare service configuration

Create the namespace and a protected environment file outside the repository, such as `/secure/arena360-services.env`. Put these settings in that file:

```dotenv
CONTROL_DATABASE_URL=postgres://USER:PASSWORD@POSTGRES_HOST:5432/arena360_control
JWT_SECRET=REPLACE_WITH_A_SECRET_OF_AT_LEAST_32_CHARACTERS
STORAGE_SERVICE_TOKEN=REPLACE_WITH_A_SEPARATE_SECRET_OF_AT_LEAST_32_ASCII_CHARACTERS
LEGACY_REST_ENABLED=true
```

Optional settings include mail/media storage credentials, and replication settings. The chart sets `PORT`, `ARENA_CELL_ID`, `TENANT_DATA_DIR`, `STORAGE_CELL_ID`, and `STORAGE_SERVICE_URL` on the appropriate deployments. Both services receive the shared Secret; they must use the same JWT and private service-token values.

```sh
kubectl create namespace arena360
kubectl create secret generic arena360-tenant-services   --namespace arena360   --from-env-file /secure/arena360-services.env   --dry-run=client -o yaml | kubectl apply -f -
```

For a private registry, create the image-pull credential in this namespace. The tenant-services chart has no `imagePullSecrets` value; supply credentials through its namespace service account or your cluster's admission configuration. The frontend charts expose `imagePullSecrets` and default to `dockerhub-secret`.

## 3. Install gateway and storage

```sh
helm upgrade --install arena360 ./infra/helm/tenant-services   --namespace arena360   --set-string existingSecret=arena360-tenant-services   --set-string gateway.image=REGISTRY/arena360-gateway:VERSION   --set-string storage.image=REGISTRY/arena360-storage:VERSION   --set-string storage.cellId=00000000-0000-4000-8000-000000000001   --wait --timeout 10m
```

This release creates `arena360-gateway:3000`, `arena360-storage:3001`, and a retained `arena360-tenants` PVC. Gateway defaults to two replicas. Storage runs one replica with a Recreate strategy and mounts `/data/tenants`.

Set `storage.persistence.storageClass` for a specific StorageClass, or `storage.persistence.existingClaim` to reuse a tenant PVC. Keep the original cell UUID when reusing its data. The storage pod's security context uses UID/GID and fsGroup 10001. An uninstall retains the chart-created tenant PVC; a separate PVC lifecycle still needs your storage provider's retention policy.

The chart has no gateway ingress template. Configure your public API ingress to forward to `arena360-gateway:3000`, with HTTPS and WebSocket support. Keep `arena360-storage:3001` internal. Private gateway-to-storage traffic requires native HTTP/2 gRPC; an HTTP/1-only intermediary cannot serve that link. The storage Service declares `appProtocol: kubernetes.io/h2c`.

## 4. Install the frontends

Deploy the portal with the split gateway upstream explicitly configured:

```sh
helm upgrade --install arena360-platform-portal ./infra/helm/platform-portal   --namespace arena360   --set-string image.repository=REGISTRY/arena360-portal   --set-string image.tag=VERSION   --set-string platformApiUpstream=http://arena360-gateway:3000   --set ingress.enabled=true   --set-string ingress.host=platform.example.com   --set-string ingress.tlsSecretName=platform-tls   --wait --timeout 5m
```

Provision DNS and the TLS Secret for that host, and use the appropriate ingress class. The portal chart defaults to the combined backend upstream `http://arena360-backend:3000`, so the override above is required for the split release.

Deploy the staff panel with the public gateway URLs:

```sh
helm upgrade --install arena360-staff ./infra/helm/frontend   --namespace arena360   --values ./infra/helm/frontend/values-prod.yaml   --set-string image.repository=REGISTRY/arena360-staff   --set-string image.tag=VERSION   --set-string 'ingress.hosts[0].host=staff.example.com'   --set-string 'ingress.tls[0].hosts[0]=staff.example.com'   --set-string 'ingress.tls[0].secretName=staff-tls'   --set-string env.VITE_API_URL=https://api.example.com   --set-string env.VITE_GATEWAY_URL=wss://api.example.com/realtime   --wait --timeout 5m
```

The production values include an Nginx ingress class and a cert-manager issuer annotation. Adjust them for your cluster. The API host must reach the gateway and its `/realtime` WebSocket endpoint.

## 5. Bootstrap and verify onboarding

If no portal operator exists, create one with a password supplied on stdin:

```sh
python3 -c 'import getpass; print(getpass.getpass("Portal password: "))' | kubectl exec -i --namespace arena360 deployment/arena360-storage -- platform_operator platform.admin
```

The operator CLI requires a 12–72 byte password, applies control migrations, and creates a new username. Existing operators can sign in without rerunning it. Complete authenticator enrollment in the portal.

Register the storage worker in the portal with the configured cell UUID and reachable address `http://arena360-storage:3001`. Confirm it is active and the Trial plan is active. Create a tenant, create its business administrator, and verify venue setup and default roles/units from the staff panel.

Control migrations apply on startup. New tenant provisioning installs current tenant migrations before activation. Existing tenants use the lease-fenced rollout tooling; application rollout does not replace that schema process.

## 6. Health checks and diagnostics

```sh
kubectl rollout status deployment/arena360-gateway --namespace arena360
kubectl rollout status deployment/arena360-storage --namespace arena360
kubectl get pods,svc,pvc --namespace arena360
kubectl logs deployment/arena360-gateway --namespace arena360 --tail=100
kubectl logs deployment/arena360-storage --namespace arena360 --tail=100
kubectl port-forward --namespace arena360 service/arena360-gateway 3000:3000
```

While port forwarding, check `http://localhost:3000/health/ready`. The Rust charts use `/health/ready` and `/health/live`; the portal uses `/ready` and `/health`. Storage's startup probe allows recovery work before normal readiness checks.

A healthy process still needs reachable storage, an active registered worker, and an active plan for onboarding. A worker's backup/hydration readiness is separate from ordinary tenant creation.

## Backups, updates, and scale

Configure `REPLICATION_BUCKET`, S3 endpoint/credentials, and a separate durable key PVC; set `storage.replicationKeyClaim` to mount it at `/run/arena360-backup-keys`. The chart then supplies `REPLICATION_KEY_DIR`. Preserve both tenant data and backup keys. Remote replication is disabled when required configuration is missing, and recovery/move workflows depend on their configured backup capability.

Update images by rerunning the Helm upgrade with new versioned tags. Before a schema-changing release, use the existing rollout and backup procedures implemented by the operator tools. For a compatible application-only rollback:

```sh
helm history arena360 --namespace arena360
helm rollback arena360 PREVIOUS_REVISION --namespace arena360 --wait --timeout 10m
```

Helm rollback restores Kubernetes release configuration, not database contents or schema. Confirm the prior binary supports the tenant and control schemas before rolling back.

Scale gateway through `gateway.replicas`. Add storage capacity through another release with `gateway.enabled=false`, a new `storage.cellId`, and its own PVC. Register that worker's reachable address before moving tenants. For gateway-only releases, set `storage.enabled=false`, `gateway.storageUrl`, and `storage.cellId` to the default provisioning worker.

## Existing GitHub workflows

| Workflow | What it deploys |
| --- | --- |
| [Backend](../.github/workflows/deploy-production-backend.yml) | Combined `backend` Rust image and `infra/helm/backend`; optional control migrations |
| [Platform portal](../.github/workflows/deploy-production-platform-portal.yml) | Portal image and separate portal chart |
| [Staff frontend](../.github/workflows/deploy-production-frontend.yml) | Staff image and frontend chart |
| [Database](../.github/workflows/deploy-production-database.yml) | PostgreSQL infrastructure |

These workflows use manual dispatch. Application build/deploy workflows use `DOCKERHUB_USERNAME`, `DOCKERHUB_TOKEN`, optional `DOCKERHUB_REPO`, and `KUBE_CONFIG`; backend also receives its configured service secrets. The portal workflow uses `PLATFORM_PORTAL_HOST` when set and otherwise the production host in its chart values.

There is currently no dedicated split-service deploy workflow. The manual split Helm commands above implement that path. When using the existing portal workflow with split services, configure its upstream/chart values to the gateway because its current production values target the combined backend.
