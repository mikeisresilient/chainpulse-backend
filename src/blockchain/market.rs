use reqwest::Client;
use serde::Deserialize;
use std::error::Error;
use tokio::time::{sleep, Duration};

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
                    last_error = format!(
                        "CoinGecko rate limit reached (attempt {}/3)",
                        attempt
                    );

                    if attempt < 3 {
                        sleep(Duration::from_secs(2 * attempt as u64)).await;
                        continue;
                    }
                } else {
                    last_error = format!(
                        "CoinGecko returned HTTP status {}",
                        status
                    );
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