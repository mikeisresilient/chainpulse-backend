use reqwest::Client;
use serde::Deserialize;
use sqlx::PgPool;
use std::error::Error;
use tokio::time::{Duration, sleep};

#[derive(Debug, Deserialize)]
pub struct CoinGeckoSimplePrice {
    pub ethereum: Option<CoinPrice>,

    #[serde(rename = "usd-coin")]
    pub usd_coin: Option<CoinPrice>,
}

#[derive(Debug, Deserialize)]
pub struct CoinPrice {
    pub usd: f64,

    #[serde(rename = "usd_24h_change")]
    pub usd_24h_change: Option<f64>,
}

pub async fn get_prices(
    client: &Client,
) -> Result<CoinGeckoSimplePrice, Box<dyn Error + Send + Sync>> {
    let url = "https://api.coingecko.com/api/v3/simple/price?ids=ethereum,usd-coin&vs_currencies=usd&include_24hr_change=true";

    let mut last_error = String::new();

    for attempt in 1..=3 {
        let response = client
            .get(url)
            .header(reqwest::header::USER_AGENT, "ChainPulse/0.1")
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await;

        match response {
            Ok(response) => {
                if response.status().is_success() {
                    let prices = response.json::<CoinGeckoSimplePrice>().await?;
                    return Ok(prices);
                }

                let status = response.status();

                if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    last_error = format!("CoinGecko rate limit reached (attempt {}/3)", attempt);

                    if attempt < 3 {
                        sleep(Duration::from_secs(2 * attempt as u64)).await;
                        continue;
                    }
                } else {
                    last_error = format!("CoinGecko returned HTTP status {}", status);
                    break;
                }
            }

            Err(error) => {
                last_error = format!(
                    "CoinGecko request failed (attempt {}/3): {}",
                    attempt, error
                );

                if attempt < 3 {
                    sleep(Duration::from_secs(2 * attempt as u64)).await;
                }
            }
        }
    }

    Err(last_error.into())
}

pub async fn get_prices_with_cache(
    client: &Client,
    pool: &PgPool,
) -> Result<CoinGeckoSimplePrice, Box<dyn Error + Send + Sync>> {
    match get_prices(client).await {
        Ok(prices) => {
            save_prices_to_cache(pool, &prices).await?;
            Ok(prices)
        }

        Err(error) => {
            eprintln!(
                "CoinGecko unavailable: {}. Attempting to use cached prices.",
                error
            );

            match load_prices_from_cache(pool).await {
                Ok(prices) => {
                    println!("Using cached market prices.");
                    Ok(prices)
                }

                Err(cache_error) => Err(format!(
                    "CoinGecko unavailable and no cached prices exist: {}",
                    cache_error
                )
                .into()),
            }
        }
    }
}

async fn save_prices_to_cache(
    pool: &PgPool,
    prices: &CoinGeckoSimplePrice,
) -> Result<(), sqlx::Error> {
    if let Some(ethereum) = &prices.ethereum {
        sqlx::query(
            r#"
            INSERT INTO market_prices (
                symbol,
                price_usd,
                change_24h_percent,
                updated_at
            )
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (symbol)
            DO UPDATE SET
                price_usd = EXCLUDED.price_usd,
                change_24h_percent = EXCLUDED.change_24h_percent,
                updated_at = NOW()
            "#,
        )
        .bind("ETH")
        .bind(ethereum.usd)
        .bind(ethereum.usd_24h_change.unwrap_or(0.0))
        .execute(pool)
        .await?;
    }

    if let Some(usdc) = &prices.usd_coin {
        sqlx::query(
            r#"
            INSERT INTO market_prices (
                symbol,
                price_usd,
                change_24h_percent,
                updated_at
            )
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (symbol)
            DO UPDATE SET
                price_usd = EXCLUDED.price_usd,
                change_24h_percent = EXCLUDED.change_24h_percent,
                updated_at = NOW()
            "#,
        )
        .bind("USDC")
        .bind(usdc.usd)
        .bind(usdc.usd_24h_change.unwrap_or(0.0))
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn load_prices_from_cache(
    pool: &PgPool,
) -> Result<CoinGeckoSimplePrice, Box<dyn Error + Send + Sync>> {
    let rows = sqlx::query_as::<_, (String, f64, f64)>(
        r#"
        SELECT
            symbol,
            price_usd,
            change_24h_percent
        FROM market_prices
        WHERE symbol IN ('ETH', 'USDC')
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut ethereum = None;
    let mut usd_coin = None;

    for (symbol, price_usd, change_24h_percent) in rows {
        match symbol.as_str() {
            "ETH" => {
                ethereum = Some(CoinPrice {
                    usd: price_usd,
                    usd_24h_change: Some(change_24h_percent),
                });
            }

            "USDC" => {
                usd_coin = Some(CoinPrice {
                    usd: price_usd,
                    usd_24h_change: Some(change_24h_percent),
                });
            }

            _ => {}
        }
    }

    if ethereum.is_none() && usd_coin.is_none() {
        return Err("No cached market prices available".into());
    }

    Ok(CoinGeckoSimplePrice { ethereum, usd_coin })
}
