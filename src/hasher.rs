use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use hex::FromHex;
use sha1::{Digest, Sha1};

type BlockHash = [u8; 20];

fn to_block_hash(s: &str) -> BlockHash {
    <BlockHash>::from_hex(s).unwrap_or_else(|_| panic!("{} is not hex string.", s))
}

#[derive(Clone)]
pub struct Sha1Hasher {
    hashmap: Arc<Mutex<HashMap<BlockHash, Vec<BlockHash>>>>,
    stack: Arc<Mutex<Vec<BlockHash>>>,
}

impl Sha1Hasher {
    const HASHMAP_LIMIT: usize = 10;

    pub fn new() -> Self {
        Self {
            hashmap: Arc::new(Mutex::new(HashMap::with_capacity(Self::HASHMAP_LIMIT * 2))),
            stack: Arc::new(Mutex::new(Vec::with_capacity(Self::HASHMAP_LIMIT * 2))),
        }
    }

    pub async fn get_hash(&self, last_block_hash: &str, expected_hash: &str, diff: u32) -> u32 {
        let last_block_hash = to_block_hash(last_block_hash);
        let expected_hash = to_block_hash(expected_hash);

        let mut hashmap = self.hashmap.lock().await;
        if let Some(hashes) = hashmap.get_mut(&last_block_hash) {
            // Optimized for lower difficulty, uses AVX.
            for (duco_numeric_result, hash) in hashes.iter().enumerate() {
                if hash == &expected_hash {
                    return duco_numeric_result as u32;
                }
            }

            let current_progress = hashes.len() as u32;
            if current_progress < diff {
                log::info!("Continuing calculation.");

                for duco_numeric_result in current_progress..diff {
                    let hash = self.hash_next_block(&last_block_hash, duco_numeric_result);
                    hashes.push(hash);

                    if hash == expected_hash {
                        return duco_numeric_result as u32;
                    }
                }
            }
        }

        let mut stack = self.stack.lock().await;

        if hashmap.len() > Self::HASHMAP_LIMIT * 2 {
            log::warn!("Too many hashes stored. Freeing.");

            for _ in 0..(hashmap.len() - Self::HASHMAP_LIMIT) {
                let k = stack.remove(0);
                hashmap.remove(&k);
            }
        }

        let mut hashes: Vec<BlockHash> = Vec::with_capacity(diff as usize);
        for duco_numeric_result in 0..diff {
            let hash = self.hash_next_block(&last_block_hash, duco_numeric_result);
            hashes.push(hash);

            if hash == expected_hash {
                hashmap.insert(last_block_hash, hashes);
                stack.push(last_block_hash);

                return duco_numeric_result;
            }
        }

        hashmap.insert(last_block_hash, hashes);
        stack.push(last_block_hash);

        0
    }

    fn hash_next_block(&self, last_block_hash: &BlockHash, duco_numeric_result: u32) -> BlockHash {
        let mut hasher = Sha1::new();

        let mut encode_slice: [u8; 40] = [0; 40];
        hex::encode_to_slice(&last_block_hash, &mut encode_slice).unwrap();

        sha1::Digest::update(&mut hasher, &encode_slice);
        sha1::Digest::update(&mut hasher, duco_numeric_result.to_string().as_bytes());
        let h = hasher.finalize();

        let mut hash: [u8; 20] = [0; 20];
        hash.copy_from_slice(&h);

        hash
    }
}
