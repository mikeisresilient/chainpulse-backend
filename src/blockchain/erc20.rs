use alloy_primitives::{Address, U256};
use alloy_provider::Provider;
use alloy_sol_types::sol;

sol! {
    #[sol(rpc)]
    interface IERC20 {
        function balanceOf(address account) external view returns (uint256);
        function decimals() external view returns (uint8);
        function symbol() external view returns (string);
    }
}

pub async fn get_token_balance<P>(
    provider: &P,
    token_address: Address,
    wallet_address: Address,
) -> Result<U256, Box<dyn std::error::Error + Send + Sync>>
where
    P: Provider + Clone + Send + Sync + 'static,
{
    let contract = IERC20::new(token_address, provider);

    let balance = contract.balanceOf(wallet_address).call().await?;

    Ok(balance)
}
