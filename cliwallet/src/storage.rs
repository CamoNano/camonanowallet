use super::error::CliError;
use super::CliClient;
use aes_gcm::Error as AESError;
use client::{
    core::{rpc::WorkManager, CoreClient, CoreClientConfig, SecretBytes},
    storage::{EncryptedWallet, WalletData},
    Client, ClientConfig, ClientError,
};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

const APP_DATA_FOLDER_NAME: &str = "CamoNano-rs";

fn is_valid_name(name: &str) -> bool {
    name.chars().all(|c| c.is_alphanumeric()) && name != "config"
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
        client: &CliClient,
        name: &str,
        key: &SecretBytes<32>,
    ) -> Result<(), CliError> {
        let cli_client = &client.client;
        let client = &cli_client.internal;

        if !is_valid_name(name) {
            return Err(CliError::InvalidWalletName);
        }
        if self.wallet_exists(name) {
            self.delete_wallet(name, key)?
        }

        let data = WalletData {
            seed: client.seed.clone(),
            wallet_db: client.wallet_db.clone(),
            frontiers_db: client.frontiers_db.clone(),
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
            return Err(CliError::WalletAlreadyExists);
        }
        self.save_wallet_override(cli_client, name, key)
    }

    fn load_wallet(
        &self,
        config: CoreClientConfig,
        name: &str,
        key: SecretBytes<32>,
    ) -> Result<CliClient, CliError> {
        if !self.wallet_exists(name) {
            return Err(CliError::WalletNotFound);
        }
        let data = self
            .wallets
            .iter()
            .find(|wallet| wallet.name == name)
            .ok_or(CliError::WalletNotFound)?
            .decrypt(&key)?;
        let client = CoreClient {
            seed: data.seed,
            config: config.clone(),
            wallet_db: data.wallet_db,
            frontiers_db: data.frontiers_db,
        };

        let client = Client {
            name: name.into(),
            key,
            internal: client,
            cached_receivable: data.cached_receivable,
            camo_history: data.camo_history,
            work_client: WorkManager::default(),
        };
        Ok(CliClient { client })
    }

    fn delete_wallet(&mut self, name: &str, key: &SecretBytes<32>) -> Result<(), CliError> {
        let index = self
            .wallets
            .iter()
            .position(|wallet| wallet.name == name)
            .ok_or(CliError::WalletNotFound)?;
        if self.wallets[index].decrypt(key).is_ok() {
            self.wallets.remove(index);
            Ok(())
        } else {
            Err(CliError::ClientError(ClientError::InvalidPassword(
                AESError,
            )))
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
pub fn save_config(config: ClientConfig) -> Result<(), CliError> {
    Ok(confy::store(APP_DATA_FOLDER_NAME, "config", config)?)
}

/// Load the config file from disk
pub fn load_config() -> Result<CoreClientConfig, CliError> {
    let config: ClientConfig = confy::load(APP_DATA_FOLDER_NAME, "config")?;
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
