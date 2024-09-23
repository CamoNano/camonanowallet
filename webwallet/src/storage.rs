use super::error::AppError;
use super::logging::LevelFilter;
use super::web_api::{get_item, get_storage, remove_item, set_item};
use super::AppClient;
use client::{
    core::{CoreClientConfig, SecretBytes},
    storage::{EncryptedWallet, WalletData},
    ClientConfig,
};

const STORAGE_CONFIG_KEY: &str = "camonano_wallet_config";
const STORAGE_WALLET_ID_KEY: &str = "camonano_wallet_id";
const STORAGE_WALLET_SALT_KEY: &str = "camonano_wallet_salt";
const STORAGE_WALLET_NONCE_KEY: &str = "camonano_wallet_nonce";
const STORAGE_WALLET_DATA_KEY: &str = "camonano_wallet_data";
const STORAGE_LOG_LEVEL_KEY: &str = "camonano_wallet_log_level";

fn save_encrypted_wallet(wallet: EncryptedWallet) -> Result<(), AppError> {
    let storage = get_storage()?;
    set_item(&storage, STORAGE_WALLET_ID_KEY, &wallet.id)?;
    set_item(&storage, STORAGE_WALLET_SALT_KEY, &wallet.salt)?;
    set_item(&storage, STORAGE_WALLET_NONCE_KEY, &wallet.nonce)?;
    set_item(&storage, STORAGE_WALLET_DATA_KEY, &wallet.data)?;
    Ok(())
}

fn load_encrypted_wallet() -> Result<Option<EncryptedWallet>, AppError> {
    let storage = get_storage()?;
    let id = get_item(&storage, STORAGE_WALLET_ID_KEY)?;
    let salt = get_item(&storage, STORAGE_WALLET_SALT_KEY)?;
    let nonce = get_item(&storage, STORAGE_WALLET_NONCE_KEY)?;
    let data = get_item(&storage, STORAGE_WALLET_DATA_KEY)?;

    match (id, salt, nonce, data) {
        (Some(id), Some(salt), Some(nonce), Some(data)) => {
            let wallet = EncryptedWallet {
                id,
                salt,
                nonce,
                data,
            };
            Ok(Some(wallet))
        }
        (None, None, None, None) => Ok(None),
        _ => Err(AppError::StorageMismatch),
    }
}

/// Save the config to storage
pub fn save_config(config: ClientConfig) -> Result<(), AppError> {
    let storage = get_storage()?;
    let encoded = hex::encode(bincode::serialize(&config)?);
    set_item(&storage, STORAGE_CONFIG_KEY, &encoded)
}

/// Load the config from storage
pub fn load_config() -> Result<CoreClientConfig, AppError> {
    let storage = get_storage()?;
    let encoded = get_item(&storage, STORAGE_CONFIG_KEY)?;
    let config: CoreClientConfig = match encoded {
        Some(encoded) => bincode::deserialize(&hex::decode(encoded.as_bytes())?)?,
        None => ClientConfig::default().into(),
    };
    Ok(config)
}

/// Delete the config from storage
pub fn delete_config() -> Result<(), AppError> {
    let storage = get_storage()?;
    remove_item(&storage, STORAGE_CONFIG_KEY)?;
    Ok(())
}

/// Save the wallet to storage
pub fn save_wallet(cli_client: &AppClient, key: &SecretBytes<32>) -> Result<(), AppError> {
    let data: WalletData = cli_client.client.as_wallet_data();
    let encrypted = data.encrypt("default", key)?;
    save_encrypted_wallet(encrypted)
}

/// Load the wallet from storage
pub fn load_wallet(key: SecretBytes<32>) -> Result<AppClient, AppError> {
    let config = load_config()?;
    let data = load_encrypted_wallet()?
        .ok_or(AppError::WalletNotInitialized)?
        .decrypt(&key)?;
    let client = data.to_client(config);
    Ok(AppClient { key, client })
}

/// Delete the wallet from storage
pub fn delete_wallet() -> Result<(), AppError> {
    let storage = get_storage()?;
    remove_item(&storage, STORAGE_WALLET_ID_KEY)?;
    remove_item(&storage, STORAGE_WALLET_SALT_KEY)?;
    remove_item(&storage, STORAGE_WALLET_NONCE_KEY)?;
    remove_item(&storage, STORAGE_WALLET_DATA_KEY)?;
    Ok(())
}

/// Load the log level from storage
pub fn get_log_level() -> Result<LevelFilter, AppError> {
    let storage = get_storage()?;
    let level = get_item(&storage, STORAGE_LOG_LEVEL_KEY)?.unwrap_or("3".into());
    let level: usize = level.parse().unwrap_or(3);
    let level = match level {
        0 => LevelFilter::Off,
        1 => LevelFilter::Error,
        2 => LevelFilter::Warn,
        3 => LevelFilter::Info,
        4 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };
    Ok(level)
}
