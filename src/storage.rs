use super::defaults::{default_representatives, default_rpcs};
use super::error::CliError;
use super::types::CamoTxSummary;
use super::CliClient;
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Error as AESError, Key, Nonce,
};
use argon2::{Argon2, Error as Argon2Error};
use bincode::Error as BincodeError;
use client::{
    frontiers::FrontiersDB,
    wallet::{WalletDB, WalletSeed},
    Client, ClientConfig, Receivable, SecretBytes,
};
use confy::ConfyError;
use hex::FromHexError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

const APP_DATA_FOLDER_NAME: &str = "CamoNano-rs";

#[allow(non_snake_case)]
#[derive(Debug, Clone, Zeroize, Serialize, Deserialize)]
pub struct ClientConfigConfy {
    config: ClientConfig,
}
impl Default for ClientConfigConfy {
    fn default() -> Self {
        ClientConfig::default_with(default_representatives(), default_rpcs()).into()
    }
}
impl From<ClientConfig> for ClientConfigConfy {
    fn from(value: ClientConfig) -> Self {
        ClientConfigConfy { config: value }
    }
}
impl From<ClientConfigConfy> for ClientConfig {
    fn from(value: ClientConfigConfy) -> Self {
        value.config
    }
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("invalid configuration file: {0}")]
    InvalidConfig(String),
    #[error("Invalid hex value: {0}")]
    InvalidHex(#[from] FromHexError),
    #[error("Error while serializing/deserializing data: {0}")]
    SerializationError(#[from] BincodeError),
    #[error("error while deriving encryption key from password: {0}")]
    Argon2Error(Argon2Error),
    #[error("Error while encrypting/decrypting data: {0}")]
    EncryptionError(AESError),
    #[error("Invalid password for wallet: {0}")]
    InvalidPassword(AESError),
    #[error(transparent)]
    DiskError(#[from] ConfyError),
    #[error("The given wallet name is invalid")]
    InvalidWalletName,
    #[error("No wallet of the given name could be found")]
    WalletNotFound,
    #[error("A wallet of the same name already exists")]
    WalletAlreadyExists,
}
impl From<Argon2Error> for StorageError {
    fn from(value: Argon2Error) -> Self {
        StorageError::Argon2Error(value)
    }
}

fn is_valid_name(name: &str) -> bool {
    name.chars().all(|c| c.is_alphanumeric()) && name != "config"
}

/// Slow hash for password hashing
fn key_hash(key: &[u8], salt: &[u8]) -> Result<Key<Aes256Gcm>, StorageError> {
    let mut output = [0_u8; 32];
    Argon2::default().hash_password_into(key, salt, &mut output)?;
    Ok(output.into())
}

#[derive(Debug, Zeroize, Serialize, Deserialize)]
struct WalletData {
    seed: WalletSeed,
    wallet_db: WalletDB,
    frontiers_db: FrontiersDB,
    #[zeroize(skip)]
    cached_receivable: HashMap<[u8; 32], Receivable>,
    camo_history: Vec<CamoTxSummary>,
}
impl WalletData {
    fn encrypt(
        mut self,
        name: &str,
        key: &SecretBytes<32>,
    ) -> Result<EncryptedWallet, StorageError> {
        let salt = rand::random::<[u8; 32]>();
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let key = key_hash(key.as_bytes(), &salt)?;

        let cipher = Aes256Gcm::new(&key);
        let mut data = bincode::serialize(&self)?;
        let encrypted = cipher
            .encrypt(&nonce, data.as_ref())
            .map_err(StorageError::EncryptionError)?;

        self.zeroize();
        data.zeroize();
        Ok(EncryptedWallet {
            name: name.into(),
            salt: hex::encode(salt),
            nonce: hex::encode(nonce),
            data: hex::encode(encrypted),
        })
    }
}

#[derive(Debug, Clone, Zeroize, ZeroizeOnDrop, Serialize, Deserialize)]
struct EncryptedWallet {
    name: String,
    salt: String,
    nonce: String,
    data: String,
}
impl EncryptedWallet {
    fn decrypt(&self, key: &SecretBytes<32>) -> Result<WalletData, StorageError> {
        let salt = hex::decode(&self.salt)?;
        let nonce = hex::decode(&self.nonce)?;
        let nonce = Nonce::from_slice(&nonce);
        let key = key_hash(key.as_bytes(), &salt)?;

        let cipher = Aes256Gcm::new(&key);
        let mut data = hex::decode(&self.data)?;
        let mut plaintext = cipher
            .decrypt(nonce, data.as_ref())
            .map_err(StorageError::InvalidPassword)?;

        let wallet: WalletData = bincode::deserialize(&plaintext)?;
        plaintext.zeroize();
        data.zeroize();
        Ok(wallet)
    }
}

#[derive(Debug, Clone, Default, Zeroize, ZeroizeOnDrop, Serialize, Deserialize)]
struct UserWallets {
    wallets: Vec<EncryptedWallet>,
}
impl UserWallets {
    fn load_from_disk() -> Result<UserWallets, CliError> {
        Ok(confy::load(APP_DATA_FOLDER_NAME, "wallets")?)
    }

    fn save_to_disk(self) -> Result<(), CliError> {
        confy::store(APP_DATA_FOLDER_NAME, "wallets", self)?;
        Ok(())
    }

    fn wallet_exists(&self, name: &str) -> bool {
        self.wallets.iter().any(|wallet| wallet.name == name)
    }

    fn save_wallet_override(
        &mut self,
        cli_client: &CliClient,
        name: &str,
        key: &SecretBytes<32>,
    ) -> Result<(), CliError> {
        let client = cli_client.internal.clone();

        if !is_valid_name(name) {
            return Err(StorageError::InvalidWalletName.into());
        }
        if self.wallet_exists(name) {
            self.delete_wallet(name, key)?
        }

        let data = WalletData {
            seed: client.seed,
            wallet_db: client.wallet_db,
            frontiers_db: client.frontiers_db,
            cached_receivable: cli_client.cached_receivable.clone(),
            camo_history: cli_client.camo_history.clone(),
        };
        let encrypted = data.encrypt(name, key)?;
        self.wallets.push(encrypted);
        Ok(())
    }

    fn save_wallet(
        &mut self,
        cli_client: &CliClient,
        name: &str,
        key: &SecretBytes<32>,
    ) -> Result<(), CliError> {
        if self.wallet_exists(name) {
            return Err(StorageError::WalletAlreadyExists.into());
        }
        self.save_wallet_override(cli_client, name, key)
    }

    fn load_wallet(
        &self,
        config: ClientConfig,
        name: &str,
        key: SecretBytes<32>,
    ) -> Result<CliClient, CliError> {
        if !self.wallet_exists(name) {
            return Err(StorageError::WalletNotFound.into());
        }
        let data = self
            .wallets
            .iter()
            .find(|wallet| wallet.name == name)
            .ok_or(StorageError::WalletNotFound)?
            .decrypt(&key)?;
        let client = Client {
            seed: data.seed,
            config,
            wallet_db: data.wallet_db,
            frontiers_db: data.frontiers_db,
        };
        Ok(CliClient {
            name: name.into(),
            key,
            internal: client,
            cached_receivable: data.cached_receivable,
            camo_history: data.camo_history,
        })
    }

    fn delete_wallet(&mut self, name: &str, key: &SecretBytes<32>) -> Result<(), CliError> {
        let index = self
            .wallets
            .iter()
            .position(|wallet| wallet.name == name)
            .ok_or(CliError::StorageError(StorageError::WalletNotFound))?;
        if self.wallets[index].decrypt(key).is_ok() {
            self.wallets.remove(index);
            Ok(())
        } else {
            Err(StorageError::InvalidPassword(AESError).into())
        }
    }
}

/// Return the path of the config file
pub fn config_location() -> Result<String, CliError> {
    let path = confy::get_configuration_file_path(APP_DATA_FOLDER_NAME, "config")?;
    Ok(path
        .to_str()
        .expect("could not get configuration file location")
        .into())
}

/// Save the config file to disk
pub fn save_config(config: ClientConfigConfy) -> Result<(), CliError> {
    Ok(confy::store(APP_DATA_FOLDER_NAME, "config", config)?)
}

/// Load the config file from disk
pub fn load_config() -> Result<ClientConfig, CliError> {
    let config: ClientConfigConfy = confy::load(APP_DATA_FOLDER_NAME, "config")?;
    Ok(config.into())
}

/// Return the names of all wallet files on disk
pub fn get_wallet_names() -> Result<Vec<String>, CliError> {
    let wallets = UserWallets::load_from_disk()?;
    Ok(wallets
        .wallets
        .iter()
        .map(|wallet| wallet.name.clone())
        .collect())
}

/// Check if the wallet exists on disk
pub fn wallet_exists(name: &str) -> Result<bool, CliError> {
    Ok(UserWallets::load_from_disk()?.wallet_exists(name))
}

/// Save the wallet, overriding any existing file if necessary
pub fn save_wallet_overriding(
    cli_client: &CliClient,
    name: &str,
    key: &SecretBytes<32>,
) -> Result<(), CliError> {
    let mut wallets = UserWallets::load_from_disk()?;
    wallets.save_wallet_override(cli_client, name, key)?;
    wallets.save_to_disk()
}

/// Save the wallet, returning `Err` if the wallet already exists on disk
pub fn save_wallet(
    cli_client: &CliClient,
    name: &str,
    key: &SecretBytes<32>,
) -> Result<(), CliError> {
    let mut wallets = UserWallets::load_from_disk()?;
    wallets.save_wallet(cli_client, name, key)?;
    wallets.save_to_disk()
}

/// Load the wallet file from disk
pub fn load_wallet(name: &str, key: SecretBytes<32>) -> Result<CliClient, CliError> {
    let config = load_config()?;
    let wallets = UserWallets::load_from_disk()?;
    wallets.load_wallet(config, name, key)
}

/// Delete the wallet file from disk, returning `Err` if the wallet file is not found
pub fn delete_wallet(name: &str, key: &SecretBytes<32>) -> Result<(), CliError> {
    let mut wallets = UserWallets::load_from_disk()?;
    wallets.delete_wallet(name, key)?;
    wallets.save_to_disk()
}

/// Load the config and wallet files to ensure that they exist on disk
pub fn init_files() -> Result<(), CliError> {
    load_config()?;
    UserWallets::load_from_disk()?;
    Ok(())
}
