-- Categories, products (merchant-owned pricing and stock), identifiers, inventory ledger,
-- and CRM placeholders (customers, suppliers) required by the multi-store schema.

CREATE TABLE categories (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    store_id    UUID NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    name        TEXT NOT NULL CHECK (char_length(btrim(name)) BETWEEN 1 AND 100),
    is_active   BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX categories_store_name_key ON categories (store_id, lower(name));

-- Products are store-specific. Monetary values are integer IDR (no floating point).
-- External provider data never writes to price, stock, or SKU columns.
CREATE TABLE products (
    id                          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    store_id                    UUID NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    category_id                 UUID REFERENCES categories(id) ON DELETE SET NULL,
    name                        TEXT NOT NULL CHECK (char_length(btrim(name)) BETWEEN 1 AND 200),
    description                 TEXT,
    brand                       TEXT,
    manufacturer                TEXT,
    sku                         TEXT CHECK (sku IS NULL OR char_length(sku) BETWEEN 1 AND 64),
    unit                        TEXT NOT NULL DEFAULT 'pcs',
    purchase_price_idr          BIGINT NOT NULL DEFAULT 0 CHECK (purchase_price_idr >= 0),
    selling_price_idr           BIGINT NOT NULL CHECK (selling_price_idr >= 0),
    stock_quantity              INTEGER NOT NULL DEFAULT 0,
    low_stock_threshold         INTEGER CHECK (low_stock_threshold IS NULL OR low_stock_threshold >= 0),
    image_url                   TEXT,
    bpom_registration_number    TEXT,
    registration_status         TEXT NOT NULL DEFAULT 'unknown'
                                CHECK (registration_status IN ('unknown', 'reported', 'verified', 'expired')),
    registration_valid_until    DATE,
    registration_verified_at    TIMESTAMPTZ,
    data_source                 TEXT NOT NULL DEFAULT 'manual',
    is_active                   BOOLEAN NOT NULL DEFAULT TRUE,
    created_by                  UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX products_store_sku_key ON products (store_id, sku) WHERE sku IS NOT NULL;
CREATE INDEX products_store_name_idx ON products (store_id, lower(name));
CREATE INDEX products_store_category_idx ON products (store_id, category_id);

-- Many identifiers per product (GTIN family, QR payloads, merchant internal codes).
-- `value` is the canonical form used for matching; `raw_value` is what was scanned.
CREATE TABLE product_identifiers (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    store_id    UUID NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    product_id  UUID NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    scheme      TEXT NOT NULL CHECK (scheme IN ('gtin', 'qr', 'internal')),
    kind        TEXT NOT NULL,
    raw_value   TEXT NOT NULL CHECK (char_length(raw_value) BETWEEN 1 AND 2048),
    value       TEXT NOT NULL CHECK (char_length(value) BETWEEN 1 AND 2048),
    is_primary  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT product_identifiers_store_value_key UNIQUE (store_id, scheme, value)
);
CREATE UNIQUE INDEX product_identifiers_one_primary_key ON product_identifiers (product_id) WHERE is_primary;
CREATE INDEX product_identifiers_product_idx ON product_identifiers (product_id);

-- Append-only stock ledger. Every change to products.stock_quantity has exactly one row here.
CREATE TABLE inventory_movements (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    store_id        UUID NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    product_id      UUID NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    quantity_delta  INTEGER NOT NULL CHECK (quantity_delta <> 0),
    quantity_after  INTEGER NOT NULL,
    reason          TEXT NOT NULL CHECK (reason IN (
                        'initial_stock', 'restock', 'sale', 'return',
                        'damage', 'loss', 'correction', 'count_adjustment', 'other')),
    note            TEXT,
    reference_type  TEXT,
    reference_id    UUID,
    created_by      UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX inventory_movements_product_idx ON inventory_movements (product_id, created_at DESC);
CREATE INDEX inventory_movements_store_idx ON inventory_movements (store_id, created_at DESC);

CREATE TABLE customers (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    store_id    UUID NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    name        TEXT NOT NULL CHECK (char_length(btrim(name)) BETWEEN 1 AND 120),
    phone       TEXT,
    email       TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX customers_store_idx ON customers (store_id);

CREATE TABLE suppliers (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    store_id    UUID NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    name        TEXT NOT NULL CHECK (char_length(btrim(name)) BETWEEN 1 AND 120),
    phone       TEXT,
    email       TEXT,
    address     TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX suppliers_store_idx ON suppliers (store_id);
