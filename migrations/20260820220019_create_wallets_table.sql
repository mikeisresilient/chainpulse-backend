-- Add migration script here
CREATE TABLE wallets (
    id BIGSERIAL PRIMARY KEY,
    address VARCHAR(42) NOT NULL,
    network VARCHAR(50) NOT NULL,
    label VARCHAR(100),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
