pub mod erc20;
pub mod etherscan;
pub mod market;
use alloy_provider::{Provider, ProviderBuilder};

pub async fn create_ethereum_provider(rpc_url: &str) -> impl Provider + Clone + 'static {
    ProviderBuilder::new()
        .connect(rpc_url)
        .await
        .expect("Failed to connect to Ethereum RPC")
}

pub async fn get_latest_block<P: Provider>(
    provider: &P,
) -> Result<u64, Box<dyn std::error::Error>> {
    let block_number = provider.get_block_number().await?;

    Ok(block_number)
}

pub async fn get_eth_balance<P: Provider>(
    provider: &P,
    address: alloy_primitives::Address,
) -> Result<alloy_primitives::U256, Box<dyn std::error::Error>> {
    let balance = provider.get_balance(address).await?;

    Ok(balance)
}

pub async fn get_erc20_balance<P: Provider>(
    provider: &P,
    token_address: alloy_primitives::Address,
    wallet_address: alloy_primitives::Address,
) -> Result<alloy_primitives::U256, Box<dyn std::error::Error>> {
    let contract = erc20::IERC20::new(token_address, provider);

    let balance = contract.balanceOf(wallet_address).call().await?;

    Ok(balance)
}

pub async fn get_erc20_metadata<P: Provider>(
    provider: &P,
    token_address: alloy_primitives::Address,
) -> Result<(String, u8), Box<dyn std::error::Error>> {
    let contract = erc20::IERC20::new(token_address, provider);

    let symbol = contract.symbol().call().await?;

    let decimals = contract.decimals().call().await?;

    Ok((symbol, decimals))
}
