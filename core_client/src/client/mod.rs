mod camo;
mod receive;
mod send;

use crate::rpc::workserver::WorkClient;
use super::config::CoreClientConfig;
use super::error::CoreClientError;
use super::frontiers::{FrontierInfo, FrontiersDB, NewFrontiers};
use super::rpc::{ClientRpc, RpcFailures, RpcResult, RpcSuccess};
use super::wallet::{DerivedAccountInfo, WalletDB, WalletSeed};
use camo::{get_camo_receivable, rescan_notifications_partial};
use log::error;
use nanopyrs::{
    camo::{CamoAccount, Notification},
    rpc::Receivable,
    Account,
};
use rand::seq::SliceRandom;
use receive::{get_accounts_receivable, receive_single};
use send::{send, send_camo, sender_ecdh};
use zeroize::Zeroize;

pub use camo::RescanData;
pub use send::{CamoPayment, Payment};

pub(crate) fn choose_representatives(
    config: &CoreClientConfig,
    current: Account,
    option: Option<Account>,
) -> Account {
    if let Some(rep) = option {
        return rep;
    }
    if config.REPRESENTATIVES.contains(&current) {
        return current;
    }
    config
        .REPRESENTATIVES
        .choose(&mut rand::thread_rng())
        .expect("no representatives to choose from")
        .clone()
}

#[derive(Debug, Zeroize)]
pub struct CoreClient {
    pub seed: WalletSeed,
    pub config: CoreClientConfig,

    pub wallet_db: WalletDB,
    pub frontiers_db: FrontiersDB,
}
impl CoreClient {
    pub fn new(seed: WalletSeed, config: CoreClientConfig) -> CoreClient {
        CoreClient {
            seed,
            config,
            wallet_db: WalletDB::default(),
            frontiers_db: FrontiersDB::default(),
        }
    }

    /// Returns the frontiers of all `nano_` accounts in the wallet with `balance >= amount`,
    /// excluding the given accounts, sorted by `balance` low to high
    pub fn accounts_with_balance(&self, amount: u128, exclude: &[Account]) -> Vec<&FrontierInfo> {
        let mut frontiers = self
            .wallet_db
            .all_nano_accounts()
            .iter()
            .filter(|account| !exclude.contains(account))
            .filter_map(|account| self.frontiers_db.account_frontier(account))
            .filter(|block| block.block.balance >= amount)
            .collect::<Vec<&FrontierInfo>>();
        frontiers.sort_by_key(|info| info.block.balance);
        frontiers
    }

    /// Get the wallet's balance, according to this database
    pub fn wallet_balance(&self) -> u128 {
        self.frontiers_db
            .accounts_balances(&self.wallet_db.all_nano_accounts())
            .iter()
            .map(|balance| balance.unwrap_or(0))
            .sum()
    }

    /// Find the derived accounts in the DB, given the master camo account
    pub fn get_derived_accounts_from_master(&self, master: &CamoAccount) -> Vec<Account> {
        self.wallet_db
            .derived_account_db
            .get_info_from_master(&self.wallet_db.camo_account_db, master)
            .into_iter()
            .map(|frontier| &frontier.account)
            .cloned()
            .collect()
    }

    /// Download the frontiers of the given accounts.
    pub async fn download_frontiers(&self, accounts: &[Account]) -> RpcResult<NewFrontiers> {
        ClientRpc()
            .download_frontiers(&self.config, &self.frontiers_db, accounts)
            .await
    }

    /// Download the frontiers of any unknown accounts.
    pub async fn download_unknown_frontiers(&self) -> RpcResult<NewFrontiers> {
        let unknown = self
            .frontiers_db
            .filter_known_accounts(self.wallet_db.all_nano_accounts());
        self.download_frontiers(&unknown).await
    }

    /// Get all receivable payments for these accounts, including camo payments.
    /// Returns receivable payments, as well as `DerivedAccountInfo`'s for the wallet DB.
    ///
    /// Note that the number of receivable payments per account that can be returned at one time is limited by `ACCOUNTS_RECEIVABLE_BATCH_SIZE`.
    pub async fn download_receivable(
        &self,
        accounts: &[Account],
    ) -> RpcResult<(Vec<Receivable>, Vec<DerivedAccountInfo>)> {
        // get receivable for normal payments
        let (receivable, mut rpc_failures) = get_accounts_receivable(self, accounts).await?.into();
        // get receivable for camo payments
        let ((camo_receivable, derived_account_info), rpc_failures_2) =
            get_camo_receivable(self, &receivable).await?.into();

        // camo payments should be received first in order to prevent losses in the event of a crash
        let receivable = [camo_receivable, receivable].concat();
        rpc_failures.merge_with(rpc_failures_2);
        Ok(((receivable, derived_account_info), rpc_failures).into())
    }

    /// Scan part of the notification account's history for camo payments.
    ///
    /// Mostly aligns with the `account_history` API,
    /// but with `count` set to `config::RPC_ACCOUNT_HISTORY_BATCH_SIZE`,
    /// and `offset` multiplied by `config::RPC_ACCOUNT_HISTORY_BATCH_SIZE`.
    ///
    /// `filter` determines whether or not to filter accounts with no value (0 balance or pending transactions).
    ///
    /// Note that the destination accounts are *not* scanned, only calculated.
    pub async fn rescan_notifications_partial(
        &self,
        account: &CamoAccount,
        head: Option<[u8; 32]>,
        offset: Option<usize>,
        filter: bool,
    ) -> RpcResult<RescanData> {
        rescan_notifications_partial(self, account, head, offset, filter).await
    }

    /// Receive a single transaction, returning the new frontier of that account (the `receive` block), **with** cached work.
    pub async fn receive_single(&self, work_client: &mut WorkClient, receivable: &Receivable) -> RpcResult<NewFrontiers> {
        receive_single(self, work_client, receivable).await
    }

    /// Send to a `nano_` account.
    pub async fn send(&self, work_client: &mut WorkClient, payment: Payment) -> RpcResult<NewFrontiers> {
        send(self, work_client, payment).await
    }

    /// Send to a `camo_` account.
    /// The notifier and sender accounts most be different for privacy reasons.
    pub async fn send_camo(&self, work_client: &mut WorkClient, payment: CamoPayment) -> RpcResult<NewFrontiers> {
        send_camo(self, work_client, payment).await
    }

    /// Returns `(derived_account, notification)`
    pub fn camo_transaction_memo(
        &self,
        payment: &CamoPayment,
    ) -> Result<(Account, Notification), CoreClientError> {
        let sender_key = self
            .wallet_db
            .find_key(&self.seed, &payment.sender)
            .ok_or(CoreClientError::AccountNotFound)?;
        let (shared_secret, notification) = sender_ecdh(self, &payment.recipient, &sender_key)?;
        let derived = payment.recipient.derive_account(&shared_secret);
        Ok((derived, notification))
    }

    /// Add or update several accounts' frontiers, also handling unopened accounts.
    pub fn set_new_frontiers(&mut self, new: NewFrontiers) {
        if let Err(err) = self.frontiers_db.insert(new) {
            // frontiers should have already been sanity-checked
            error!("Attempted to set invalid frontier(s): {err}")
        }
    }

    /// Remove an account from the wallet and frontier DB's, and returns its frontier.
    /// This method works for both normal and derived Nano accounts.
    pub fn remove_account(&mut self, account: &Account) -> Result<FrontierInfo, CoreClientError> {
        let account_db = self.wallet_db.account_db.remove(account).map(|_| ());
        let derived_db = self
            .wallet_db
            .derived_account_db
            .remove(account)
            .map(|_| ());
        let frontier_db = self.frontiers_db.remove(account);

        account_db.or(derived_db)?;
        frontier_db
    }

    /// Remove a camo account and its derived accounts from the wallet and frontier DB's, and returns its frontier.
    pub fn remove_camo_account(
        &mut self,
        account: &CamoAccount,
    ) -> Result<FrontierInfo, CoreClientError> {
        let derived = self.get_derived_accounts_from_master(account);
        for account in derived {
            match self.wallet_db.derived_account_db.remove(&account) {
                Ok(_) => (),
                Err(err) => {
                    error!("Unknown account {account} marked for removal from wallet DB: {err}")
                }
            }
            match self.frontiers_db.remove(&account) {
                Ok(_) => (),
                Err(err) => {
                    error!("Unknown account {account} marked for removal from frontiers DB: {err}")
                }
            }
        }

        self.wallet_db.camo_account_db.remove(account)?;
        self.frontiers_db.remove(&account.signer_account())
    }

    /// Handle the given RPC failures, adjusting future RPC selections as necessary
    pub fn handle_rpc_failures(&mut self, failures: RpcFailures) {
        ClientRpc().handle_failures(&mut self.config, failures)
    }

    /// Handle the given RPC failures, adjusting future RPC selections as necessary
    pub fn handle_rpc_success<T>(&mut self, success: RpcSuccess<T>) -> T {
        self.handle_rpc_failures(success.failures);
        success.item
    }
}
