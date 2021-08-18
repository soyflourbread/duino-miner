use sha1::{Digest, Sha1};

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};

fn sha1_digest(input: &str) -> String {
    let mut hasher = Sha1::new();
    sha1::Digest::update(&mut hasher, input.as_bytes());

    let h = hasher.finalize();
    format!("{:x}", h)
}

#[derive(Clone)]
pub struct Sha1Hasher {
    hashmap: Arc<RwLock<HashMap<String, Vec<String>>>>,
    stack: Arc<Mutex<Vec<String>>>,
}

impl Sha1Hasher {
    const HASHMAP_LIMIT: usize = 100;

    pub fn new() -> Self {
        Self {
            hashmap: Arc::new(RwLock::new(HashMap::new())),
            stack: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn get_hash(&self, last_block_hash: String, expected_hash: String, diff: u32) -> u32 {
        {
            let hashmap = self.hashmap.read().await;
            if let Some(hashes) = hashmap.get(&last_block_hash) {
                if hashes.len() < diff as usize {
                    log::warn!("Diff changed, recalculating.");
                } else {
                    // Optimized for lower difficulty, uses AVX.
                    for (duco_numeric_result, hash) in hashes.iter().enumerate() {
                        if hash == &expected_hash {
                            return duco_numeric_result as u32;
                        }
                    }
                }
            }
        } // Unlock hashmap

        let hashes = self.precompute(last_block_hash, diff).await;

        for (duco_numeric_result, hash) in hashes.iter().enumerate() {
            if hash == &expected_hash {
                return duco_numeric_result as u32;
            }
        }

        return 0;
    }

    async fn precompute(&self, last_block_hash: String, diff: u32) -> Vec<String> {
        let mut hashmap = self.hashmap.write().await;
        let mut stack = self.stack.lock().await;

        if hashmap.len() > Self::HASHMAP_LIMIT {
            for _ in 0..(hashmap.len() - Self::HASHMAP_LIMIT) {
                let k = stack.remove(0);
                hashmap.remove(&k);
            }
        }

        let hashes: Vec<String> = (0..diff)
            .map(|duco_numeric_result| format!("{}{}", last_block_hash, duco_numeric_result))
            .map(|h| sha1_digest(h.as_str()))
            .collect();

        hashmap.insert(last_block_hash.clone(), hashes.clone());
        stack.push(last_block_hash);

        hashes
    }
}
