use std::sync::Arc;

use alloy_provider::Provider;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Wallet {
    pub id: i64,
    pub address: String,
    pub network: String,
    pub label: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateWallet {
    pub address: String,
    pub network: String,
    pub label: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Token {
    pub id: i64,
    pub symbol: String,
    pub name: Option<String>,
    pub contract_address: String,
    pub network: String,
    pub decimals: i16,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateToken {
    pub symbol: String,
    pub name: Option<String>,
    pub contract_address: String,
    pub network: String,
    pub decimals: i16,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Transaction {
    pub id: i64,
    pub wallet_id: i64,
    pub hash: String,
    pub from_address: String,
    pub to_address: Option<String>,
    pub value_wei: sqlx::types::BigDecimal,
    pub block_number: i64,
    pub timestamp: i64,
    pub gas_used: Option<sqlx::types::BigDecimal>,
    pub gas_price_wei: Option<sqlx::types::BigDecimal>,
    pub is_error: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone)]
pub struct AppState<P>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    pub db: PgPool,
    pub ethereum: Arc<P>,
    pub http_client: reqwest::Client,
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct PortfolioSnapshot {
    pub id: i64,
    pub wallet_id: i64,
    pub total_value_usd: bigdecimal::BigDecimal,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
