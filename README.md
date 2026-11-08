# POS with BPOM RI & QR Product Integration

> Scan barcode/BPOM RI, the product goes into the POS automatically. Purchase and selling prices are yours to set.

A point-of-sale web application for Indonesian small businesses: minimarkets, grocery stores and shops.
The main idea is to remove most of the typing needed to add a product:

```
SCAN → IDENTIFY → REVIEW → SET PRICE → SELL
```

Scan a barcode or QR code. The app first checks your own catalog, then external product data sources.
If it finds something, the product form is pre-filled. You check the details, set the prices and stock, and save.
External data never sets your prices, stock, SKU or category. If no source has the product, or a source is down,
you can enter the product by hand and keep working.

## What works today

| Area | Status |
| --- | --- |
| Sign-in, sessions, store membership, roles (owner, manager, cashier) | Done |
| Barcode normalization: EAN-8, UPC-E, UPC-A, EAN-13, GTIN-14 with check digits | Done |
| QR parsing: GS1 Digital Link URLs, GS1 element strings, internal codes; links are never opened | Done |
| Product identification: local match first, then cache, then providers, with honest status values | Done (mock provider) |
| BPOM RI data source | Adapter only. Not connected. See [docs/integrations/bpom.md](docs/integrations/bpom.md) |
| Products: create, edit, deactivate, multiple barcodes, provenance | Done |
| Categories | Done |
| Inventory: stock on hand, adjustments with reasons, movement ledger, low-stock alerts | Done |
| Cashier (POS): scanner or search, cart, sale discount, cash/QRIS/card payment, change, receipt | Done |
| Checkout safety: server-side pricing, idempotent retries, atomic stock updates | Done |
| Transactions: history with date and receipt filters, receipt view and print | Done |
| Dashboard: sales today, 7-day trend, low stock, recent sales | Done |
| Store settings (owner): tax rate, time zone, negative-stock policy, receipt footer | Done |
| Multi-store switcher in the UI | Not yet (the API supports many stores per user) |
| Voiding sales, refunds, supplier purchase orders, customer accounts, reports export | Not yet (see [Roadmap](#roadmap)) |

## Architecture at a glance

```
Browser ──► Next.js (App Router, :3000) ──/api/v1──► Rust API (Axum, :8080) ──► PostgreSQL 16
              UI, cookies stay first-party       modular monolith:
                                                 auth · products · product_identification
                                                 inventory · sales · dashboard · settings
                                                        │
                                                        └──► external data providers (mock, BPOM adapter)
```

- `backend/` Rust (Axum, Tokio, SQLx, PostgreSQL). Business rules live in the domain modules, not in handlers.
- `frontend/` Next.js 16, React 19, TypeScript, Tailwind CSS 4, TanStack Query, React Hook Form with Zod, Zustand for the cart.
- `docs/` architecture, product identification rules, BPOM integration, OpenAPI description.

Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for module boundaries, the data model and the security model.

## Quick start with Docker

Requires Docker with Compose v2.

```bash
cp .env.example .env                  # set POSTGRES_PASSWORD
docker compose up --build             # app on http://localhost:3000
docker compose --profile seed run --rm seed   # optional demo data, once
```

Demo accounts after seeding (password from `SEED_PASSWORD`, default `ChangeMe123!`, development only):

| Email | Role |
| --- | --- |
| `owner@example.com` | owner |
| `manager@example.com` | manager |
| `kasir@example.com` | cashier |

## Local development without Docker

Requirements: Rust stable (1.85 or newer), Node.js 22, PostgreSQL 16.

```bash
# 1. Database (any PostgreSQL 16 instance). Example role and database:
createuser -P pos && createdb -O pos pos_dev

# 2. API. Migrations run automatically at startup.
cd backend
cp .env.example .env                                   # adjust DATABASE_URL
cargo run --bin seed                                   # demo store, users, products (idempotent)
cargo run --bin pos-backend                            # http://localhost:8080

# 3. Web app (in another terminal)
cd frontend
cp .env.example .env.local                             # BACKEND_URL=http://localhost:8080
npm ci
npm run dev                                            # http://localhost:3000
```

The browser only talks to the Next.js origin. `/api/v1/*` is proxied to `BACKEND_URL` by a route handler, so
the httpOnly session cookie is first-party and no CORS configuration is needed for the browser.

## Configuration

Backend (`backend/.env.example`), frontend (`frontend/.env.example`), and Compose (`.env.example`) document every
variable. The important ones:

| Variable | Where | Purpose |
| --- | --- | --- |
| `DATABASE_URL` | backend | PostgreSQL connection string (required) |
| `CORS_ALLOWED_ORIGINS` | backend | Origins allowed to call the API directly (the web app does not need this) |
| `COOKIE_SECURE` | backend | Set `true` behind HTTPS |
| `SESSION_TTL_HOURS` | backend | Session lifetime (default 12) |
| `PROVIDER_TIMEOUT_MS` | backend | Per-provider lookup timeout (default 4000) |
| `LOOKUP_CACHE_HIT_HOURS` / `LOOKUP_CACHE_MISS_MINUTES` | backend | Cache lifetime for found and not-found results |
| `BPOM_API_BASE_URL` | backend | Only set after an approved BPOM endpoint is available. Empty by default. |
| `BACKEND_URL` | frontend | Where the Next.js server sends `/api/v1` requests (read at runtime) |

Never commit `.env` files or real credentials. They are git-ignored.

## API

- Base path `/api/v1`, JSON, and a session cookie named `pos_session` (HttpOnly, SameSite=Lax).
- Send `X-Store-Id: <uuid>` to pick a store. Without it, the user's first store is used. Non-members get 403.
- Errors use one shape: `{"error": {"code": "...", "message": "...", "details": ...}}`.
- Lists return `{"data": [...], "pagination": {"limit", "offset", "total"}}`. Money is integer IDR.
- Full description: [docs/openapi.yaml](docs/openapi.yaml). Also see [docs/PRODUCT_IDENTIFICATION.md](docs/PRODUCT_IDENTIFICATION.md) for the status values returned by identification.

## Testing

```bash
# Backend: unit tests (no database) and integration tests (creates temporary databases)
cd backend
DATABASE_URL=postgres://pos:...@127.0.0.1:5432/pos_dev cargo test

# Frontend: unit and component tests, typecheck, production build
cd frontend
npm run typecheck && npm test && npm run build
```

Covered by automated tests:

- GTIN check digits, UPC-E expansion, EAN-8/UPC-A/EAN-13 equivalence, GS1 parsing, URL classification.
- Identification status rules: a provider outage is never reported as "not found".
- Provider output sanitization (control characters, non-HTTPS images, unverified "verified" status).
- Checkout arithmetic, payment rules, tax rounding.
- Concurrency: 25 simultaneous checkouts against 10 units of stock sell exactly 10. Simultaneous creation of the same barcode produces exactly one product.
- Tenant isolation, logout revocation, generic login errors.
- Form schemas, cart totals, scanner input, and suggestion mapping (external data never fills prices, stock, SKU or category).

### End-to-end tests

The Playwright suite drives the real UI through the whole workflow. It needs a clean database so every run starts from the same state.

```bash
createdb -O pos pos_e2e && DATABASE_URL=postgres://pos:...@127.0.0.1:5432/pos_e2e (cd backend && cargo run --bin seed)
(cd backend && DATABASE_URL=... BIND_ADDR=127.0.0.1:8081 CORS_ALLOWED_ORIGINS=http://localhost:3001 cargo run --bin pos-backend)
(cd frontend && BACKEND_URL=http://localhost:8081 npx next dev --port 3001)
cd frontend && E2E_BASE_URL=http://localhost:3001 npx playwright test --trace off
```

If your machine has a Chromium build that does not match `@playwright/test`, set `PLAYWRIGHT_CHROMIUM_PATH` to its binary.

## Security

- Passwords: argon2id. Session tokens: 256-bit random values, stored only as SHA-256 hashes, revoked on logout.
- Every business query is scoped to the caller's store. Other stores' records return 404 or 403, never the data.
- Roles are enforced on the server. Prices and totals are recalculated from the database at checkout.
- Login: generic error messages, timing-equalized for unknown emails, and rate-limited per account.
- External data is treated as untrusted: sanitized, length-limited, never auto-opened (no URL is fetched), and kept separate from merchant data.
- Security headers, a 1 MB request body limit, and a no-store cache policy on API responses.

## Roadmap

1. Multi-store switcher in the UI, and the Bahasa Indonesia translation of the interface.
2. Void and refund flows with inventory reversal, plus a shift/cash-drawer summary.
3. Mixed tender in the cashier (the API already accepts several payments).
4. Reports: profit by product (purchase vs. selling price), CSV export.
5. Connect an approved BPOM RI source through the existing provider interface (see the integration note).
6. Redis-backed rate limits and session cache for multi-instance deployments.
7. Refresh-token rotation and optional two-factor authentication.

## License

No license has been chosen yet. Add one before accepting outside contributions.
