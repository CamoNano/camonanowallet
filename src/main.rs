#![warn(unused_crate_dependencies, unsafe_code)]

mod balance;
mod defaults;
mod error;
mod init;
mod interface;
mod logging;
mod storage;
mod types;

use clap::Parser;
use client::{Account, CamoAccount, Client, Receivable, RescanData, SecretBytes, WalletSeed};
use error::CliError;
use init::{prompt_password, Init};
use interface::Command;
use std::collections::HashMap;
use std::io::{stdin, stdout, Write};
use storage::{load_config, save_config, save_wallet_overriding, StorageError};
use tokio::runtime::Runtime;
use types::CamoTxSummary;
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Debug, Zeroize, ZeroizeOnDrop)]
pub struct CliClient {
    pub name: String,
    pub key: SecretBytes<32>,
    pub internal: Client,
    #[zeroize(skip)]
    pub cached_receivable: HashMap<[u8; 32], Receivable>,
    pub camo_history: Vec<CamoTxSummary>,
}
impl CliClient {
    pub fn new(
        seed: WalletSeed,
        name: String,
        key: SecretBytes<32>,
    ) -> Result<CliClient, CliError> {
        Ok(CliClient {
            name,
            key,
            internal: Client::new(seed, load_config()?),
            cached_receivable: HashMap::new(),
            camo_history: vec![],
        })
    }

    pub fn save_to_disk(&mut self) -> Result<(), CliError> {
        save_config(self.internal.config.clone().into())?;
        save_wallet_overriding(self, &self.name, &self.key)
    }

    /// Authenticate the user: if the password is incorrect, returns an error.
    /// Useful for e.g. displaying the wallet's seed.
    pub fn authenticate(&self) -> Result<(), CliError> {
        if prompt_password()? == self.key {
            Ok(())
        } else {
            Err(CliError::StorageError(StorageError::InvalidPassword(
                aes_gcm::Error,
            )))
        }
    }

    /// Remove this account's receivable transactions from the DB
    pub fn remove_receivable(&mut self, account: &Account) {
        self.cached_receivable
            .retain(|_, receivable| &receivable.recipient != account);
    }

    pub fn insert_receivable(&mut self, receivables: Vec<Receivable>) {
        for receivable in receivables {
            self.cached_receivable
                .insert(receivable.block_hash, receivable);
        }
    }

    /// Remove an account from all DB's.
    /// This method works for both normal and derived Nano accounts.
    pub fn remove_account(&mut self, account: &Account) -> Result<(), CliError> {
        self.remove_receivable(account);
        self.internal.remove_account(account)?;
        Ok(())
    }

    /// Remove a camo account, and its derived accounts, from all DB's.
    pub fn remove_camo_account(&mut self, camo_account: &CamoAccount) -> Result<(), CliError> {
        let derived = self.internal.get_derived_accounts_from_master(camo_account);
        for account in derived {
            self.remove_receivable(&account)
        }

        self.remove_receivable(&camo_account.signer_account());
        self.internal.remove_camo_account(camo_account)?;
        Ok(())
    }

    pub fn handle_rescan(&mut self, rescan: RescanData) {
        self.internal.set_new_frontiers(rescan.new_frontiers);
        self.internal
            .wallet_db
            .derived_account_db
            .insert_many(rescan.derived_info);
        self.insert_receivable(rescan.receivable);
    }

    pub fn start_cli(&mut self) {
        let rt = Runtime::new().expect("could not create Tokio runtime");

        loop {
            print!("> ");
            stdout().flush().expect("failed to flush stdout");

            let mut input = String::new();
            stdin().read_line(&mut input).expect("failed to read stdin");

            let result = Command::execute(self, &rt, &input);
            self.save_to_disk().expect("failed to save wallet to disk");

            match result {
                Ok(true) => continue,
                Ok(false) => break,
                Err(err) => println!("{:?}", err),
            }
        }
    }
}

fn main() {
    let init = Init::parse().execute();
    let (client, logger) = match init {
        Ok((client, logger)) => (client, logger),
        Err(err) => {
            println!("{:?}", err);
            return;
        }
    };

    let mut client = match client {
        Some(client) => client,
        None => return,
    };

    match logger.start_logging() {
        Ok(()) => (),
        Err(err) => println!("Failed to start logging: {err}"),
    }

    client.start_cli();
}
