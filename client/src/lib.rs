#![warn(unused_crate_dependencies, unsafe_code)]

mod balance;
mod defaults;
mod error;
mod interface;

pub mod storage;
pub mod types;

use core_client::{
    rpc::WorkManager, Account, CamoAccount, CoreClient, CoreClientConfig, Receivable, RescanData,
    SecretBytes, WalletSeed,
};
use defaults::{default_representatives, default_rpcs};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use types::CamoTxSummary;
use zeroize::{Zeroize, ZeroizeOnDrop};

pub use core_client as core;
pub use error::ClientError;
pub use interface::Command;

#[allow(non_snake_case)]
#[derive(Debug, Clone, Zeroize, Serialize, Deserialize)]
pub struct ClientConfig {
    config: CoreClientConfig,
}
impl Default for ClientConfig {
    fn default() -> Self {
        CoreClientConfig::default_with(default_representatives(), default_rpcs()).into()
    }
}
impl From<CoreClientConfig> for ClientConfig {
    fn from(value: CoreClientConfig) -> Self {
        ClientConfig { config: value }
    }
}
impl From<ClientConfig> for CoreClientConfig {
    fn from(value: ClientConfig) -> Self {
        value.config
    }
}

pub trait CliFrontend {
    /// Print a string
    fn print(s: &str);
    /// Clear the terminal
    fn clear_screen();
    /// Authenticate the user: if the password is incorrect, returns an error.
    /// Useful for e.g. displaying the wallet's seed.
    fn authenticate(&self) -> Result<(), ClientError>;
    /// Get this frontend's CliClient
    fn client(&self) -> &Client;
    /// Get this frontend's CliClient as mutable
    fn client_mut(&mut self) -> &mut Client;
}

#[derive(Debug, Zeroize, ZeroizeOnDrop)]
pub struct Client {
    pub name: String,
    pub key: SecretBytes<32>,
    pub internal: CoreClient,
    #[zeroize(skip)]
    pub cached_receivable: HashMap<[u8; 32], Receivable>,
    pub camo_history: Vec<CamoTxSummary>,
    #[zeroize(skip)]
    pub work_client: WorkManager,
}
impl Client {
    pub fn new(
        seed: WalletSeed,
        name: String,
        key: SecretBytes<32>,
        config: CoreClientConfig,
    ) -> Result<Client, ClientError> {
        let client = Client {
            name,
            key,
            internal: CoreClient::new(seed, config),
            cached_receivable: HashMap::new(),
            camo_history: vec![],
            work_client: WorkManager::default(),
        };
        Ok(client)
    }

    /// Remove this account's receivable transactions from the DB
    fn remove_receivable(&mut self, account: &Account) {
        self.cached_receivable
            .retain(|_, receivable| &receivable.recipient != account);
    }

    fn insert_receivable(&mut self, receivables: Vec<Receivable>) {
        for receivable in receivables {
            self.cached_receivable
                .insert(receivable.block_hash, receivable);
        }
    }

    /// Remove an account from all DB's.
    /// This method works for both normal and derived Nano accounts.
    fn remove_account(&mut self, account: &Account) -> Result<(), ClientError> {
        self.remove_receivable(account);
        self.internal.remove_account(account)?;
        Ok(())
    }

    /// Remove a camo account, and its derived accounts, from all DB's.
    fn remove_camo_account(&mut self, camo_account: &CamoAccount) -> Result<(), ClientError> {
        let derived = self.internal.get_derived_accounts_from_master(camo_account);
        for account in derived {
            self.remove_receivable(&account)
        }

        self.remove_receivable(&camo_account.signer_account());
        self.internal.remove_camo_account(camo_account)?;
        Ok(())
    }

    fn handle_rescan(&mut self, rescan: RescanData) {
        self.internal.set_new_frontiers(rescan.new_frontiers);
        self.internal
            .wallet_db
            .derived_account_db
            .insert_many(rescan.derived_info);
        self.insert_receivable(rescan.receivable);
    }

    pub async fn update_work_cache(&mut self) -> Result<(), ClientError> {
        Ok(self
            .internal
            .handle_work_results(&mut self.work_client)
            .await?)
    }
}
