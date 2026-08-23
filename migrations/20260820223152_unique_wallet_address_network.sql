-- Add migration script here
CREATE UNIQUE INDEX wallets_address_network_idx
ON wallets (LOWER(address), network);