use duino_miner::error::MinerError;

use hex::FromHex;
use sha1::{Digest, Sha1};

type BlockHash = [u8; 20];

fn to_block_hash(s: &str) -> Result<BlockHash, MinerError> {
    <BlockHash>::from_hex(s).map_err(|_| MinerError::MalformedJob(format!("Non hex string: {}", s)))
}

fn precompute_sha1(last_block_hash: &BlockHash) -> Sha1 {
    let mut hasher = Sha1::new();

    let mut encode_slice: [u8; 40] = [0; 40];
    hex::encode_to_slice(&last_block_hash, &mut encode_slice).unwrap();

    sha1::Digest::update(&mut hasher, &encode_slice);

    hasher
}

fn next_compute_numeric(mut hasher: Sha1, duco_numeric_result: u32) -> BlockHash {
    sha1::Digest::update(&mut hasher, duco_numeric_result.to_string().as_bytes());
    let h = hasher.finalize();

    let mut hash: [u8; 20] = [0; 20];
    hash.copy_from_slice(&h);

    hash
}

#[derive(Clone)]
pub struct Sha1Hasher {}

impl Sha1Hasher {
    pub fn new() -> Self {
        Self {}
    }

    pub fn get_hash(
        &self,
        last_block_hash: &str,
        expected_hash: &str,
        diff: u32,
    ) -> Result<u32, MinerError> {
        let last_block_hash = to_block_hash(last_block_hash)?;
        let expected_hash = to_block_hash(expected_hash)?;

        let hasher = precompute_sha1(&last_block_hash);
        for duco_numeric_result in 0..diff {
            let hash = next_compute_numeric(hasher.clone(), duco_numeric_result);

            if hash == expected_hash {
                return Ok(duco_numeric_result);
            }
        }

        Err(MinerError::MalformedJob(
            "Job impossible to solve.".to_string(),
        ))
    }
}
