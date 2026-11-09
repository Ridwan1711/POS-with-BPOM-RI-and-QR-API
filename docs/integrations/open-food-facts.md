# Open Food Facts integration

**Status:** implemented as provider `open_food_facts`. It is disabled by default. It starts only when `OFF_USER_AGENT` is set and the source is enabled (the seed does this when the variable is present).

## Terms that apply (official terms and API docs)

- **Database:** ODbL. **Individual contents:** Database Contents Licence 1.0. **Product photos:** CC BY-SA.
- Commercial use is allowed. Re-users must name the licence and attribute the authorship to Open Food Facts. Derivative works must be shared under the same conditions.
- The data is not guaranteed to be accurate or complete. It is entered by volunteers and can contain errors. It is also subject to change at any time.
- We show the attribution in the UI: "Data from Open Food Facts, licensed under ODbL. Attribution required. Photos are CC BY-SA."
- Photos are a separate matter. Third-party rights such as product design and trademarks can apply, so the image is shown as a link only and is never downloaded or stored.

## Rate limits and etiquette (official API docs)

- **Product reads: 15 requests per minute per IP.** Search: 10 requests per minute per IP.
- All merchants share this server's IP, so the limit is enforced in code. `external_product_sources.rate_limit_per_minute` is set to 15, and the provider also has its own limiter for each HTTP attempt.
- A global limit applies on top of the per-IP one. When it is exceeded, OFF answers **503**. Earlier 503 responses during testing may have come from this global limit, but that is a hypothesis, not a confirmed cause.
- Exceeding the limit can lead to an IP ban. The ban can be lifted by emailing OFF.
- More than a few hundred products should be fetched by downloading CSV or JSONL instead of repeated API calls.
- **User-Agent is required:** `AppName/Version (ContactEmail)`. Set it with `OFF_USER_AGENT`. Keep the real value out of git. The `.env.example` files contain only placeholders.
- Retries: wait a few seconds, then double the delay with jitter, and limit the number of attempts. Do not retry other 4xx responses.
- For apps, OFF recommends creating an account and filling in the API usage form. This is not yet done (see open items).

## Behaviour in the code

- Endpoint: `GET {OFF_API_BASE_URL}/api/v3/product/{code}?fields=code,product_name,brands,quantity,ingredients_text,image_front_small_url`. v3 is the version OFF recommends for new integrations.
- Barcode: the canonical GTIN-14 is sent without its padding zeros. OFF normalizes the other forms itself (see the OFF barcode normalization reference).
- Mapping: `product_name` becomes the name, and the first entry in `brands` becomes the brand. `quantity` and `ingredients_text` go into the description as `Net weight:` and `Ingredients:`. `image_front_small_url` becomes the image link. Manufacturer and registration are not set, because OFF does not provide them reliably.
- Outcomes: HTTP 404 or a missing `product` is `not_found`. HTTP 429, 5xx, network errors, and timeouts are retried up to 3 times with backoff, then reported as `unavailable`. Other 4xx responses are `unavailable` without a retry.
- Confidence: `medium`, because the data is crowdsourced.
- The provider timeout is `PROVIDER_TIMEOUT_MS` (default 12 s). This covers three attempts of up to 3 s each, plus backoff.

## Configuration

| Variable | Default | Purpose |
|---|---|---|
| `OFF_USER_AGENT` | unset | Required. Format `AppName/Version (ContactEmail)`. Without it the provider reports `NotConfigured`. |
| `OFF_API_BASE_URL` | `https://world.openfoodfacts.org` | Base URL. Use the staging host only for testing. |

## Open items

1. Create an OFF account and fill in the API usage form for this app. This is a manual step that needs the owner.
2. Get a legal review of storing OFF data per merchant. The main question is whether it creates a derivative database under ODbL. This document is not legal advice.
3. Decide whether to store the nutrition data (Nutri-Score, NOVA) in the provenance payload. It is not stored today.
