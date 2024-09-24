use super::types::CamoTxSummary;
use crate::{Client, ClientError, CoreClient};
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use argon2::Argon2;
use core_client::{
    frontiers::FrontiersDB,
    rpc::WorkManager,
    wallet::{WalletDB, WalletSeed},
    CoreClientConfig, Receivable, SecretBytes,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Slow hash for password hashing
fn key_hash(key: &[u8], salt: &[u8]) -> Result<Key<Aes256Gcm>, ClientError> {
    let mut output = [0_u8; 32];
    Argon2::default().hash_password_into(key, salt, &mut output)?;
    Ok(output.into())
}

#[derive(Debug, Zeroize, Serialize, Deserialize)]
pub struct WalletData {
    pub seed: WalletSeed,
    pub wallet_db: WalletDB,
    pub frontiers_db: FrontiersDB,
    #[zeroize(skip)]
    pub cached_receivable: HashMap<[u8; 32], Receivable>,
    pub camo_history: Vec<CamoTxSummary>,
}
impl WalletData {
    pub fn encrypt(
        mut self,
        id: &str,
        key: &SecretBytes<32>,
    ) -> Result<EncryptedWallet, ClientError> {
        let salt = rand::random::<[u8; 32]>();
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let key = key_hash(key.as_bytes(), &salt)?;

        let cipher = Aes256Gcm::new(&key);
        let mut data = bincode::serialize(&self)?;
        let encrypted = cipher
            .encrypt(&nonce, data.as_ref())
            .map_err(ClientError::EncryptionError)?;

        self.zeroize();
        data.zeroize();
        Ok(EncryptedWallet {
            name: id.into(),
            salt: hex::encode(salt),
            nonce: hex::encode(nonce),
            data: hex::encode(encrypted),
        })
    }

    pub fn to_client(self, config: CoreClientConfig) -> Client {
        let client = CoreClient {
            seed: self.seed,
            config,
            wallet_db: self.wallet_db,
            frontiers_db: self.frontiers_db,
        };

        Client {
            core: client,
            receivable: self.cached_receivable,
            camo_history: self.camo_history,
            work: WorkManager::default(),
        }
    }
}

#[derive(Debug, Clone, Zeroize, ZeroizeOnDrop, Serialize, Deserialize)]
pub struct EncryptedWallet {
    pub name: String,
    pub salt: String,
    pub nonce: String,
    pub data: String,
}
impl EncryptedWallet {
    pub fn decrypt(&self, key: &SecretBytes<32>) -> Result<WalletData, ClientError> {
        let salt = hex::decode(&self.salt)?;
        let nonce = hex::decode(&self.nonce)?;
        let nonce = Nonce::from_slice(&nonce);
        let key = key_hash(key.as_bytes(), &salt)?;

        let cipher = Aes256Gcm::new(&key);
        let mut data = hex::decode(&self.data)?;
        let mut plaintext = cipher
            .decrypt(nonce, data.as_ref())
            .map_err(ClientError::InvalidPassword)?;

        let wallet: WalletData = bincode::deserialize(&plaintext)?;
        plaintext.zeroize();
        data.zeroize();
        Ok(wallet)
    }
}
