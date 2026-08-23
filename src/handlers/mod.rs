use alloy_provider::Provider;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use sqlx::QueryBuilder;

use crate::models::{
    AppState, CreateToken, CreateWallet, PortfolioSnapshot, Token, Transaction, Wallet,
};

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub database: String,
}

pub async fn health_check<P>(State(state): State<AppState<P>>) -> Json<HealthResponse>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    let database_status = match sqlx::query("SELECT 1").execute(&state.db).await {
        Ok(_) => "connected",
        Err(_) => "disconnected",
    };

    Json(HealthResponse {
        status: "ok".to_string(),
        service: "ChainPulse API".to_string(),
        database: database_status.to_string(),
    })
}

// ============================================================
// WALLETS
// ============================================================

pub async fn create_wallet<P>(
    State(state): State<AppState<P>>,
    Json(payload): Json<CreateWallet>,
) -> Result<(StatusCode, Json<Wallet>), (StatusCode, Json<serde_json::Value>)>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    let network = payload.network.to_lowercase();

    if network != "ethereum" && network != "base" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Unsupported network",
                "message": "Supported networks are ethereum and base"
            })),
        ));
    }

    let address = payload.address.trim();

    if address.parse::<alloy_primitives::Address>().is_err() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Invalid wallet address",
                "message": "Please provide a valid Ethereum-compatible wallet address"
            })),
        ));
    }

    let wallet = sqlx::query_as::<_, Wallet>(
        r#"
        INSERT INTO wallets (address, network, label)
        VALUES ($1, $2, $3)
        RETURNING id, address, network, label, created_at, updated_at
        "#,
    )
    .bind(address)
    .bind(network)
    .bind(payload.label)
    .fetch_one(&state.db)
    .await
    .map_err(|error| {
        if let Some(database_error) = error.as_database_error() {
            if database_error.is_unique_violation() {
                return (
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({
                        "error": "Wallet already exists",
                        "message": "This wallet is already registered on this network"
                    })),
                );
            }
        }

        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": "Database error",
                "message": "Failed to create wallet"
            })),
        )
    })?;

    Ok((StatusCode::CREATED, Json(wallet)))
}

pub async fn get_wallets<P>(
    State(state): State<AppState<P>>,
) -> Result<Json<Vec<Wallet>>, StatusCode>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    let wallets = sqlx::query_as::<_, Wallet>(
        r#"
        SELECT id, address, network, label, created_at, updated_at
        FROM wallets
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(wallets))
}

pub async fn get_wallet<P>(
    State(state): State<AppState<P>>,
    Path(id): Path<i64>,
) -> Result<Json<Wallet>, StatusCode>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    let wallet = sqlx::query_as::<_, Wallet>(
        r#"
        SELECT id, address, network, label, created_at, updated_at
        FROM wallets
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match wallet {
        Some(wallet) => Ok(Json(wallet)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

// ============================================================
// ETH BALANCE
// ============================================================

pub async fn get_wallet_balance<P>(
    State(state): State<AppState<P>>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, StatusCode>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    let wallet = sqlx::query_as::<_, Wallet>(
        r#"
        SELECT id, address, network, label, created_at, updated_at
        FROM wallets
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let wallet = match wallet {
        Some(wallet) => wallet,
        None => return Err(StatusCode::NOT_FOUND),
    };

    if wallet.network != "ethereum" {
        return Err(StatusCode::BAD_REQUEST);
    }

    let address = wallet
        .address
        .parse::<alloy_primitives::Address>()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let balance = crate::blockchain::get_eth_balance(state.ethereum.as_ref(), address)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    let balance_wei = balance.to_string();

    let balance_eth = wei_to_eth(&balance_wei);

    Ok(Json(serde_json::json!({
        "wallet_id": wallet.id,
        "address": wallet.address,
        "network": wallet.network,
        "balance_wei": balance_wei,
        "balance_eth": balance_eth
    })))
}

// ============================================================
// TOKENS
// ============================================================

pub async fn create_token<P>(
    State(state): State<AppState<P>>,
    Json(payload): Json<CreateToken>,
) -> Result<(StatusCode, Json<Token>), (StatusCode, Json<serde_json::Value>)>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    let network = payload.network.to_lowercase();

    if network != "ethereum" && network != "base" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Unsupported network",
                "message": "Supported networks are ethereum and base"
            })),
        ));
    }

    let contract_address = payload.contract_address.trim();

    if contract_address
        .parse::<alloy_primitives::Address>()
        .is_err()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Invalid contract address",
                "message": "Please provide a valid Ethereum-compatible contract address"
            })),
        ));
    }

    let token = sqlx::query_as::<_, Token>(
        r#"
        INSERT INTO tokens (
            symbol,
            name,
            contract_address,
            network,
            decimals
        )
        VALUES ($1, $2, $3, $4, $5)
        RETURNING
            id,
            symbol,
            name,
            contract_address,
            network,
            decimals,
            created_at,
            updated_at
        "#,
    )
    .bind(payload.symbol.to_uppercase())
    .bind(payload.name)
    .bind(contract_address)
    .bind(network)
    .bind(payload.decimals)
    .fetch_one(&state.db)
    .await
    .map_err(|error| {
        if let Some(database_error) = error.as_database_error() {
            if database_error.is_unique_violation() {
                return (
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({
                        "error": "Token already exists",
                        "message": "This token is already registered"
                    })),
                );
            }
        }

        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": "Database error",
                "message": "Failed to create token"
            })),
        )
    })?;

    Ok((StatusCode::CREATED, Json(token)))
}

pub async fn get_tokens<P>(State(state): State<AppState<P>>) -> Result<Json<Vec<Token>>, StatusCode>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    let tokens = sqlx::query_as::<_, Token>(
        r#"
        SELECT
            id,
            symbol,
            name,
            contract_address,
            network,
            decimals,
            created_at,
            updated_at
        FROM tokens
        ORDER BY symbol ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(tokens))
}

// ============================================================
// WALLET TOKEN BALANCES
// ============================================================

pub async fn get_wallet_tokens<P>(
    State(state): State<AppState<P>>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, StatusCode>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    let wallet = sqlx::query_as::<_, Wallet>(
        r#"
        SELECT id, address, network, label, created_at, updated_at
        FROM wallets
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let wallet = match wallet {
        Some(wallet) => wallet,
        None => return Err(StatusCode::NOT_FOUND),
    };

    let wallet_address = wallet
        .address
        .parse::<alloy_primitives::Address>()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let tokens = sqlx::query_as::<_, Token>(
        r#"
        SELECT
            id,
            symbol,
            name,
            contract_address,
            network,
            decimals,
            created_at,
            updated_at
        FROM tokens
        WHERE network = $1
        ORDER BY symbol ASC
        "#,
    )
    .bind(&wallet.network)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut token_balances = Vec::new();

    for token in tokens {
        let token_address = match token.contract_address.parse::<alloy_primitives::Address>() {
            Ok(address) => address,
            Err(_) => continue,
        };

        let raw_balance = crate::blockchain::erc20::get_token_balance(
            state.ethereum.as_ref(),
            token_address,
            wallet_address,
        )
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

        let raw_balance_string = raw_balance.to_string();

        let balance = format_token_balance(&raw_balance_string, token.decimals);

        token_balances.push(serde_json::json!({
            "token_id": token.id,
            "symbol": token.symbol,
            "name": token.name,
            "contract_address": token.contract_address,
            "decimals": token.decimals,
            "raw_balance": raw_balance_string,
            "balance": balance
        }));
    }

    Ok(Json(serde_json::json!({
        "wallet_id": wallet.id,
        "address": wallet.address,
        "network": wallet.network,
        "tokens": token_balances
    })))
}

// ============================================================
// TRANSACTIONS
// ============================================================

#[derive(Debug, Deserialize)]
pub struct TransactionQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub direction: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

pub async fn get_wallet_transactions<P>(
    State(state): State<AppState<P>>,
    Path(id): Path<i64>,
    Query(params): Query<TransactionQuery>,
) -> Result<Json<serde_json::Value>, StatusCode>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    let wallet = sqlx::query_as::<_, Wallet>(
        r#"
        SELECT
            id,
            address,
            network,
            label,
            created_at,
            updated_at
        FROM wallets
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let wallet = match wallet {
        Some(wallet) => wallet,
        None => return Err(StatusCode::NOT_FOUND),
    };

    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * limit;

    let direction = params.direction.as_deref();
    let status = params.status.as_deref();

    let total: i64 = {
        let mut builder = QueryBuilder::new("SELECT COUNT(*) FROM transactions WHERE wallet_id = ");

        builder.push_bind(wallet.id);

        if let Some(direction) = direction {
            if direction == "incoming" {
                builder.push(" AND LOWER(to_address) = LOWER(");
                builder.push_bind(&wallet.address);
                builder.push(")");
            } else if direction == "outgoing" {
                builder.push(" AND LOWER(from_address) = LOWER(");
                builder.push_bind(&wallet.address);
                builder.push(")");
            }
        }

        if let Some(status) = status {
            if status == "success" {
                builder.push(" AND is_error = false");
            } else if status == "failed" {
                builder.push(" AND is_error = true");
            }
        }

        builder
            .build_query_scalar()
            .fetch_one(&state.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    let transactions = {
        let mut builder = QueryBuilder::new(
            r#"
            SELECT
                id,
                wallet_id,
                hash,
                from_address,
                to_address,
                value_wei,
                block_number,
                timestamp,
                gas_used,
                gas_price_wei,
                is_error,
                created_at
            FROM transactions
            WHERE wallet_id =
            "#,
        );

        builder.push_bind(wallet.id);

        if let Some(direction) = direction {
            if direction == "incoming" {
                builder.push(" AND LOWER(to_address) = LOWER(");
                builder.push_bind(&wallet.address);
                builder.push(")");
            } else if direction == "outgoing" {
                builder.push(" AND LOWER(from_address) = LOWER(");
                builder.push_bind(&wallet.address);
                builder.push(")");
            }
        }

        if let Some(status) = status {
            if status == "success" {
                builder.push(" AND is_error = false");
            } else if status == "failed" {
                builder.push(" AND is_error = true");
            }
        }

        builder.push(" ORDER BY id ASC LIMIT ");
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        builder
            .build_query_as::<Transaction>()
            .fetch_all(&state.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    let mut response_transactions = Vec::new();

    for transaction in transactions {
        let value_eth = wei_to_eth(&transaction.value_wei.to_string());

        let gas_used = transaction
            .gas_used
            .as_ref()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "0".to_string());

        let gas_price_wei = transaction
            .gas_price_wei
            .as_ref()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "0".to_string());

        let gas_price_gwei = wei_to_gwei(&gas_price_wei);

        let gas_fee_eth = calculate_gas_fee_eth(&gas_used, &gas_price_wei);

        let direction = if transaction
            .to_address
            .as_deref()
            .map(|address| address.eq_ignore_ascii_case(&wallet.address))
            .unwrap_or(false)
        {
            "incoming"
        } else {
            "outgoing"
        };

        let status = if transaction.is_error {
            "failed"
        } else {
            "success"
        };

        response_transactions.push(serde_json::json!({
            "id": transaction.id,
            "wallet_id": transaction.wallet_id,
            "hash": transaction.hash,
            "from_address": transaction.from_address,
            "to_address": transaction.to_address,
            "value_eth": value_eth,
            "block_number": transaction.block_number,
            "timestamp": transaction.timestamp,
            "gas_used": gas_used,
            "gas_price_gwei": gas_price_gwei,
            "gas_fee_eth": gas_fee_eth,
            "status": status,
            "direction": direction
        }));
    }

    let pages = if total == 0 {
        0
    } else {
        (total + limit - 1) / limit
    };

    Ok(Json(serde_json::json!({
        "wallet_id": wallet.id,
        "page": page,
        "limit": limit,
        "total": total,
        "pages": pages,
        "transactions": response_transactions
    })))
}

pub async fn get_wallet_transaction<P>(
    State(state): State<AppState<P>>,
    Path((wallet_id, transaction_id)): Path<(i64, i64)>,
) -> Result<Json<serde_json::Value>, StatusCode>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    let wallet = sqlx::query_as::<_, Wallet>(
        r#"
        SELECT
            id,
            address,
            network,
            label,
            created_at,
            updated_at
        FROM wallets
        WHERE id = $1
        "#,
    )
    .bind(wallet_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let wallet = match wallet {
        Some(wallet) => wallet,
        None => return Err(StatusCode::NOT_FOUND),
    };

    let transaction = sqlx::query_as::<_, Transaction>(
        r#"
        SELECT
            id,
            wallet_id,
            hash,
            from_address,
            to_address,
            value_wei,
            block_number,
            timestamp,
            gas_used,
            gas_price_wei,
            is_error,
            created_at
        FROM transactions
        WHERE wallet_id = $1
          AND id = $2
        "#,
    )
    .bind(wallet.id)
    .bind(transaction_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let transaction = match transaction {
        Some(transaction) => transaction,
        None => return Err(StatusCode::NOT_FOUND),
    };

    let value_eth = wei_to_eth(&transaction.value_wei.to_string());

    let gas_used = transaction
        .gas_used
        .as_ref()
        .map(|value| value.to_string())
        .unwrap_or_else(|| "0".to_string());

    let gas_price_wei = transaction
        .gas_price_wei
        .as_ref()
        .map(|value| value.to_string())
        .unwrap_or_else(|| "0".to_string());

    let gas_price_gwei = wei_to_gwei(&gas_price_wei);

    let gas_fee_eth = calculate_gas_fee_eth(&gas_used, &gas_price_wei);

    let direction = if transaction
        .to_address
        .as_deref()
        .map(|address| address.eq_ignore_ascii_case(&wallet.address))
        .unwrap_or(false)
    {
        "incoming"
    } else {
        "outgoing"
    };

    let status = if transaction.is_error {
        "failed"
    } else {
        "success"
    };

    Ok(Json(serde_json::json!({
        "id": transaction.id,
        "wallet_id": transaction.wallet_id,
        "hash": transaction.hash,
        "from_address": transaction.from_address,
        "to_address": transaction.to_address,
        "value_eth": value_eth,
        "block_number": transaction.block_number,
        "timestamp": transaction.timestamp,
        "gas_used": gas_used,
        "gas_price_gwei": gas_price_gwei,
        "gas_fee_eth": gas_fee_eth,
        "status": status,
        "direction": direction
    })))
}

// ============================================================
// ANALYTICS
// ============================================================

pub async fn get_wallet_analytics<P>(
    State(state): State<AppState<P>>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, StatusCode>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    let wallet = sqlx::query_as::<_, Wallet>(
        r#"
        SELECT
            id,
            address,
            network,
            label,
            created_at,
            updated_at
        FROM wallets
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let wallet = match wallet {
        Some(wallet) => wallet,
        None => return Err(StatusCode::NOT_FOUND),
    };

    let address = wallet
        .address
        .parse::<alloy_primitives::Address>()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let balance = crate::blockchain::get_eth_balance(state.ethereum.as_ref(), address)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    let stats = sqlx::query(
        r#"
        SELECT
            COALESCE(
                SUM(
                    CASE
                        WHEN LOWER(to_address) = LOWER($1)
                        THEN value_wei
                        ELSE 0
                    END
                ),
                0
            ) AS total_received_wei,

            COALESCE(
                SUM(
                    CASE
                        WHEN LOWER(from_address) = LOWER($1)
                        THEN value_wei
                        ELSE 0
                    END
                ),
                0
            ) AS total_sent_wei,

            COALESCE(
                SUM(
                    CASE
                        WHEN LOWER(from_address) = LOWER($1)
                        THEN COALESCE(gas_used, 0) * COALESCE(gas_price_wei, 0)
                        ELSE 0
                    END
                ),
                0
            ) AS total_gas_spent_wei,

            COUNT(*) AS transaction_count,

            COUNT(
                CASE
                    WHEN LOWER(to_address) = LOWER($1)
                    THEN 1
                END
            ) AS incoming_count,

            COUNT(
                CASE
                    WHEN LOWER(from_address) = LOWER($1)
                    THEN 1
                END
            ) AS outgoing_count

        FROM transactions
        WHERE wallet_id = $2
        "#,
    )
    .bind(&wallet.address)
    .bind(wallet.id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    use sqlx::Row;

    let total_received_wei = stats
        .try_get::<bigdecimal::BigDecimal, _>("total_received_wei")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let total_sent_wei = stats
        .try_get::<bigdecimal::BigDecimal, _>("total_sent_wei")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let total_gas_spent_wei = stats
        .try_get::<bigdecimal::BigDecimal, _>("total_gas_spent_wei")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let transaction_count = stats
        .try_get::<i64, _>("transaction_count")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let incoming_count = stats
        .try_get::<i64, _>("incoming_count")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let outgoing_count = stats
        .try_get::<i64, _>("outgoing_count")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({
        "wallet_id": wallet.id,
        "address": wallet.address,
        "network": wallet.network,
        "balance_eth": wei_to_eth(&balance.to_string()),
        "transaction_count": transaction_count,
        "incoming_count": incoming_count,
        "outgoing_count": outgoing_count,
        "total_received_eth": wei_to_eth(
            &total_received_wei.to_string()
        ),
        "total_sent_eth": wei_to_eth(
            &total_sent_wei.to_string()
        ),
        "total_gas_spent_eth": wei_to_eth(
            &total_gas_spent_wei.to_string()
        )
    })))
}

// ============================================================
// PORTFOLIO
// ============================================================

pub async fn get_wallet_portfolio<P>(
    State(state): State<AppState<P>>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, StatusCode>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    let wallet = sqlx::query_as::<_, Wallet>(
        r#"
        SELECT
            id,
            address,
            network,
            label,
            created_at,
            updated_at
        FROM wallets
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let wallet = match wallet {
        Some(wallet) => wallet,
        None => return Err(StatusCode::NOT_FOUND),
    };

    if wallet.network != "ethereum" {
        return Err(StatusCode::BAD_REQUEST);
    }

    let prices = crate::blockchain::market::get_prices(&state.http_client)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    let address = wallet
        .address
        .parse::<alloy_primitives::Address>()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let eth_balance = crate::blockchain::get_eth_balance(state.ethereum.as_ref(), address)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    let eth_balance_string = wei_to_eth(&eth_balance.to_string());

    let eth_price = prices
        .ethereum
        .as_ref()
        .map(|price| price.usd)
        .unwrap_or(0.0);

    let eth_24h_change = prices
        .ethereum
        .as_ref()
        .and_then(|price| price.usd_24h_change)
        .unwrap_or(0.0);

    let eth_balance_f64 = eth_balance_string.parse::<f64>().unwrap_or(0.0);

    let eth_value_usd = eth_balance_f64 * eth_price;

    let tokens = sqlx::query_as::<_, Token>(
        r#"
        SELECT
            id,
            symbol,
            name,
            contract_address,
            network,
            decimals,
            created_at,
            updated_at
        FROM tokens
        WHERE network = $1
        ORDER BY symbol ASC
        "#,
    )
    .bind(&wallet.network)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut assets: Vec<serde_json::Value> = Vec::new();

    assets.push(serde_json::json!({
        "type": "native",
        "symbol": "ETH",
        "name": "Ethereum",
        "balance": eth_balance_string,
        "decimals": 18,
        "price_usd": eth_price,
        "value_usd": eth_value_usd,
        "change_24h_percent": eth_24h_change
    }));

    for token in tokens {
        let token_address = match token.contract_address.parse::<alloy_primitives::Address>() {
            Ok(address) => address,
            Err(_) => continue,
        };

        let raw_balance = crate::blockchain::erc20::get_token_balance(
            state.ethereum.as_ref(),
            token_address,
            address,
        )
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

        let raw_balance_string = raw_balance.to_string();

        let formatted_balance = format_token_balance(&raw_balance_string, token.decimals);

        let token_price = match token.symbol.to_uppercase().as_str() {
            "USDC" => prices
                .usd_coin
                .as_ref()
                .map(|price| price.usd)
                .unwrap_or(0.0),

            _ => 0.0,
        };

        let token_24h_change = match token.symbol.to_uppercase().as_str() {
            "USDC" => prices
                .usd_coin
                .as_ref()
                .and_then(|price| price.usd_24h_change)
                .unwrap_or(0.0),

            _ => 0.0,
        };

        let token_balance_f64 = formatted_balance.parse::<f64>().unwrap_or(0.0);

        let token_value_usd = token_balance_f64 * token_price;

        assets.push(serde_json::json!({
            "type": "token",
            "token_id": token.id,
            "symbol": token.symbol,
            "name": token.name,
            "contract_address": token.contract_address,
            "balance": formatted_balance,
            "raw_balance": raw_balance_string,
            "decimals": token.decimals,
            "price_usd": token_price,
            "value_usd": token_value_usd,
            "change_24h_percent": token_24h_change
        }));
    }

    let total_value_usd = assets
        .iter()
        .map(|asset| {
            asset
                .get("value_usd")
                .and_then(|value| value.as_f64())
                .unwrap_or(0.0)
        })
        .sum::<f64>();

    for asset in &mut assets {
        let value = asset
            .get("value_usd")
            .and_then(|value| value.as_f64())
            .unwrap_or(0.0);

        let allocation_percent = if total_value_usd > 0.0 {
            (value / total_value_usd) * 100.0
        } else {
            0.0
        };

        if let Some(object) = asset.as_object_mut() {
            object.insert(
                "allocation_percent".to_string(),
                serde_json::json!(allocation_percent),
            );
        }
    }

    save_portfolio_snapshot(&state.db, wallet.id, total_value_usd)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({
        "wallet_id": wallet.id,
        "address": wallet.address,
        "network": wallet.network,
        "total_value_usd": total_value_usd,
        "assets": assets
    })))
}

// ============================================================
// HELPERS
// ============================================================

fn wei_to_eth(value: &str) -> String {
    if value == "0" {
        return "0".to_string();
    }

    if value.len() <= 18 {
        let decimal = format!("{:0>18}", value).trim_end_matches('0').to_string();

        if decimal.is_empty() {
            "0".to_string()
        } else {
            format!("0.{}", decimal)
        }
    } else {
        let split_position = value.len() - 18;

        let (whole, decimal) = value.split_at(split_position);

        let decimal = decimal.trim_end_matches('0');

        if decimal.is_empty() {
            whole.to_string()
        } else {
            format!("{}.{}", whole, decimal)
        }
    }
}

fn wei_to_gwei(value: &str) -> String {
    if value == "0" {
        return "0".to_string();
    }

    if value.len() <= 9 {
        let decimal = format!("{:0>9}", value).trim_end_matches('0').to_string();

        if decimal.is_empty() {
            "0".to_string()
        } else {
            format!("0.{}", decimal)
        }
    } else {
        let split_position = value.len() - 9;

        let (whole, decimal) = value.split_at(split_position);

        let decimal = decimal.trim_end_matches('0');

        if decimal.is_empty() {
            whole.to_string()
        } else {
            format!("{}.{}", whole, decimal)
        }
    }
}

fn calculate_gas_fee_eth(gas_used: &str, gas_price_wei: &str) -> String {
    let gas_used = gas_used.parse::<bigdecimal::BigDecimal>();

    let gas_price = gas_price_wei.parse::<bigdecimal::BigDecimal>();

    match (gas_used, gas_price) {
        (Ok(gas_used), Ok(gas_price)) => {
            let fee_wei = gas_used * gas_price;

            wei_to_eth(&fee_wei.to_string())
        }

        _ => "0".to_string(),
    }
}

fn format_token_balance(raw_balance: &str, decimals: i16) -> String {
    let decimals = decimals.max(0) as usize;

    if decimals == 0 {
        return raw_balance.to_string();
    }

    if raw_balance.len() <= decimals {
        let decimal = format!("{:0>width$}", raw_balance, width = decimals)
            .trim_end_matches('0')
            .to_string();

        if decimal.is_empty() {
            "0".to_string()
        } else {
            format!("0.{}", decimal)
        }
    } else {
        let split_position = raw_balance.len() - decimals;

        let (whole, decimal) = raw_balance.split_at(split_position);

        let decimal = decimal.trim_end_matches('0');

        if decimal.is_empty() {
            whole.to_string()
        } else {
            format!("{}.{}", whole, decimal)
        }
    }
}

pub async fn calculate_portfolio_value<P>(
    state: &AppState<P>,
    wallet_id: i64,
) -> Result<f64, Box<dyn std::error::Error + Send + Sync>>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    let portfolio = get_wallet_portfolio(State(state.clone()), Path(wallet_id))
        .await
        .map_err(|status| format!("Portfolio request failed: {}", status))?;

    let Json(data) = portfolio;

    let total_value = data
        .get("total_value_usd")
        .and_then(|value| value.as_f64())
        .ok_or("total_value_usd missing from portfolio response")?;

    Ok(total_value)
}

pub async fn save_portfolio_snapshot(
    pool: &sqlx::PgPool,
    wallet_id: i64,
    total_value_usd: f64,
) -> Result<(), sqlx::Error> {
    let latest_snapshot = sqlx::query_scalar::<_, chrono::DateTime<chrono::Utc>>(
        r#"
        SELECT created_at
        FROM portfolio_snapshots
        WHERE wallet_id = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(wallet_id)
    .fetch_optional(pool)
    .await?;

    if let Some(created_at) = latest_snapshot {
        let elapsed = chrono::Utc::now() - created_at;

        if elapsed < chrono::Duration::minutes(5) {
            return Ok(());
        }
    }

    sqlx::query(
        r#"
        INSERT INTO portfolio_snapshots (
            wallet_id,
            total_value_usd
        )
        VALUES ($1, $2)
        "#,
    )
    .bind(wallet_id)
    .bind(total_value_usd)
    .execute(pool)
    .await?;

    Ok(())
}

// ============================================================
// PORTFOLIO HISTORY
// ============================================================

pub async fn get_portfolio_history<P>(
    State(state): State<AppState<P>>,
    Path(id): Path<i64>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<serde_json::Value>, StatusCode>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    let wallet = sqlx::query_as::<_, Wallet>(
        r#"
        SELECT
            id,
            address,
            network,
            label,
            created_at,
            updated_at
        FROM wallets
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let wallet = match wallet {
        Some(wallet) => wallet,
        None => return Err(StatusCode::NOT_FOUND),
    };

    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(20).clamp(1, 100);

    let offset = (page - 1) * limit;

    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM portfolio_snapshots
        WHERE wallet_id = $1
        "#,
    )
    .bind(wallet.id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let snapshots = sqlx::query_as::<_, PortfolioSnapshot>(
        r#"
        SELECT
            id,
            wallet_id,
            total_value_usd,
            created_at
        FROM portfolio_snapshots
        WHERE wallet_id = $1
        ORDER BY created_at ASC
        LIMIT $2
        OFFSET $3
        "#,
    )
    .bind(wallet.id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let pages = if total == 0 {
        0
    } else {
        (total + limit - 1) / limit
    };

    Ok(Json(serde_json::json!({
        "wallet_id": wallet.id,
        "address": wallet.address,
        "network": wallet.network,
        "page": page,
        "limit": limit,
        "total": total,
        "pages": pages,
        "history": snapshots
    })))
}

pub async fn get_portfolio_chart<P>(
    State(state): State<AppState<P>>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, StatusCode>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    let wallet = sqlx::query_as::<_, Wallet>(
        r#"
        SELECT
            id,
            address,
            network,
            label,
            created_at,
            updated_at
        FROM wallets
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let wallet = match wallet {
        Some(wallet) => wallet,
        None => return Err(StatusCode::NOT_FOUND),
    };

    let snapshots = sqlx::query_as::<_, PortfolioSnapshot>(
        r#"
        SELECT
            id,
            wallet_id,
            total_value_usd,
            created_at
        FROM portfolio_snapshots
        WHERE wallet_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(wallet.id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let data: Vec<serde_json::Value> = snapshots
        .iter()
        .map(|snapshot| {
            serde_json::json!({
                "timestamp": snapshot.created_at,
                "value_usd": snapshot.total_value_usd.to_string()
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "wallet_id": wallet.id,
        "address": wallet.address,
        "network": wallet.network,
        "data": data
    })))
}

pub async fn get_portfolio_performance<P>(
    State(state): State<AppState<P>>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, StatusCode>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    let wallet = sqlx::query_as::<_, Wallet>(
        r#"
        SELECT
            id,
            address,
            network,
            label,
            created_at,
            updated_at
        FROM wallets
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let wallet = match wallet {
        Some(wallet) => wallet,
        None => return Err(StatusCode::NOT_FOUND),
    };

    let snapshots = sqlx::query_as::<_, PortfolioSnapshot>(
        r#"
        SELECT
            id,
            wallet_id,
            total_value_usd,
            created_at
        FROM portfolio_snapshots
        WHERE wallet_id = $1
        ORDER BY created_at DESC
        LIMIT 2
        "#,
    )
    .bind(wallet.id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if snapshots.is_empty() {
        return Ok(Json(serde_json::json!({
            "wallet_id": wallet.id,
            "address": wallet.address,
            "network": wallet.network,
            "current_value_usd": "0.00",
            "previous_value_usd": null,
            "change_usd": "0.00",
            "change_percent": "0.0000",
            "has_previous_snapshot": false
        })));
    }

    let current = &snapshots[0];

    let current_value = current.total_value_usd.clone();

    let (previous_value, change_usd, change_percent) = if snapshots.len() > 1 {
        let previous = &snapshots[1];

        let previous_value = previous.total_value_usd.clone();

        let change_usd = &current_value - &previous_value;

        let change_percent = if previous_value == bigdecimal::BigDecimal::from(0) {
            bigdecimal::BigDecimal::from(0)
        } else {
            (&change_usd / &previous_value) * bigdecimal::BigDecimal::from(100)
        };

        (Some(previous_value), change_usd, change_percent)
    } else {
        (
            None,
            bigdecimal::BigDecimal::from(0),
            bigdecimal::BigDecimal::from(0),
        )
    };

    let current_value_rounded = current_value.round(2).to_string();

    let previous_value_rounded = previous_value.map(|value| value.round(2).to_string());

    let change_usd_rounded = change_usd.round(2).to_string();

    let change_percent_rounded = change_percent.round(4).to_string();

    Ok(Json(serde_json::json!({
        "wallet_id": wallet.id,
        "address": wallet.address,
        "network": wallet.network,
        "current_value_usd": current_value_rounded,
        "previous_value_usd": previous_value_rounded,
        "change_usd": change_usd_rounded,
        "change_percent": change_percent_rounded,
        "has_previous_snapshot": snapshots.len() > 1
    })))
}
