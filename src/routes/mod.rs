use alloy_provider::Provider;
use axum::{
    Router,
    routing::{get, post},
};
use tower_http::cors::CorsLayer;

use crate::handlers::{
    create_token, create_wallet, get_portfolio_chart, get_portfolio_history,
    get_portfolio_performance, get_tokens, get_wallet, get_wallet_analytics, get_wallet_balance,
    get_wallet_portfolio, get_wallet_tokens, get_wallet_transaction, get_wallet_transactions,
    get_wallets, health_check,
};

use crate::models::AppState;

pub fn create_router<P>(state: AppState<P>) -> Router
where
    P: Provider + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/health", get(health_check::<P>))
        .route(
            "/api/wallets",
            post(create_wallet::<P>).get(get_wallets::<P>),
        )
        .route("/api/wallets/{id}", get(get_wallet::<P>))
        .route("/api/wallets/{id}/balance", get(get_wallet_balance::<P>))
        .route("/api/tokens", post(create_token::<P>).get(get_tokens::<P>))
        .route("/api/wallets/{id}/tokens", get(get_wallet_tokens::<P>))
        .route(
            "/api/wallets/{id}/transactions",
            get(get_wallet_transactions::<P>),
        )
        .route(
            "/api/wallets/{wallet_id}/transactions/{transaction_id}",
            get(get_wallet_transaction::<P>),
        )
        .route(
            "/api/wallets/{id}/analytics",
            get(get_wallet_analytics::<P>),
        )
        .route(
            "/api/wallets/{id}/portfolio",
            get(get_wallet_portfolio::<P>),
        )
        .route(
            "/api/wallets/{id}/portfolio/history",
            get(get_portfolio_history::<P>),
        )
        .route(
            "/api/wallets/{id}/portfolio/performance",
            get(get_portfolio_performance::<P>),
        )
        .route(
            "/api/wallets/{id}/portfolio/chart",
            get(get_portfolio_chart::<P>),
        )
        .layer(CorsLayer::permissive())
        .with_state(state)
}
