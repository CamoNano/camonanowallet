use super::balance;
use super::error::ClientError;
use super::types::{Amount, CamoTxSummary};
use super::types::{Hex32Bytes, ParsedAccount, ParsedCamoVersion};
use super::CliFrontend;
use clap::{Args, Parser, Subcommand};
use core_client::{
    constants::CAMO_SENDER_DUST_THRESHOLD, Account, CamoAccount, Receivable, CamoPayment, CamoVersion,
    CamoVersions, CoreClientError, Notification, Payment, rpc::RpcManager
};
use std::cmp::{max, min};

#[derive(Debug, Parser)]
#[command(no_binary_name = true, arg_required_else_help = true)]
#[command(version, name = "")]
pub struct Command {
    #[clap(subcommand)]
    command: CommandType,
}
impl Command {
    /// `Ok(true)` means continue looping, `Ok(false)` means exit
    pub async fn execute<Frontend: CliFrontend>(
        frontend: &mut Frontend,
        command: &str,
    ) -> Result<bool, ClientError> {
        let command = command.split_whitespace();
        let command = match Command::try_parse_from(command) {
            Ok(command) => command,
            Err(err) => {
                Frontend::print(&err.to_string());
                return Ok(true);
            }
        };

        match command.command {
            CommandType::Account(args) => args.execute(frontend).await,
            CommandType::Balance(args) => args.execute(frontend),
            CommandType::CamoHistory(args) => args.execute(frontend),
            CommandType::Clear(args) => args.execute::<Frontend>(),
            CommandType::ClearCache(args) => args.execute(frontend).await,
            CommandType::Notify(args) => args.execute(frontend).await,
            CommandType::Receive(args) => args.execute(frontend).await,
            CommandType::Refresh(args) => args.execute(frontend).await,
            CommandType::Remove(args) => args.execute(frontend).await,
            CommandType::Rescan(args) => args.execute(frontend).await,
            CommandType::Seed(args) => args.execute(frontend),
            CommandType::Send(args) => args.execute(frontend).await,
            CommandType::SendCamo(args) => args.execute(frontend).await,
            CommandType::Quit(args) => args.execute(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum CommandType {
    /// Get account at the specified index
    Account(AccountArgs),
    /// Display wallet balance
    Balance(BalanceArgs),
    /// Display send history of Camo transactions
    #[clap(name = "camo_history")]
    CamoHistory(CamoHistoryArgs),
    /// Clear the terminal
    Clear(ClearArgs),
    /// Clear the work cache
    #[clap(name = "clear_cache")]
    ClearCache(ClearCacheArgs),
    /// Send a notification to a Camo account for a Camo payment
    Notify(NotifyArgs),
    /// Receive transactions
    Receive(ReceiveArgs),
    /// Refresh the wallet
    Refresh(RefreshArgs),
    /// Stop tracking a Nano or Camo account
    Remove(RemoveArgs),
    /// Rescan a Camo account for Camo payments
    Rescan(RescanArgs),
    /// Show the seed of this wallet
    Seed(SeedArgs),
    /// Send coins to a normal Nano account
    Send(SendArgs),
    /// Send coins to a Camo account
    #[clap(name = "send_camo")]
    SendCamo(SendCamoArgs),
    /// Exit the program
    #[clap(alias = "exit")]
    Quit(QuitArgs),
}

#[derive(Debug, Args)]
struct AccountArgs {
    index: u32,
    #[arg(short, long, default_value_t = false)]
    camo: bool,
    /// Which Camo protocol versions to support.
    /// Only used when creating a camo_ account.
    /// A reasonable default will be used if no value is given.
    #[arg(short, long, hide = true)]
    versions: Option<Vec<ParsedCamoVersion>>,
}
impl AccountArgs {
    async fn execute<Frontend: CliFrontend>(
        self,
        frontend: &mut Frontend,
    ) -> Result<bool, ClientError> {
        let cli_client = frontend.client_mut();
        let client = &mut cli_client.internal;

        let string = if self.camo {
            let mut versions = client.config.DEFAULT_CAMO_VERSIONS.clone();
            if let Some(v) = self.versions {
                versions = v.iter().map(|v| v.0).collect::<Vec<CamoVersion>>()
            }

            let (key, info) = client
                .seed
                .get_camo_key(self.index, CamoVersions::new(&versions))
                .ok_or(ClientError::InvalidArguments)?;
            client
                .wallet_db
                .camo_account_db
                .insert(&client.config, info)?;
            key.to_camo_account().to_string()
        } else {
            if self.versions.is_some() {
                Frontend::print("The 'versions' option is only used for camo accounts");
                return Err(ClientError::InvalidArguments);
            }
            let (key, info) = client.seed.get_key(self.index);
            client.wallet_db.account_db.insert(&client.config, info)?;
            key.to_account().to_string()
        };

        let downloaded = client.download_unknown_frontiers().await?;
        let downloaded = client.handle_rpc_success(downloaded);
        client.set_new_frontiers(downloaded);

        Frontend::print(&string);
        Ok(true)
    }
}

#[derive(Debug, Args)]
struct BalanceArgs {}
impl BalanceArgs {
    fn execute<Frontend: CliFrontend>(self, frontend: &Frontend) -> Result<bool, ClientError> {
        balance::execute(frontend)?;
        Ok(true)
    }
}

#[derive(Debug, Args)]
struct CamoHistoryArgs {
    /// The maximum number of transactions to display
    #[arg(short, long, default_value_t = 20, conflicts_with = "clear")]
    count: usize,
    /// Clear the Camo history for this wallet
    #[arg(short = 'C', long, default_value_t = false, conflicts_with = "count")]
    clear: bool,
}
impl CamoHistoryArgs {
    fn execute<Frontend: CliFrontend>(self, frontend: &mut Frontend) -> Result<bool, ClientError> {
        let cli_client = frontend.client_mut();
        if self.clear {
            cli_client.camo_history.clear();
            return Ok(true);
        }

        for (i, payment) in cli_client.camo_history.iter().enumerate() {
            if i == self.count {
                break;
            }
            Frontend::print(&payment.to_string());
        }

        Ok(true)
    }
}

#[derive(Debug, Args)]
struct ClearArgs {}
impl ClearArgs {
    fn execute<Frontend: CliFrontend>(self) -> Result<bool, ClientError> {
        Frontend::clear_screen();
        Ok(true)
    }
}

#[derive(Debug, Args)]
struct ClearCacheArgs {
    /// Clear the work cache for all accounts
    #[arg(short, long, conflicts_with = "accounts")]
    all: bool,
    /// Clear the work cache on these accounts
    #[arg(short, long, conflicts_with = "all")]
    accounts: Vec<Account>,
}
impl ClearCacheArgs {
    async fn execute<Frontend: CliFrontend>(
        self,
        frontend: &mut Frontend,
    ) -> Result<bool, ClientError> {
        let cli_client = frontend.client_mut();
        let client = &mut cli_client.internal;

        let accounts = if !self.accounts.is_empty() {
            self.accounts
        } else if self.all {
            client.frontiers_db.all_accounts()
        } else {
            Frontend::print("Please specify which work caches to clear");
            return Err(ClientError::InvalidArguments);
        };

        for account in accounts {
            if let Some(frontier) = client.frontiers_db.account_frontier_mut(&account) {
                frontier.clear_work();
            }
        }

        Ok(true)
    }
}

#[derive(Debug, Args)]
struct NotifyArgs {
    /// Notifier nano_ account
    notifier: Account,
    /// Recipient camo_ account
    recipient: CamoAccount,
    /// The notification to send, encoded as a 64-character hex string (see 'camo_history')
    notification: Hex32Bytes,
    /// Amount of Nano that the notifier account should send
    #[arg(short, long, default_value_t = Amount::from(CAMO_SENDER_DUST_THRESHOLD))]
    amount: Amount,
}
impl NotifyArgs {
    async fn execute<Frontend: CliFrontend>(
        self,
        frontend: &mut Frontend,
    ) -> Result<bool, ClientError> {
        let cli_client = frontend.client_mut();
        let work_client = &mut cli_client.work_client;
        let client = &mut cli_client.internal;

        if self.amount.value < CAMO_SENDER_DUST_THRESHOLD {
            return Err(ClientError::AmountBelowDustThreshold);
        }

        let payment = Payment {
            sender: self.notifier,
            amount: self.amount.into(),
            recipient: self.recipient.signer_account(),
            new_representative: Some(Account::from_bytes(self.notification.0)?),
        };
        Frontend::print("Sending...");
        let success = client.send(work_client, payment).await?;

        let frontiers = client.handle_rpc_success(success);
        client.set_new_frontiers(frontiers);
        Frontend::print("Done");
        Ok(true)
    }
}

#[derive(Debug, Args)]
struct ReceiveArgs {
    /// List receivable transactions (default behavior)
    #[arg(short, long)]
    list: bool,
    /// The block hash to receive
    #[arg(short, long, conflicts_with = "accounts")]
    block: Option<Hex32Bytes>,
    // TODO: re-enable
    // /// The block hashes to receive
    // #[arg(short, long, conflicts_with = "accounts")]
    // blocks: Vec<Hex32Bytes>,
    // /// The accounts to receive transactions on
    // #[arg(short, long, conflicts_with = "blocks")]
    // accounts: Vec<Account>,
}
impl ReceiveArgs {
    async fn execute<Frontend: CliFrontend>(
        self,
        frontend: &mut Frontend,
    ) -> Result<bool, ClientError> {
        let cli_client = frontend.client_mut();
        let work_client = &mut cli_client.work_client;
        let client = &mut cli_client.internal;

        if let Some(block) = &self.block {
            let receivables = cli_client.cached_receivable.remove(&block.0).ok_or(CoreClientError::AccountNotFound)?;
            Frontend::print("Receiving...");
            let result = client.receive_single(work_client, &receivables).await?;
            let frontiers = client.handle_rpc_success(result);
            client.set_new_frontiers(frontiers);

            Frontend::print("Done");
        } else {
            let mut receivables: Vec<&Receivable> = cli_client.cached_receivable.values().collect();
            receivables.sort_by(|a, b| b.amount.cmp(&a.amount));
            if receivables.is_empty() {
                println!("No transactions to receive.");
            } else {
                println!("Specify which transactions to receive by account (-a) or block (-b):");
            }
            for receivable in receivables {
                println!(
                    "{}: {} ({} Nano)",
                    receivable.recipient,
                    hex::encode_upper(receivable.block_hash),
                    Amount::from(receivable.amount)
                );
            }
            return Ok(true);
        }

        Ok(true)
    }
}

#[derive(Debug, Args)]
struct RefreshArgs {}
impl RefreshArgs {
    async fn execute<Frontend: CliFrontend>(
        self,
        frontend: &mut Frontend,
    ) -> Result<bool, ClientError> {
        let cli_client = frontend.client_mut();
        Frontend::print("Downloading receivable transactions...");
        let client = &mut cli_client.internal;
        let accounts = client.wallet_db.all_nano_accounts();
        let receivables = client.download_receivable(&accounts).await?;
        let (receivables, infos) = client.handle_rpc_success(receivables);

        client.wallet_db.derived_account_db.insert_many(infos);
        for account in &accounts {
            cli_client.remove_receivable(account);
        }
        cli_client.insert_receivable(receivables);

        Frontend::print("Updating account frontiers...");
        let client = &mut cli_client.internal;
        let accounts = client.wallet_db.all_nano_accounts();
        let frontiers = client.download_frontiers(&accounts).await?;
        let frontiers = client.handle_rpc_success(frontiers);
        client.set_new_frontiers(frontiers);

        Frontend::print("Done");
        Ok(true)
    }
}

#[derive(Debug, Args)]
struct RemoveArgs {
    /// The nano_ or camo_ account to remove
    account: ParsedAccount,
}
impl RemoveArgs {
    async fn execute<Frontend: CliFrontend>(
        self,
        frontend: &mut Frontend,
    ) -> Result<bool, ClientError> {
        let cli_client = frontend.client_mut();
        if let ParsedAccount::Nano(account) = self.account {
            cli_client.remove_account(&account)?;
        } else if let ParsedAccount::Camo(camo) = self.account {
            cli_client.remove_camo_account(&camo)?;
        } else {
            Frontend::print("Please specify an account to remove");
            return Err(ClientError::InvalidArguments);
        }
        Ok(true)
    }
}

#[derive(Debug, Args)]
struct RescanArgs {
    /// The camo_ account to rescan
    account: CamoAccount,
    /// The block to use as the starting point (default is the account's frontier)
    #[arg(short, long)]
    head: Option<Hex32Bytes>,
    /// Do not filter worthless accounts ("worthless" means 0 balance or pending transactions)
    #[arg(short = 'f', long, default_value_t = false)]
    no_filter: bool,
}
impl RescanArgs {
    async fn execute<Frontend: CliFrontend>(
        self,
        frontend: &mut Frontend,
    ) -> Result<bool, ClientError> {
        let cli_client = frontend.client_mut();
        let client = &mut cli_client.internal;

        let filter = !self.no_filter;
        let account = self.account.signer_account();

        let db_head = client
            .frontiers_db
            .account_frontier(&account)
            .map(|frontier| frontier.block.hash());
        let head = self.head.map(|head| head.0).or(db_head);

        if let Some(head) = head {
            let batch_size = client.config.RPC_ACCOUNT_HISTORY_BATCH_SIZE;

            let head_info_success = RpcManager()
                .block_info(&client.config, head)
                .await?;
            let (head_info, mut rpc_failures) = head_info_success.into();
            let head_height = head_info.map(|info| info.height).unwrap_or(0);

            let bottom_height = head_height.saturating_sub(batch_size);
            Frontend::print(&format!(
                "Scanning {} blocks ({} -> {})...",
                min(head_height, batch_size),
                head_height,
                bottom_height
            ));
            let (rescan, rescan_rpc_failures) = client
                .rescan_notifications_partial(&self.account, Some(head), None, filter)
                .await?
                .into();
            rpc_failures.merge_with(rescan_rpc_failures);

            if let Some(head) = rescan.new_head {
                if head != [0; 32] {
                    Frontend::print(&format!("Ended on block: {}", hex::encode(head)));
                }
            }

            cli_client.handle_rescan(rescan);
        } else {
            Frontend::print("No blocks to scan. Maybe refresh?");
        }
        Frontend::print("Done");
        Ok(true)
    }
}

#[derive(Debug, Args)]
struct SeedArgs {}
impl SeedArgs {
    fn execute<Frontend: CliFrontend>(self, frontend: &Frontend) -> Result<bool, ClientError> {
        frontend.authenticate()?;
        Frontend::print(&frontend.client().internal.seed.as_hex().to_string());
        Ok(true)
    }
}

#[derive(Debug, Args)]
struct SendArgs {
    /// Sender nano_ account (use 'any' to automatically select one)
    sender: Account,
    /// Amount of Nano to send to the recipient
    amount: Amount,
    /// Recipient nano_ account
    recipient: Account,
    /// Set a new representative account
    #[arg(short, long)]
    representative: Option<Account>,
}
impl SendArgs {
    async fn execute<Frontend: CliFrontend>(
        self,
        frontend: &mut Frontend,
    ) -> Result<bool, ClientError> {
        let cli_client = frontend.client_mut();
        let work_client = &mut cli_client.work_client;
        let client = &mut cli_client.internal;

        let payment = Payment {
            sender: self.sender,
            amount: self.amount.into(),
            recipient: self.recipient,
            new_representative: self.representative,
        };
        Frontend::print("Sending...");
        let success = client.send(work_client, payment).await?;

        let frontiers = client.handle_rpc_success(success);
        client.set_new_frontiers(frontiers);
        Frontend::print("Done");
        Ok(true)
    }
}

#[derive(Debug, Args)]
struct SendCamoArgs {
    /// Sender nano_ account
    sender: Account,
    /// Total amount of Nano to send to the recipient
    amount: Amount,
    /// Recipient camo_ account
    recipient: CamoAccount,
    /// Automatically choose a notifier account and notification amount (disable for best privacy)
    #[arg(short, long, default_value_t = false)]
    auto: bool,
    /// Notifier nano_ account
    #[arg(short, long)]
    notifier: Option<Account>,
    /// Amount of Nano that the notifier account should send (subtracted from `amount`)
    #[arg(short = 'A', long)]
    notifier_amount: Option<Amount>,
}
impl SendCamoArgs {
    async fn execute<Frontend: CliFrontend>(
        self,
        frontend: &mut Frontend,
    ) -> Result<bool, ClientError> {
        let cli_client = frontend.client_mut();
        let work_client = &mut cli_client.work_client;
        let client = &mut cli_client.internal;

        let notifier_amount = if let Some(notifier_amount) = self.notifier_amount {
            // if a notifier amount was given
            notifier_amount.value
        } else if self.auto {
            // if a notifier account was NOT given (must be selected automatically)
            CAMO_SENDER_DUST_THRESHOLD
        } else {
            Frontend::print("'notification_amount' is required if 'auto' is not set");
            return Err(ClientError::InvalidArguments);
        };

        if notifier_amount < CAMO_SENDER_DUST_THRESHOLD {
            return Err(ClientError::AmountBelowDustThreshold);
        }
        if self.amount.value < max(notifier_amount, CAMO_SENDER_DUST_THRESHOLD) {
            return Err(ClientError::AmountBelowDustThreshold);
        }

        let notifier = if let Some(notifier) = self.notifier {
            // if a notifier account was given
            notifier
        } else if self.auto {
            // if a notifier account was NOT given (must be selected automatically)
            let auto_selected = client.accounts_with_balance(
                notifier_amount,
                &[self.sender.clone(), self.recipient.signer_account()],
            );
            match auto_selected.first() {
                // if another account can be automatically selected
                Some(info) => info.block.account.clone(),
                // if no accounts have the necessary balance, use the same account
                None => self.sender.clone(),
            }
        } else {
            Frontend::print("'notifier' is required if 'auto' is not set");
            return Err(ClientError::InvalidArguments);
        };

        let sender_amount = self.amount.value - notifier_amount;
        let payment = CamoPayment {
            sender: self.sender,
            sender_amount,
            notifier: notifier.clone(),
            notification_amount: notifier_amount,
            recipient: self.recipient.clone(),
        };

        // create the transaction summary
        let (_, notification) = client.camo_transaction_memo(&payment)?;
        let Notification::V1(notification) = &notification;
        let tx_summary = CamoTxSummary {
            recipient: self.recipient,
            camo_amount: sender_amount,
            total_amount: self.amount.value,
            notification: notification.representative_payload.compressed.to_bytes(),
        };
        if cli_client.camo_history.first() != Some(&tx_summary) {
            cli_client.camo_history.insert(0, tx_summary);
        }

        Frontend::print("Sending...");
        let success = client.send_camo(work_client, payment).await?;

        let frontiers = client.handle_rpc_success(success);
        client.set_new_frontiers(frontiers);
        Frontend::print("Done");
        Ok(true)
    }
}

#[derive(Debug, Args)]
struct QuitArgs {}
impl QuitArgs {
    fn execute(self) -> Result<bool, ClientError> {
        Ok(false)
    }
}
