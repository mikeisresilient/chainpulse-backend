use reqwest::Client;
use serde::Deserialize;
use sqlx::PgPool;

#[derive(Debug, Deserialize)]
pub struct EtherscanResponse {
    pub status: String,
    pub message: String,
    pub result: Vec<EtherscanTransaction>,
}

#[derive(Debug, Deserialize)]
pub struct EtherscanTransaction {
    pub hash: String,
    pub from: String,
    pub to: String,
    pub value: String,

    #[serde(rename = "blockNumber")]
    pub block_number: String,

    #[serde(rename = "timeStamp")]
    pub time_stamp: String,

    pub gas: String,

    #[serde(rename = "gasPrice")]
    pub gas_price: String,

    #[serde(rename = "gasUsed")]
    pub gas_used: String,

    #[serde(rename = "isError")]
    pub is_error: String,
}

pub async fn get_transactions(
    client: &Client,
    api_key: &str,
    address: &str,
) -> Result<Vec<EtherscanTransaction>, Box<dyn std::error::Error>> {
    let url = format!(
        "https://api.etherscan.io/v2/api?chainid=1&module=account&action=txlist&address={}&startblock=0&endblock=99999999&page=1&offset=20&sort=desc&apikey={}",
        address, api_key
    );

    let response = client
        .get(&url)
        .send()
        .await?
        .json::<EtherscanResponse>()
        .await?;

    if response.status != "1" {
        return Err(format!("Etherscan error: {}", response.message).into());
    }

    Ok(response.result)
}

pub async fn sync_transactions(
    pool: &PgPool,
    wallet_id: i64,
    transactions: &[EtherscanTransaction],
) -> Result<usize, Box<dyn std::error::Error>> {
    let mut inserted = 0;

    for transaction in transactions {
        let result = sqlx::query(
            r#"
            INSERT INTO transactions (
                wallet_id,
                hash,
                from_address,
                to_address,
                value_wei,
                block_number,
                timestamp,
                gas_used,
                gas_price_wei,
                is_error
            )
            VALUES (
                $1,
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                $8,
                $9,
                $10
            )
            ON CONFLICT (wallet_id, hash) DO NOTHING
            "#,
        )
        .bind(wallet_id)
        .bind(&transaction.hash)
        .bind(&transaction.from)
        .bind(if transaction.to.is_empty() {
            None
        } else {
            Some(transaction.to.as_str())
        })
        .bind(transaction.value.parse::<bigdecimal::BigDecimal>()?)
        .bind(transaction.block_number.parse::<i64>()?)
        .bind(transaction.time_stamp.parse::<i64>()?)
        .bind(if transaction.gas_used.is_empty() {
            None
        } else {
            Some(transaction.gas_used.parse::<bigdecimal::BigDecimal>()?)
        })
        .bind(if transaction.gas_price.is_empty() {
            None
        } else {
            Some(transaction.gas_price.parse::<bigdecimal::BigDecimal>()?)
        })
        .bind(transaction.is_error == "1")
        .execute(pool)
        .await?;

        inserted += result.rows_affected() as usize;
    }

    Ok(inserted)
}
