# Start

Use this guide after completing [setup guidelines](setup-guidelines.md). Run every command from the repository root. If the services are already running, open the URLs below.

## 1. Start local infrastructure

With Docker running:

```sh
docker compose up -d postgres postgres-init redis
```

For Colima on macOS, start the VM first:

```sh
colima start
```

If Docker Compose is unavailable and this workspace's containers already exist, start them directly:

```sh
docker start arena360-postgres-1 arena360-redis-1
```

That fallback requires the existing `arena360_control` database. Follow the setup guide for a fresh environment.

## 2. Start the services

Use a separate terminal for each process. Both Rust services read the root `.env`.

Storage:

```sh
PORT=3001 pnpm storage:dev
```

Gateway:

```sh
PORT=3000 pnpm gateway:dev
```

Platform portal:

```sh
pnpm portal:dev
```

Staff panel:

```sh
pnpm admin:dev
```

| Application | URL |
| --- | --- |
| Platform portal | http://localhost:5174 |
| Staff panel | http://localhost:5173 |
| Gateway health | http://localhost:3000/health |
| Storage health | http://localhost:3001/health |

To run the station application when needed:

```sh
pnpm kiosk:dev
```

## 3. Test tenant creation

1. Open the platform portal and sign in with your portal operator. On first login, enroll an authenticator and verify its six-digit code.
2. Open **Tenants** and check **Tenant creation checklist**. Provisioning, an active database worker, and the active **Trial** plan must be available.
3. If needed, register the worker with the ID configured in `ARENA_CELL_ID` / `STORAGE_CELL_ID` and address `http://localhost:3001`.
4. Create a tenant with its business name, timezone, and trial duration. Confirm creation succeeds.
5. Create its business administrator with a unique username and a 12–72 byte password. Portal operators and business administrators are separate accounts.
6. Open the staff panel and sign in as that business administrator. Add a venue, then its devices, staff, and products.
7. Check **Access management**: Administrator, Counter operator, and Location administrator roles; six templates; and location setup access. Check that the unit catalog contains eleven units.
8. Test sessions, sales, inventory, and reports as you add the data each workflow requires.

If administrator creation fails after tenant creation, retry administrator setup from tenant details. The saved tenant can be used to continue onboarding.

## 4. Stop

Use `Ctrl+C` in each application terminal. To stop local infrastructure without removing its saved data:

```sh
docker stop arena360-postgres-1 arena360-redis-1
```

For Colima, stop the VM after stopping the applications:

```sh
colima stop
```
