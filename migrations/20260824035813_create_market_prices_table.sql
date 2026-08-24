-- Add migration script here
CREATE TABLE market_prices (
    id BIGSERIAL PRIMARY KEY,
    symbol VARCHAR(20) NOT NULL UNIQUE,
    price_usd DOUBLE PRECISION NOT NULL,
    change_24h_percent DOUBLE PRECISION NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);