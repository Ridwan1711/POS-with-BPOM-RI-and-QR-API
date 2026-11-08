-- Product identification: external source registry, lookup cache, lookup history,
-- and provenance records for data that was merchant-confirmed from an external source.

-- Registry of external product data sources. Only providers implemented in code can run,
-- and only when `enabled` is true here. Rate limits are enforced per provider.
CREATE TABLE external_product_sources (
    code                    TEXT PRIMARY KEY CHECK (code ~ '^[a-z][a-z0-9_]{1,40}$'),
    name                    TEXT NOT NULL,
    description             TEXT,
    enabled                 BOOLEAN NOT NULL DEFAULT FALSE,
    priority                INTEGER NOT NULL DEFAULT 100,
    requires_credentials    BOOLEAN NOT NULL DEFAULT FALSE,
    rate_limit_per_minute   INTEGER NOT NULL DEFAULT 30 CHECK (rate_limit_per_minute > 0),
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO external_product_sources (code, name, description, enabled, priority, requires_credentials, rate_limit_per_minute)
VALUES
    ('mock', 'Development mock catalog',
     'Deterministic fake data for local development and tests. Every value is labelled as mock. Never enable in production.',
     FALSE, 10, FALSE, 120),
    ('bpom_registry', 'BPOM RI registry (adapter)',
     'Adapter for an official BPOM RI product registration source. Stays disabled until a legitimate, documented endpoint and credentials are configured. See docs/integrations/bpom.md.',
     FALSE, 1, TRUE, 30);

-- Short-lived cache of external lookups keyed by canonical GTIN-14.
-- Errors are never cached. Negative results expire quickly so new registrations appear.
CREATE TABLE product_lookup_cache (
    canonical_gtin  TEXT PRIMARY KEY CHECK (char_length(canonical_gtin) = 14),
    outcome         TEXT NOT NULL CHECK (outcome IN ('identified', 'partially_identified', 'not_found')),
    providers       JSONB NOT NULL,
    fetched_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at      TIMESTAMPTZ NOT NULL
);
CREATE INDEX product_lookup_cache_expiry_idx ON product_lookup_cache (expires_at);

CREATE TABLE product_lookup_history (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    store_id         UUID NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    user_id          UUID REFERENCES users(id) ON DELETE SET NULL,
    raw_input        TEXT NOT NULL CHECK (char_length(raw_input) <= 2048),
    scheme           TEXT,
    canonical_value  TEXT,
    status           TEXT NOT NULL,
    product_id       UUID REFERENCES products(id) ON DELETE SET NULL,
    from_cache       BOOLEAN NOT NULL DEFAULT FALSE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX product_lookup_history_store_idx ON product_lookup_history (store_id, created_at DESC);

-- Provenance: which external source a product's initial details were confirmed from.
-- The payload is the sanitized provider output at confirmation time, kept separate from
-- merchant-editable product columns so that re-lookups never overwrite merchant data.
CREATE TABLE product_external_records (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    store_id     UUID NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    product_id   UUID NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    source_code  TEXT NOT NULL REFERENCES external_product_sources(code),
    confidence   TEXT NOT NULL CHECK (confidence IN ('low', 'medium', 'high')),
    payload      JSONB NOT NULL,
    fetched_at   TIMESTAMPTZ NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (product_id, source_code)
);
