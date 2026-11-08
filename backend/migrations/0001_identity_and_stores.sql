-- Identity, tenancy (stores), memberships, server-side sessions and audit logs.

CREATE TABLE users (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email           TEXT NOT NULL CHECK (email = lower(email) AND char_length(email) BETWEEN 3 AND 254),
    full_name       TEXT NOT NULL CHECK (char_length(btrim(full_name)) BETWEEN 1 AND 120),
    password_hash   TEXT NOT NULL,
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    last_login_at   TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX users_email_key ON users (email);

-- A store (merchant location). Every business table is scoped by store_id.
CREATE TABLE stores (
    id                          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name                        TEXT NOT NULL CHECK (char_length(btrim(name)) BETWEEN 1 AND 120),
    address                     TEXT,
    phone                       TEXT,
    timezone                    TEXT NOT NULL DEFAULT 'Asia/Jakarta',
    currency_code               TEXT NOT NULL DEFAULT 'IDR' CHECK (currency_code = 'IDR'),
    tax_rate_bps                INTEGER NOT NULL DEFAULT 0 CHECK (tax_rate_bps BETWEEN 0 AND 3000),
    allow_negative_stock        BOOLEAN NOT NULL DEFAULT FALSE,
    default_low_stock_threshold INTEGER NOT NULL DEFAULT 5 CHECK (default_low_stock_threshold >= 0),
    receipt_footer              TEXT,
    -- Per-store receipt counter. Incremented under a row lock during checkout.
    next_receipt_seq            BIGINT NOT NULL DEFAULT 1 CHECK (next_receipt_seq >= 1),
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE store_memberships (
    store_id    UUID NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role        TEXT NOT NULL CHECK (role IN ('owner', 'manager', 'cashier')),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (store_id, user_id)
);
CREATE INDEX store_memberships_user_idx ON store_memberships (user_id, created_at);

-- Opaque server-side sessions. Only the SHA-256 hash of the token is stored.
CREATE TABLE auth_sessions (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  TEXT NOT NULL UNIQUE,
    expires_at  TIMESTAMPTZ NOT NULL,
    revoked_at  TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX auth_sessions_user_idx ON auth_sessions (user_id);

CREATE TABLE audit_logs (
    id            BIGSERIAL PRIMARY KEY,
    store_id      UUID REFERENCES stores(id) ON DELETE SET NULL,
    user_id       UUID REFERENCES users(id) ON DELETE SET NULL,
    action        TEXT NOT NULL,
    entity_type   TEXT NOT NULL,
    entity_id     TEXT,
    metadata      JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX audit_logs_store_created_idx ON audit_logs (store_id, created_at DESC);
