-- Sales (transactions), sale line items with price snapshots, and payments.

CREATE TABLE sales (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    store_id            UUID NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    receipt_number      TEXT NOT NULL,
    -- Client-generated idempotency key: a retried checkout returns the original sale.
    client_request_id   UUID,
    cashier_id          UUID REFERENCES users(id) ON DELETE SET NULL,
    customer_id         UUID REFERENCES customers(id) ON DELETE SET NULL,
    status              TEXT NOT NULL DEFAULT 'completed' CHECK (status IN ('completed')),
    subtotal_idr        BIGINT NOT NULL CHECK (subtotal_idr >= 0),
    item_discount_idr   BIGINT NOT NULL DEFAULT 0 CHECK (item_discount_idr >= 0),
    sale_discount_idr   BIGINT NOT NULL DEFAULT 0 CHECK (sale_discount_idr >= 0),
    tax_rate_bps        INTEGER NOT NULL DEFAULT 0 CHECK (tax_rate_bps BETWEEN 0 AND 3000),
    tax_idr             BIGINT NOT NULL DEFAULT 0 CHECK (tax_idr >= 0),
    total_idr           BIGINT NOT NULL CHECK (total_idr >= 0),
    paid_idr            BIGINT NOT NULL CHECK (paid_idr >= total_idr),
    change_idr          BIGINT NOT NULL DEFAULT 0 CHECK (change_idr >= 0),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT sales_store_receipt_key UNIQUE (store_id, receipt_number),
    CONSTRAINT sales_store_client_request_key UNIQUE (store_id, client_request_id)
);
CREATE INDEX sales_store_created_idx ON sales (store_id, created_at DESC);

CREATE TABLE sale_items (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sale_id           UUID NOT NULL REFERENCES sales(id) ON DELETE CASCADE,
    line_no           INTEGER NOT NULL CHECK (line_no > 0),
    product_id        UUID REFERENCES products(id) ON DELETE SET NULL,
    -- Snapshots: historical receipts must not change when the product is edited later.
    product_name      TEXT NOT NULL,
    sku               TEXT,
    unit              TEXT NOT NULL,
    unit_price_idr    BIGINT NOT NULL CHECK (unit_price_idr >= 0),
    quantity          INTEGER NOT NULL CHECK (quantity > 0),
    discount_idr      BIGINT NOT NULL DEFAULT 0 CHECK (discount_idr >= 0),
    line_total_idr    BIGINT NOT NULL CHECK (line_total_idr >= 0),
    CHECK (discount_idr <= unit_price_idr * quantity),
    UNIQUE (sale_id, line_no)
);

CREATE TABLE payments (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sale_id     UUID NOT NULL REFERENCES sales(id) ON DELETE CASCADE,
    line_no     INTEGER NOT NULL CHECK (line_no > 0),
    method      TEXT NOT NULL CHECK (method IN ('cash', 'qris', 'debit_card', 'credit_card', 'bank_transfer', 'ewallet')),
    amount_idr  BIGINT NOT NULL CHECK (amount_idr > 0),
    reference   TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (sale_id, line_no)
);
