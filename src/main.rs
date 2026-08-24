mod blockchain;
mod config;
mod db;
mod errors;
mod handlers;
mod models;
mod routes;

use sqlx;
use std::sync::Arc;

use blockchain::{create_ethereum_provider, get_latest_block};
use config::Config;
use models::AppState;
use tokio::time::{interval, Duration};

#[tokio::main]
async fn main() {
    let config = Config::from_env();

    let db_pool = db::create_pool(&config.database_url).await;

    println!("Connected to PostgreSQL successfully.");

    let ethereum_provider =
        create_ethereum_provider(&config.ethereum_rpc_url).await;

    println!("Connected to Ethereum RPC successfully.");

    let http_client = reqwest::Client::new();

    match blockchain::market::get_prices(&http_client).await {
        Ok(prices) => {
            if let Some(ethereum) = prices.ethereum {
                println!("ETH price: ${}", ethereum.usd);
            }

            if let Some(usdc) = prices.usd_coin {
                println!("USDC price: ${}", usdc.usd);
            }
        }

        Err(error) => {
            println!("Failed to retrieve market prices: {}", error);
        }
    }

    /*
     * Sync transactions for all wallets currently stored
     * in the database.
     */
    let wallets = match sqlx::query_as::<_, (i64, String)>(
        r#"
        SELECT id, address
        FROM wallets
        ORDER BY id ASC
        "#,
    )
    .fetch_all(&db_pool)
    .await
    {
        Ok(wallets) => wallets,

        Err(error) => {
            eprintln!("Failed to retrieve wallets: {}", error);
            Vec::new()
        }
    };

    println!(
        "Found {} wallet(s) for transaction synchronization.",
        wallets.len()
    );

    for (wallet_id, address) in wallets {
        println!(
            "Syncing transactions for wallet {} ({})...",
            wallet_id, address
        );

        match blockchain::etherscan::get_transactions(
            &http_client,
            &config.etherscan_api_key,
            &address,
        )
        .await
        {
            Ok(transactions) => {
                println!(
                    "Etherscan transactions retrieved for wallet {}: {}",
                    wallet_id,
                    transactions.len()
                );

                match blockchain::etherscan::sync_transactions(
                    &db_pool,
                    wallet_id,
                    &transactions,
                )
                .await
                {
                    Ok(inserted) => {
                        println!(
                            "Transactions inserted for wallet {}: {}",
                            wallet_id,
                            inserted
                        );
                    }

                    Err(error) => {
                        eprintln!(
                            "Failed to sync transactions for wallet {}: {}",
                            wallet_id,
                            error
                        );
                    }
                }
            }

            Err(error) => {
                eprintln!(
                    "Failed to retrieve Etherscan transactions for wallet {}: {}",
                    wallet_id,
                    error
                );
            }
        }
    }

    match get_latest_block(&ethereum_provider).await {
        Ok(block_number) => {
            println!("Latest Ethereum block: {}", block_number);
        }

        Err(error) => {
            println!("Failed to get latest Ethereum block: {}", error);
        }
    }

    let state = AppState {
        db: db_pool,
        ethereum: Arc::new(ethereum_provider),
        http_client,
    };

    /*
     * Background portfolio snapshot worker.
     *
     * Runs every 5 minutes and creates a snapshot
     * for every wallet stored in the database.
     */
    let snapshot_state = state.clone();

    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(300));

        loop {
            ticker.tick().await;

            let wallets = match sqlx::query_as::<_, (i64,)>(
                r#"
                SELECT id
                FROM wallets
                ORDER BY id ASC
                "#,
            )
            .fetch_all(&snapshot_state.db)
            .await
            {
                Ok(wallets) => wallets,

                Err(error) => {
                    eprintln!(
                        "Failed to retrieve wallets for snapshots: {}",
                        error
                    );

                    continue;
                }
            };

            println!(
                "Starting portfolio snapshot check for {} wallet(s)...",
                wallets.len()
            );

            for (wallet_id,) in wallets {
                match handlers::calculate_portfolio_value(
                    &snapshot_state,
                    wallet_id,
                )
                .await
                {
                    Ok(total_value_usd) => {
                        match handlers::save_portfolio_snapshot(
                            &snapshot_state.db,
                            wallet_id,
                            total_value_usd,
                        )
                        .await
                        {
                            Ok(_) => {
                                println!(
                                    "Portfolio snapshot completed: wallet_id={}, value=${}",
                                    wallet_id,
                                    total_value_usd
                                );
                            }

                            Err(error) => {
                                eprintln!(
                                    "Failed to save snapshot for wallet {}: {}",
                                    wallet_id,
                                    error
                                );
                            }
                        }
                    }

                    Err(error) => {
                        eprintln!(
                            "Failed to calculate portfolio for wallet {}: {}",
                            wallet_id,
                            error
                        );
                    }
                }
            }
        }
    });

    let app = routes::create_router(state);

    let address = format!(
        "{}:{}",
        config.server_host,
        config.server_port
    );

    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .expect("Failed to bind server");

    println!(
        "ChainPulse API running on http://{}",
        address
    );

    axum::serve(listener, app)
        .await
        .expect("Server failed");
}