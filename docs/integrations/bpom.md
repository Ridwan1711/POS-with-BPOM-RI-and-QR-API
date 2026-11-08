# BPOM RI integration

**Status: not connected.** The application has a provider adapter for BPOM RI product registration data, but it does not call any external service.

## Why it is not connected

- No public, documented BPOM RI endpoint was verified while building this project, and no access credentials were available. The adapter therefore does not guess URLs or response formats.
- Scraping BPOM web pages or other restricted services is out of scope.
- A barcode match is not proof of registration. The application does not label any product as BPOM-registered or verified unless an official source said so.

The POS works without BPOM data. Merchants enter registration numbers by hand, and those numbers are shown as **reported**, never **verified**.

## What exists in the code

- `backend/src/modules/product_identification/providers/bpom.rs`: `BpomRegistryAdapter`, which implements `ProductDataProvider` with code `bpom_registry`.
  - No `BPOM_API_BASE_URL`: returns `NotConfigured`.
  - With the variable set: returns `Unavailable` (not implemented). It never returns "not found".
- `external_product_sources` row `bpom_registry`, disabled by default and marked `requires_credentials = true`.
- Registration fields on products: number, status (`unknown`, `reported`, `verified`, `expired`), validity date, verification timestamp. Manual saves can only use `unknown` or `reported`.

## Fields the integration should provide

When an approved source exists, the adapter should map its response to `ExternalProductData` and `RegistrationInfo`:

| Field | Source of truth | Notes |
| --- | --- | --- |
| Registration number | official registry | Store as provided. Do not reformat. |
| Product name, brand, manufacturer | official registry | Only if the registry publishes them. |
| Registration status | official registry | `verified` only with `verified_at` set to the time of the check. |
| Validity date | official registry | Store as `valid_until`. Show "Expired" when it is in the past. |
| Source and time of check | the adapter | Recorded in provenance (`product_external_records`). |

## Steps to connect a legitimate source

1. Confirm the data source, its terms of use, whether a licence or agreement is needed, and the permitted use.
2. Obtain the endpoint documentation and any credentials. Store credentials only in environment variables or a secret manager, never in the repository.
3. Implement `lookup_gtin` in `providers/bpom.rs`:
   - Use an HTTP client with a timeout (`PROVIDER_TIMEOUT_MS`) and respect the provider's rate limits.
   - Return `NotFound` only when the source answered and has no record.
   - Return `Unavailable` for network errors, timeouts, 5xx responses, rate limits, or unparseable responses.
   - Return `Found` with sanitized data. `verified` requires a real verification timestamp.
4. Add contract tests with recorded responses, including the "source down" and "rate limited" cases.
5. Set `BPOM_API_BASE_URL` and the credentials in the environment, then enable the row in `external_product_sources`.
6. Record the change in the README and this document.

## Until then

- Merchants use manual entry for registration numbers, as described above.
- The product page shows "Reported, not verified" or "No registration number recorded", with a note that a barcode match is not proof of registration.
