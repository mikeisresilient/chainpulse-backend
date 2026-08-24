-- Add migration script here
CREATE TABLE portfolio_snapshots (
    id BIGSERIAL PRIMARY KEY,

    wallet_id BIGINT NOT NULL REFERENCES wallets(id) ON DELETE CASCADE,

    total_value_usd NUMERIC(30, 10) NOT NULL,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_portfolio_snapshots_wallet_id
ON portfolio_snapshots(wallet_id);

CREATE INDEX idx_portfolio_snapshots_created_at
ON portfolio_snapshots(created_at);