-- Add migration script here
CREATE TABLE tokens (
    id BIGSERIAL PRIMARY KEY,
    symbol VARCHAR(20) NOT NULL,
    name VARCHAR(100),
    contract_address VARCHAR(42) NOT NULL,
    network VARCHAR(50) NOT NULL,
    decimals SMALLINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);