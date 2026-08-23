use reqwest::Client;
use serde::Deserialize;
use std::error::Error;

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

    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, "ChainPulse/0.1")
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await?
        .error_for_status()?
        .json::<CoinGeckoSimplePrice>()
        .await?;

    Ok(response)
}
