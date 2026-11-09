# Product identification

Identification turns a scanned or typed code into one clear outcome. It never invents product data, and it never reports a failed lookup as "not found".

Endpoints:

| Endpoint | Who | Calls external providers | Used by |
| --- | --- | --- | --- |
| `POST /api/v1/products/identify` | manager, owner | yes | Add-product screen |
| `POST /api/v1/products/scan` | any member | no (local only) | Cashier |
| `GET /api/v1/products/identifiers/{identifier}` | any member | no | direct lookup |

## Input classification

Every input is trimmed, then classified:

| Input | Example | Result |
| --- | --- | --- |
| GTIN barcode | `8991002101029` (EAN-13), `96385074` (EAN-8), `036000291452` (UPC-A), `04252614` (UPC-E), 14-digit GTIN | `gtin`, checksum verified |
| Barcode with a bad check digit | `4006381333932` | `invalid_code` |
| GS1 Digital Link URL | `https://example.org/01/08991002101029/10/ABC` | GTIN read from the `/01/` path segment. The URL is not opened. |
| GS1 element string | `(01)08991002101029(17)261231` | GTIN from AI 01 |
| Other web link | `https://example.com/promo` | `unsupported_format`. Stored as a QR payload only. Never fetched. |
| Merchant internal code | `ROTI-001` | Internal code. Matched to identifiers and SKU. |
| Other QR payload | `BEGIN:VCARD…` | `unsupported_format` |

Numeric codes with 8, 12, 13 or 14 digits are GTINs. Each is canonicalized to a 14-digit form (left-padded with zeros), so an EAN-13 and its UPC-A form match. UPC-E is expanded to UPC-A first. An 8-digit code that starts with 0 or 1 and has a valid UPC-E check digit is read as UPC-E. Otherwise it is read as EAN-8.

## Workflow

```
classify input
  ├─ invalid            → invalid_code
  ├─ QR payload / link  → check local QR identifiers → existing_product, or unsupported_format
  ├─ internal code      → check local identifiers and SKU → existing_product, or not_found
  └─ GTIN
       1. local identifiers (gtin, qr) → existing_product (never duplicated)
       2. lookup cache (unexpired) → use cached provider results (from_cache = true)
       3. enabled providers, in parallel, each with a rate limit and a timeout
       4. decide status (below)
       5. cache complete answers only
  → record the lookup in product_lookup_history
```

## Status values

| Status | Meaning | UI wording (summary) |
| --- | --- | --- |
| `existing_product` | Your store already has this code. Returned with the product. | Show the product. Do not create another. |
| `identified` | A provider supplied a product name. The merchant must review it. | "Product found in external data." |
| `partially_identified` | A provider supplied some fields but no name. | "Partial information found." |
| `not_found` | Every enabled provider answered, and none has this GTIN. | "No external record for this code." |
| `provider_unavailable` | A provider failed, timed out, was rate-limited, or no provider is enabled. | "External sources could not be checked." This is not proof that the product does not exist. |
| `invalid_code` | The barcode failed validation. | Show the reason. |
| `unsupported_format` | This code cannot identify a product. | Enter the product manually. |

Decision rules (`service.rs::assess`):

- No enabled provider → `provider_unavailable`.
- Any provider found a name → `identified`. Data without a name → `partially_identified`.
- No data, and any provider failed or is not configured → `provider_unavailable`.
- No data, and every provider answered "not found" → `not_found`.

Field merging: providers are ordered by priority. For each field, the first provider that supplies it wins.

## Caching and rate limits

- Cache key: canonical GTIN-14. The cache is global and holds public product data only.
- Found or partially identified results: cached for `LOOKUP_CACHE_HIT_HOURS` (default 168).
- Not-found results: cached for `LOOKUP_CACHE_MISS_MINUTES` (default 60), so newly registered products appear quickly.
- `provider_unavailable` results are never cached.
- Provider rate limit: `external_product_sources.rate_limit_per_minute` (fixed one-minute window, per process).
- Provider timeout: `PROVIDER_TIMEOUT_MS` (default 4000).
- Identification requests per user: 60 per minute.

## Providers

Providers implement the `ProductDataProvider` trait (`provider.rs`):

```rust
#[async_trait]
pub trait ProductDataProvider: Send + Sync {
    fn code(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn confidence(&self) -> Confidence;
    async fn lookup_gtin(&self, canonical_gtin: &str) -> LookupOutcome;
}
```

`LookupOutcome` is one of `Found(data)`, `NotFound`, `Unavailable(message)`, or `NotConfigured(message)`. Only the registry in code decides which providers exist. The `external_product_sources` table decides which ones run.

| Code | Status | Notes |
| --- | --- | --- |
| `mock` | Development only, disabled by default | Deterministic catalog. Every value is labelled `[MOCK]`. GTIN `08990000000006` simulates an outage. |
| `open_food_facts` | Live, disabled by default | Crowdsourced food database (ODbL). Needs `OFF_USER_AGENT`. Rate-limited to 15 per minute. See [integrations/open-food-facts.md](integrations/open-food-facts.md). |
| `bpom_registry` | Adapter, disabled | Reports `NotConfigured` until `BPOM_API_BASE_URL` is set. It never calls a URL today. See [integrations/bpom.md](integrations/bpom.md). |

## Sanitization of provider output

Applied to every provider result before it is used (`sanitize.rs`):

- Control characters are removed, text is trimmed, and length limits are applied in characters.
- Image URLs must be `https://` with no whitespace. They are stored as text and never fetched.
- A registration marked `verified` without a verification timestamp is downgraded to `reported`.
- An empty registration is dropped.

## What the merchant sees and saves

- The suggestion lists the fields that were actually supplied (`fields`) and the sources with their confidence. Fields a source did not supply are left empty.
- The form only receives supplied fields. Prices, stock, SKU, category and the low-stock threshold are never filled from external data.
- A provider's "verified" status is shown as "reported". Merchants cannot set "verified", and only an official source can.
- On save, the server re-reads provenance from its own cache for the product's GTIN. It does not trust the client's copy of the external data.
- The product keeps a provenance record (`product_external_records`), which is separate from the editable product fields.

## Adding a provider

1. Implement `ProductDataProvider` in `backend/src/modules/product_identification/providers/`. Return `Unavailable` for every error. Never return `NotFound` for a failure.
2. Register it in `providers/mod.rs::ProviderRegistry::from_config`.
3. Add a row to `external_product_sources` (migration) with `enabled = false` and an appropriate rate limit.
4. Add tests for its outcomes, including timeout and rate-limit behaviour.
5. Document the source, its terms of use, and its access requirements.
