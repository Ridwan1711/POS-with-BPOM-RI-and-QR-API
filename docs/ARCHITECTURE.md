# Architecture

## Principles

- **Modular monolith.** One deployable API and one database. Modules are separated by directory and by who may write their tables. No microservices yet.
- **The backend is the source of truth.** Prices, discounts, tax, totals and stock are calculated on the server. The frontend previews totals with the same rounding, but it never decides them.
- **Integer money.** All amounts are whole rupiah (`BIGINT`, `i64`, JSON integers). There is no floating point anywhere in money paths.
- **Tenant isolation in one place.** The `StoreContext` extractor resolves the store and the caller's role. Every business query takes `store_id` from it, never from the request body.
- **External data is untrusted and separate.** Provider output is sanitized and stored as provenance. It never overwrites merchant-owned columns.

## Module map (backend)

| Module | Responsibility | Writes to |
| --- | --- | --- |
| `auth` | Login, sessions, password hashing, `CurrentUser` and `StoreContext` extractors, role checks | `auth_sessions`, `users.last_login_at` |
| `settings` | Store profile, tax rate, time zone, inventory policy, receipt footer | `stores` |
| `modules/categories` | Product categories | `categories` |
| `modules/products` | Catalog: create, edit, deactivate, identifiers, detail view | `products`, `product_identifiers`, `product_external_records` |
| `modules/product_identification` | Barcode/QR classification, local match, lookup cache, provider fan-out, status decision | `product_lookup_cache`, `product_lookup_history` |
| `modules/inventory` | Stock adjustments and the movement ledger | `products.stock_quantity`, `inventory_movements` |
| `modules/sales` | Checkout (pricing, payments, stock, receipt number), transaction history | `sales`, `sale_items`, `payments`, `stores.next_receipt_seq` |
| `modules/dashboard` | Read-only summary for a store day | none |
| `audit` | Append-only audit records written in the same transaction as the change | `audit_logs` |

Each module owns its HTTP handlers, SQL and DTOs. Shared code is limited to `error`, `pagination`, `money`, `util`, `rate_limit`.

## Request lifecycle

1. The request reaches `build_router` (`app.rs`). Body size is limited, security headers are added, and a trace span is opened.
2. If the handler takes a `StoreContext`, the extractor reads the `pos_session` cookie, looks up an unexpired, unrevoked session, reads `X-Store-Id` (or defaults to the first membership), and loads the role. Failures are 401 (no valid session) or 403 (not a member).
3. The handler checks the role (`ctx.require(Role::Manager)`), validates the body with `validator`, and calls domain logic.
4. Writes run in one transaction together with their audit row. Reads are scoped by `ctx.store_id`.
5. Errors become `AppError`, which renders the common error shape. Database errors are logged and returned as a generic 500.

## Concurrency model

| Operation | Mechanism | Guarantee |
| --- | --- | --- |
| Checkout | `SELECT … FROM stores WHERE id = $1 FOR UPDATE`, then `products … ORDER BY id FOR UPDATE` | No overselling. Stock is read and decremented under lock. Receipt numbers have no gaps. No deadlocks, because locks are always taken in the same order. |
| Idempotent checkout | `client_request_id` is checked after the store lock is taken | A retried request returns the original sale (HTTP 200), never a second charge. |
| Stock adjustment | `products … FOR UPDATE` | Adjustments on the same product serialize with each other and with sales. |
| Product creation with a duplicate barcode | `UNIQUE (store_id, scheme, value)` plus a pre-check | Exactly one product per barcode per store, even under races. The loser gets 409 `duplicate_identifier`. |
| Session revocation | `revoked_at` checked on every request | Logout takes effect on the next request. |

These guarantees are covered by integration tests (`backend/tests/api.rs`).

## Data model (summary)

```
users ──< store_memberships >── stores ──< categories
   │                              │  ├──< products ──< product_identifiers
   └──< auth_sessions             │  │        ├──< inventory_movements
                                  │  │        └──< product_external_records
                                  │  ├──< sales ──< sale_items   (name/price snapshots)
                                  │  │        └──< payments
                                  │  ├──< customers, suppliers   (schema only, for later)
                                  │  └──< product_lookup_history
external_product_sources  (registry: which providers may run, rate limits)
product_lookup_cache      (global cache keyed by canonical GTIN-14; no merchant data)
audit_logs
```

Notes:

- `products.stock_quantity` is the current balance. `inventory_movements` is the history. Each balance change writes one movement with the reason and the balance after the change.
- `product_identifiers.value` is the canonical form used for matching (GTIN-14 for barcodes). `raw_value` keeps what was scanned.
- `sale_items` copies the product name, SKU, unit and price at sale time, so old receipts never change.
- `product_external_records` keeps the provider data a product was confirmed from, separate from the editable product columns.

## Frontend structure

```
app/                       routes (App Router)
  (app)/…                  authenticated shell: pos, products, inventory, transactions,
                           dashboard, categories, settings
  login/                   sign-in
  api/v1/[...path]/        runtime proxy to the API (cookies forwarded both ways)
features/                  screens and feature logic
  pos/                     cart store (Zustand), payment dialog, receipt
  products/                identification panel, product form, list, detail, suggestion mapping
  inventory/, sales/, dashboard/, settings/, auth/
components/                shared UI (scan field, camera scanner, app shell) and components/ui (small design system)
lib/
  api/client.ts            fetch wrapper: X-Store-Id, error mapping
  api/queries.ts           TanStack Query hooks and cache keys, one place for all server state
  schemas.ts               Zod schemas for every form
  format.ts                IDR and date formatting
```

Server state lives in TanStack Query. Local UI state lives in components. Zustand holds only the cart, which several components share.

## Security model

- **Authentication:** opaque session tokens in an HttpOnly, SameSite=Lax cookie (`Secure` when `COOKIE_SECURE=true`). Tokens are stored as SHA-256 hashes. Sessions expire and can be revoked. There are no JWTs, so there is no signing key to leak.
- **Passwords:** argon2id with a per-password salt. Unknown-email logins run a dummy verification so response time does not reveal which emails exist.
- **Brute force:** login attempts are limited per account in a fixed window (default 10 per 15 minutes). Identification calls are limited per user and per provider. The limiter is in memory, so it is per instance.
- **Authorization:** roles are checked in handlers (`cashier` < `manager` < `owner`). Store settings are owner-only. Products, categories, inventory adjustments and external identification are manager-or-higher. Selling is open to every member.
- **Input:** every request body is validated. Identifiers are length-limited (2048). Search terms are escaped for `LIKE`. Uploaded images are not fetched, and only `https://` URLs are stored.
- **Browser:** the page never reads the session token. CSP is not set yet (see the roadmap). Security headers include `X-Frame-Options: DENY`, `nosniff`, and `Referrer-Policy: no-referrer`.
- **Logs:** request traces do not include bodies. The database password is never logged. Provider messages are short plain text.

## Operational notes

- Migrations are embedded in the binary and run at startup (`sqlx::migrate!`). Run only one API instance during a migration, or run migrations as a separate step.
- `/health` is a liveness check. `/health/ready` checks the database.
- Set `LOG_JSON=true` for structured logs in containers.
- The external lookup cache is global and keyed by GTIN. It holds public product data only, never merchant data.
