use duino_miner::error::MinerError;

use serde::{Deserialize, Serialize};

use rand::Rng;

pub fn generate_8hex() -> String {
    const HEX_ARRAY: [char; 16] = [
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
    ];

    let mut result = String::new();

    for _ in 0..8 {
        let n: usize = rand::thread_rng().gen_range(0..16);
        result.push(HEX_ARRAY[n]);
    }

    result
}

pub async fn get_pool_info() -> Result<String, MinerError> {
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Pool {
        pub name: String,
        pub ip: String,
        pub port: u16,
        pub connections: u32,
    }

    let pool: Pool = reqwest::get("http://51.15.127.80:4242/getPool")
        .await
        .map_err(|_| MinerError::Connection)?
        .json()
        .await
        .map_err(|_| MinerError::Connection)?;

    Ok(format!("{}:{}", pool.ip, pool.port))
}
