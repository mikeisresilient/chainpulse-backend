use std::env;

pub struct Config {
    pub server_host: String,
    pub server_port: u16,
    pub database_url: String,
    pub ethereum_rpc_url: String,
    pub base_rpc_url: String,
    pub etherscan_api_key: String,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        let server_host = env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());

        let server_port = env::var("SERVER_PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse::<u16>()
            .expect("SERVER_PORT must be a valid number");

        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

        let ethereum_rpc_url = env::var("ETHEREUM_RPC_URL").expect("ETHEREUM_RPC_URL must be set");

        let base_rpc_url = env::var("BASE_RPC_URL").expect("BASE_RPC_URL must be set");

        let etherscan_api_key =
            env::var("ETHERSCAN_API_KEY").expect("ETHERSCAN_API_KEY must be set");

        Self {
            server_host,
            server_port,
            database_url,
            ethereum_rpc_url,
            base_rpc_url,
            etherscan_api_key,
        }
    }
}
