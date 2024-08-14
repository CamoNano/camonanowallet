use super::balance;
use super::error::ClientError;
use super::types::{Amount, CamoTxSummary};
use super::types::{Hex32Bytes, ParsedAccount, ParsedCamoVersion};
use super::WalletFrontend;
use clap::{Args, Parser, Subcommand};
use core_client::{
    constants::CAMO_SENDER_DUST_THRESHOLD, rpc::RpcManager, Account, CamoAccount, CamoPayment,
    CamoVersion, CamoVersions, CoreClientError, Notification, NotificationV1, Payment, Receivable,
};
use std::cmp::{max, min};

fn notification_payload_bytes(notification: Notification) -> [u8; 32] {
    let Notification::V1(notification) = &notification;
    notification.representative_payload.compressed.to_bytes()
}

#[derive(Debug, Parser)]
#[command(no_binary_name = true, arg_required_else_help = true)]
#[command(version, name = "")]
pub struct Command {
    #[clap(subcommand)]
    command: CommandType,
}
impl Command {
    /// `Ok(true)` means continue looping, `Ok(false)` means exit.
    pub async fn execute<Frontend: WalletFrontend>(
        frontend: &mut Frontend,
        command: &str,
    ) -> Result<bool, ClientError> {
        frontend.client_mut().update_work_cache().await?;

        let command = command.split_whitespace();
        let command = match Command::try_parse_from(command) {
            Ok(command) => command,
            Err(err) => {
                Frontend::println(&err.to_string());
                return Ok(true);
            }
        };

        let result = match command.command {
            CommandType::RecoverNotification(args) => args.execute(frontend),
            CommandType::AckNotification(args) => args.execute(frontend),
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
        }?;

        frontend.client_mut().update_work_cache().await?;
        Ok(result)
    }
}

#[derive(Debug, Subcommand)]
enum CommandType {
    /// Dev tool - recover a Camo notification
    #[clap(hide = true, name = "dev_recover_notification")]
    RecoverNotification(RecoverNotificationArgs),
    /// Dev tool - acknowledge a Camo notification
    #[clap(hide = true, name = "dev_ack_notification")]
    AckNotification(AckNotificationArgs),
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
struct RecoverNotificationArgs {
    /// Sender nano_ account (ours)
    sender: Account,
    /// Recipient camo_ account (theirs)
    recipient: CamoAccount,
    /// Hash of the sender's frontier at desired point of derivation
    frontier: Hex32Bytes,
}
impl RecoverNotificationArgs {
    fn execute<Frontend: WalletFrontend>(
        self,
        frontend: &mut Frontend,
    ) -> Result<bool, ClientError> {
        let client = &mut frontend.client_mut().core;
        let seed = &client.seed;
        if let Some(key) = client.wallet_db.find_key(seed, &self.sender) {
            let (_, notification) = self.recipient.sender_ecdh(&key, self.frontier.0);
            let notification = hex::encode(notification_payload_bytes(notification));
            Frontend::println(&format!("Notification: {notification}"));
            Ok(true)
        } else {
            Frontend::println(&format!("We must know the private key for {}", self.sender));
            Err(CoreClientError::AccountNotFound.into())
        }
    }
}

#[derive(Debug, Args)]
struct AckNotificationArgs {
    /// Recipient camo_ account (ours)
    recipient: CamoAccount,
    /// Camo transaction notification
    notification: Hex32Bytes,
}
impl AckNotificationArgs {
    fn execute<Frontend: WalletFrontend>(
        self,
        frontend: &mut Frontend,
    ) -> Result<bool, ClientError> {
        let client = &mut frontend.client_mut().core;
        let seed = &client.seed;
        if let Some(info) = client.wallet_db.camo_account_db.get_info(&self.recipient) {
            let notification = NotificationV1 {
                recipient: self.recipient.signer_account(),
                representative_payload: Account::from_bytes(self.notification.0)?,
            };
            let (_, info) = seed.derive_key(info, &Notification::V1(notification));
            client.wallet_db.derived_account_db.insert(info);

            Frontend::println("Done");
            Ok(true)
        } else {
            Frontend::println(&format!(
                "We must know the private key for {}",
                self.recipient
            ));
            Err(CoreClientError::AccountNotFound.into())
        }
    }
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
    async fn execute<Frontend: WalletFrontend>(
        self,
        frontend: &mut Frontend,
    ) -> Result<bool, ClientError> {
        let client = frontend.client_mut();
        let core_client = &mut client.core;

        let string = if self.camo {
            let mut versions = core_client.config.DEFAULT_CAMO_VERSIONS.clone();
            if let Some(v) = self.versions {
                versions = v.iter().map(|v| v.0).collect::<Vec<CamoVersion>>()
            }

            let (key, info) = core_client
                .seed
                .get_camo_key(self.index, CamoVersions::new(&versions))
                .ok_or(ClientError::InvalidArguments)?;
            core_client
                .wallet_db
                .camo_account_db
                .insert(&core_client.config, info)?;
            key.to_camo_account().to_string()
        } else {
            if self.versions.is_some() {
                Frontend::println("The 'versions' option is only used for camo accounts");
                return Err(ClientError::InvalidArguments);
            }
            let (key, info) = core_client.seed.get_key(self.index);
            core_client
                .wallet_db
                .account_db
                .insert(&core_client.config, info)?;
            key.to_account().to_string()
        };

        let downloaded = core_client.download_unknown_frontiers().await?;
        let downloaded = core_client.handle_rpc_success(downloaded);
        core_client.set_new_frontiers(downloaded);

        Frontend::println(&string);
        Ok(true)
    }
}

#[derive(Debug, Args)]
struct BalanceArgs {}
impl BalanceArgs {
    fn execute<Frontend: WalletFrontend>(self, frontend: &Frontend) -> Result<bool, ClientError> {
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
    fn execute<Frontend: WalletFrontend>(
        self,
        frontend: &mut Frontend,
    ) -> Result<bool, ClientError> {
        let client = frontend.client_mut();
        if self.clear {
            client.camo_history.clear();
            return Ok(true);
        }

        for (i, payment) in client.camo_history.iter().enumerate() {
            if i == self.count {
                break;
            }
            Frontend::println(&payment.to_string());
        }

        Ok(true)
    }
}

#[derive(Debug, Args)]
struct ClearArgs {}
impl ClearArgs {
    fn execute<Frontend: WalletFrontend>(self) -> Result<bool, ClientError> {
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
    async fn execute<Frontend: WalletFrontend>(
        self,
        frontend: &mut Frontend,
    ) -> Result<bool, ClientError> {
        let client = frontend.client_mut();
        let core_client = &mut client.core;

        let accounts = if !self.accounts.is_empty() {
            self.accounts
        } else if self.all {
            core_client.frontiers_db.all_accounts()
        } else {
            Frontend::println("Please specify which work caches to clear");
            return Err(ClientError::InvalidArguments);
        };

        for account in accounts {
            if let Some(frontier) = core_client.frontiers_db.account_frontier_mut(&account) {
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
    async fn execute<Frontend: WalletFrontend>(
        self,
        frontend: &mut Frontend,
    ) -> Result<bool, ClientError> {
        let client = frontend.client_mut();
        let work_client = &mut client.work;
        let core_client = &mut client.core;

        if self.amount.value < CAMO_SENDER_DUST_THRESHOLD {
            return Err(ClientError::AmountBelowDustThreshold);
        }

        let payment = Payment {
            sender: self.notifier,
            amount: self.amount.into(),
            recipient: self.recipient.signer_account(),
            new_representative: Some(Account::from_bytes(self.notification.0)?),
        };
        Frontend::println("Sending...");
        let success = core_client.send(work_client, payment).await?;

        let frontiers = core_client.handle_rpc_success(success);
        core_client.set_new_frontiers(frontiers);
        Frontend::println("Done");
        Ok(true)
    }
}

#[derive(Debug, Args)]
struct ReceiveArgs {
    /// List receivable transactions (default behavior)
    #[arg(short, long, conflicts_with = "blocks", conflicts_with = "accounts")]
    list: bool,
    /// The block hashes to receive
    #[arg(short, long, conflicts_with = "accounts", conflicts_with = "list")]
    blocks: Vec<Hex32Bytes>,
    /// The accounts to receive transactions on
    #[arg(short, long, conflicts_with = "blocks", conflicts_with = "list")]
    accounts: Vec<Account>,
}
impl ReceiveArgs {
    async fn execute<Frontend: WalletFrontend>(
        self,
        frontend: &mut Frontend,
    ) -> Result<bool, ClientError> {
        let client = frontend.client_mut();
        let work_client = &mut client.work;
        let core_client = &mut client.core;
        let cached_receivable = &mut client.receivable;

        let receivables: Vec<Receivable> = if !self.blocks.is_empty() {
            self.blocks
                .into_iter()
                .map(|receivable| cached_receivable.remove(&receivable.0))
                .collect::<Option<Vec<Receivable>>>()
                .ok_or(CoreClientError::AccountNotFound)?
        } else if !self.accounts.is_empty() {
            cached_receivable
                .iter()
                .filter(|receivable| self.accounts.contains(&receivable.1.recipient))
                .map(|receivable| receivable.0)
                .cloned()
                .collect::<Vec<[u8; 32]>>()
                .into_iter()
                .map(|receivable| cached_receivable.remove(&receivable))
                .collect::<Option<Vec<Receivable>>>()
                .ok_or(CoreClientError::AccountNotFound)?
        } else {
            let mut receivables: Vec<&Receivable> = cached_receivable.values().collect();
            receivables.sort_by(|a, b| b.amount.cmp(&a.amount));
            if receivables.is_empty() {
                Frontend::println("No transactions to receive.");
            } else {
                Frontend::println(
                    "Specify which transactions to receive by account (-a) or block (-b):",
                );
            }
            for receivable in receivables {
                Frontend::println(&format!(
                    "{}: {} ({} Nano)",
                    receivable.recipient,
                    hex::encode_upper(receivable.block_hash),
                    Amount::from(receivable.amount)
                ));
            }
            return Ok(true);
        };

        Frontend::println("Receiving...");
        let result = core_client.receive(work_client, receivables).await;
        let frontiers = core_client.handle_rpc_success(result.successes);
        core_client.set_new_frontiers(frontiers);

        let (return_value, unreceived) = if let Err(err) = result.failures {
            (Err(err.err.into()), err.unreceived)
        } else {
            (Ok(true), vec![])
        };

        client.insert_receivable(unreceived);
        Frontend::println("Done");
        return_value
    }
}

#[derive(Debug, Args)]
struct RefreshArgs {}
impl RefreshArgs {
    async fn execute<Frontend: WalletFrontend>(
        self,
        frontend: &mut Frontend,
    ) -> Result<bool, ClientError> {
        let client = frontend.client_mut();
        Frontend::println("Downloading receivable transactions...");
        let core_client = &mut client.core;
        let accounts = core_client.wallet_db.all_nano_accounts();
        let receivables = core_client.download_receivable(&accounts).await?;
        let (receivables, infos) = core_client.handle_rpc_success(receivables);

        core_client.wallet_db.derived_account_db.insert_many(infos);
        for account in &accounts {
            client.remove_receivable(account);
        }
        client.insert_receivable(receivables);

        Frontend::println("Updating account frontiers...");
        let core_client = &mut client.core;
        let frontiers = core_client.download_frontiers(&accounts).await?;
        let frontiers = core_client.handle_rpc_success(frontiers);
        core_client.set_new_frontiers(frontiers);

        Frontend::println("Done");
        Ok(true)
    }
}

#[derive(Debug, Args)]
struct RemoveArgs {
    /// The nano_ or camo_ account to remove
    account: ParsedAccount,
}
impl RemoveArgs {
    async fn execute<Frontend: WalletFrontend>(
        self,
        frontend: &mut Frontend,
    ) -> Result<bool, ClientError> {
        let client = frontend.client_mut();
        if let ParsedAccount::Nano(account) = self.account {
            client.remove_account(&account)?;
        } else if let ParsedAccount::Camo(camo) = self.account {
            client.remove_camo_account(&camo)?;
        } else {
            Frontend::println("Please specify an account to remove");
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
    async fn execute<Frontend: WalletFrontend>(
        self,
        frontend: &mut Frontend,
    ) -> Result<bool, ClientError> {
        let client = frontend.client_mut();
        let core_client = &mut client.core;

        let filter = !self.no_filter;
        let account = self.account.signer_account();

        let db_head = core_client
            .frontiers_db
            .account_frontier(&account)
            .map(|frontier| frontier.block.hash());
        let head = self.head.map(|head| head.0).or(db_head);

        if let Some(head) = head {
            let batch_size = core_client.config.RPC_ACCOUNT_HISTORY_BATCH_SIZE;

            let head_info_success = RpcManager().block_info(&core_client.config, head).await?;
            let (head_info, mut rpc_failures) = head_info_success.into();
            let head_height = head_info.map(|info| info.height).unwrap_or(0);

            let bottom_height = head_height.saturating_sub(batch_size);
            Frontend::println(&format!(
                "Scanning {} blocks ({} -> {})...",
                min(head_height, batch_size),
                head_height,
                bottom_height
            ));
            let (rescan, rescan_rpc_failures) = core_client
                .rescan_notifications_partial(&self.account, Some(head), None, filter)
                .await?
                .into();
            rpc_failures.merge_with(rescan_rpc_failures);

            if let Some(head) = rescan.new_head {
                if head != [0; 32] {
                    Frontend::println(&format!("Ended on block: {}", hex::encode(head)));
                }
            }

            client.handle_rescan(rescan);
        } else {
            Frontend::println("No blocks to scan. Maybe refresh?");
        }
        Frontend::println("Done");
        Ok(true)
    }
}

#[derive(Debug, Args)]
struct SeedArgs {}
impl SeedArgs {
    fn execute<Frontend: WalletFrontend>(self, frontend: &Frontend) -> Result<bool, ClientError> {
        frontend.authenticate()?;
        Frontend::println(&frontend.client().core.seed.as_hex().to_string());
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
    async fn execute<Frontend: WalletFrontend>(
        self,
        frontend: &mut Frontend,
    ) -> Result<bool, ClientError> {
        let client = frontend.client_mut();
        let work_client = &mut client.work;
        let core_client = &mut client.core;

        let payment = Payment {
            sender: self.sender,
            amount: self.amount.into(),
            recipient: self.recipient,
            new_representative: self.representative,
        };
        Frontend::println("Sending...");
        let success = core_client.send(work_client, payment).await?;

        let frontiers = core_client.handle_rpc_success(success);
        core_client.set_new_frontiers(frontiers);
        Frontend::println("Done");
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
    async fn execute<Frontend: WalletFrontend>(
        self,
        frontend: &mut Frontend,
    ) -> Result<bool, ClientError> {
        let client = frontend.client_mut();
        let work_client = &mut client.work;
        let core_client = &mut client.core;

        let notifier_amount = if let Some(notifier_amount) = self.notifier_amount {
            // if a notifier amount was given
            notifier_amount.value
        } else if self.auto {
            // if a notifier account was NOT given (must be selected automatically)
            CAMO_SENDER_DUST_THRESHOLD
        } else {
            Frontend::println("'notification_amount' is required if 'auto' is not set");
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
            let auto_selected = core_client.accounts_with_balance(
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
            Frontend::println("'notifier' is required if 'auto' is not set");
            return Err(ClientError::InvalidArguments);
        };

        if self.auto {
            Frontend::println(&format!("Automatically selected {notifier} as notifier"));
            Frontend::println(&format!(
                "Automatically selected {} Nano as notification amount",
                Amount::from(notifier_amount)
            ));
        }

        let sender_amount = self.amount.value - notifier_amount;
        let payment = CamoPayment {
            sender: self.sender,
            sender_amount,
            notifier: notifier.clone(),
            notification_amount: notifier_amount,
            recipient: self.recipient.clone(),
        };

        // create the transaction summary
        let (_, notification) = core_client.camo_transaction_memo(&payment)?;
        let tx_summary = CamoTxSummary {
            recipient: self.recipient,
            camo_amount: sender_amount,
            total_amount: self.amount.value,
            notification: notification_payload_bytes(notification),
        };
        if client.camo_history.first() != Some(&tx_summary) {
            client.camo_history.insert(0, tx_summary);
        }

        Frontend::println("Sending...");
        let success = core_client.send_camo(work_client, payment).await?;

        let frontiers = core_client.handle_rpc_success(success);
        core_client.set_new_frontiers(frontiers);
        Frontend::println("Done");
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
