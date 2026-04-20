# Level 2 Integration Tests — crAPI

Level 2 tests exercise `mitm2openapi` against [OWASP crAPI](https://github.com/OWASP/crAPI) — a deliberately vulnerable API with a realistic React SPA.

## Why crAPI

- Multi-arch pre-built images (`linux/amd64` + `linux/arm64`)
- Rich API surface: 44 endpoints across identity, community, and workshop services
- Official OpenAPI spec pinned at `OWASP/crAPI@940d1c4f`
- Auto-seeded with users, vehicles, and products — no manual seed step

## Services

| Service     | Port (host) | Purpose                      |
| ----------- | ----------- | ---------------------------- |
| `crapi-web` | 8888        | nginx+Lua gateway, React SPA |
| `mailhog`   | 8025        | email testing                |
| `mitmproxy` | 8080        | traffic capture              |

All API traffic routes through `crapi-web:80`. Path prefixes:

- `/identity/api/*` → crapi-identity (Java/Spring)
- `/community/api/*` → crapi-community (Go)
- `/workshop/api/*` → crapi-workshop (Python/Django)

## Seed Users

| Email                 | Password      | Role           |
| --------------------- | ------------- | -------------- |
| `admin@example.com`   | `Admin!123`   | ROLE_ADMIN     |
| `test@example.com`    | `Test!123`    | ROLE_USER      |
| `adam007@example.com` | `adam007!123` | ROLE_PREDEFINE |

No manual seed needed — crAPI auto-loads users on first boot.

## Running Locally

```bash
cd tests/integration/level2

# Start all services
make up

# Run Playwright scenarios
npm ci
npx playwright install --with-deps chromium
npx playwright test

# View captured traffic
ls -lh out/crapi.flow

# Teardown
make down
```

## Port Conflicts

crAPI and Level 1 (Petstore) both use port 8080 (mitmproxy). Do not run both simultaneously.

## Full Pipeline

```bash
./run-l2.sh
```

This runs: compose up → Playwright → discover → generate → normalize → oasdiff diff → compose down.
