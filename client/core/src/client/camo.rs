use super::receive::get_accounts_receivable;
use crate::client::CoreClient;
use crate::frontiers::{FrontierInfo, NewFrontiers};
use crate::rpc::{RpcManager, RpcResult, RpcSuccess};
use crate::wallet::{DerivedAccountInfo, WalletDB, WalletSeed};
use futures::future;
use log::{debug, error};
use nanopyrs::{
    camo::{CamoAccount, Notification},
    constants::CAMO_RECIPIENT_DUST_THRESHOLD,
    rpc::Receivable,
    Account, Block,
};
use zeroize::Zeroize;

#[derive(Debug, Clone, Default, Zeroize)]
pub struct RescanData {
    /// Receivable transactions
    pub receivable: Vec<Receivable>,
    /// New frontiers for the frontier DB
    pub new_frontiers: NewFrontiers,
    /// Info of derived accounts
    pub derived_info: Vec<DerivedAccountInfo>,
    /// Block that scanning ended on (`previous` field of the last scanned block)
    pub new_head: Option<[u8; 32]>,
}

/// Used to determine which derived accounts have value when re-scanning history for camo payments
fn account_has_value(
    receivable: &[Receivable],
    frontiers: &[FrontierInfo],
    account: &Account,
) -> bool {
    let has_receivable = receivable
        .iter()
        .any(|receivable| &receivable.recipient == account);
    let has_balance = frontiers
        .iter()
        .any(|frontier| &frontier.block.account == account && frontier.block.balance > 0);
    debug!("{account} has receivable: {has_receivable}, has balance: {has_balance}");
    has_receivable || has_balance
}

/// Removes worthless accounts from Vec<DerivedAccountInfo> when re-scanning history for camo payments
fn filter_worthless(
    info: Vec<DerivedAccountInfo>,
    frontiers: &NewFrontiers,
    receivable: &[Receivable],
) -> Vec<DerivedAccountInfo> {
    info.into_iter()
        .filter(|info| account_has_value(receivable, &frontiers.new, &info.account))
        .collect()
}

async fn download_notification_blocks(
    client: &CoreClient,
    hashes: &[[u8; 32]],
) -> RpcResult<Vec<Block>> {
    let (notification_blocks, rpc_failures) = RpcManager()
        .blocks_info(&client.config, hashes)
        .await?
        .into();
    let notification_blocks: Vec<Block> = notification_blocks
        .into_iter()
        .flatten()
        .map(|info| info.block.clone())
        .collect();
    Ok((notification_blocks, rpc_failures).into())
}

/// Filters out non-notification receivable transactions
async fn get_notification_blocks(
    client: &CoreClient,
    all_receivable: &[Receivable],
) -> RpcResult<Vec<Block>> {
    let hashes = all_receivable
        .iter()
        .filter(|receivable| receivable.amount >= CAMO_RECIPIENT_DUST_THRESHOLD)
        .filter(|receivable| {
            client
                .wallet_db
                .camo_account_db
                .contains_notification_account(&receivable.recipient)
        })
        .map(|receivable| receivable.block_hash)
        .collect::<Vec<[u8; 32]>>();
    download_notification_blocks(client, &hashes).await
}

/// Get the destination accounts of camo payments, given the notification blocks.
///
/// Returns `DerivedAccountInfo`'s for the wallet DB.
fn get_camo_destinations_from_blocks(
    wallet_db: &WalletDB,
    seed: &WalletSeed,
    notification_blocks: Vec<Block>,
) -> Vec<DerivedAccountInfo> {
    if notification_blocks.is_empty() {
        return vec![];
    }

    // calculate the destination account
    let mut accounts_to_scan = vec![];
    let mut derived_account_info = vec![];
    for notification_block in notification_blocks.iter() {
        let block_hash = hex::encode_upper(notification_block.hash());
        debug!("Scanning {block_hash}");

        let recipient = if let Ok(recipient) = notification_block.link_as_account() {
            recipient
        } else {
            let link = hex::encode_upper(notification_block.link);
            debug!("Invalid link field ({link}) (expected account)");
            continue;
        };

        // Get the key of the camo_ account associated with the notification accounts
        let camo_account_info = wallet_db
            .camo_account_db
            .get_info_from_notification_account(&recipient);

        let camo_account_info = match camo_account_info {
            Some(info) => info,
            None => {
                // Non-notification blocks should have been filtered earlier
                error!("Attempted to scan invalid notification block: {recipient} not in DB");
                continue;
            }
        };

        let notification = Notification::from_v1(notification_block);
        let (key, info) = seed.derive_key(camo_account_info, &notification);
        let account = key.to_account();

        debug!("Derived {account} from {block_hash}");

        accounts_to_scan.push(key.to_account());
        derived_account_info.push(info);
    }
    derived_account_info
}

/// Scan part of the notification account's history for camo notifications.
///
/// Mostly aligns with the `account_history` API,
/// but with `count` set to `config::RPC_ACCOUNT_HISTORY_BATCH_SIZE`,
/// and `offset` multiplied by `config::RPC_ACCOUNT_HISTORY_BATCH_SIZE`.
///
/// Note that the destination accounts are *not* scanned, only calculated.
async fn download_historical_notifications(
    client: &CoreClient,
    account: &CamoAccount,
    head: Option<[u8; 32]>,
    offset: Option<usize>,
) -> RpcResult<(Vec<DerivedAccountInfo>, Option<[u8; 32]>)> {
    // TODO: maybe cache account histories to avoid re-downloading?
    let (history, mut rpc_failures) = RpcManager()
        .account_history(
            &client.config,
            &account.signer_account(),
            client.config.RPC_ACCOUNT_HISTORY_BATCH_SIZE,
            head,
            offset.map(|offset| offset * client.config.RPC_ACCOUNT_HISTORY_BATCH_SIZE),
        )
        .await?
        .into();
    let new_head = history.last().map(|last| last.previous);
    debug!(
        "Found {} blocks to scan for {}",
        history.len(),
        account.signer_account()
    );

    let notification_hashes: Vec<[u8; 32]> = history.iter().map(|block| block.link).collect();
    let (blocks, blocks_failures) = download_notification_blocks(client, &notification_hashes)
        .await?
        .into();
    rpc_failures.merge_with(blocks_failures);

    let destinations_info =
        get_camo_destinations_from_blocks(&client.wallet_db, &client.seed, blocks);

    Ok(((destinations_info, new_head), rpc_failures).into())
}

/// Get the receivable camo payments, given the normal receivable payments.
/// Internally, the notification blocks are downloaded and passed to `get_camo_destinations_from_blocks()`.
///
/// Returns receivable payments, as well as `DerivedAccountInfo`'s for the wallet DB.
///
/// Note that the number of receivable payments per account that can be returned at one time is limited by `ACCOUNTS_RECEIVABLE_BATCH_SIZE`.
pub async fn get_camo_receivable(
    client: &CoreClient,
    initial_receivable: &[Receivable],
) -> RpcResult<(Vec<Receivable>, Vec<DerivedAccountInfo>)> {
    if initial_receivable.is_empty() {
        return Ok(RpcSuccess::default());
    }

    let (notification_blocks, mut rpc_failures) =
        get_notification_blocks(client, initial_receivable)
            .await?
            .into();

    let destinations_info: Vec<DerivedAccountInfo> =
        get_camo_destinations_from_blocks(&client.wallet_db, &client.seed, notification_blocks);
    let destination_accounts: Vec<Account> = destinations_info
        .iter()
        .map(|info| &info.account)
        .cloned()
        .collect();

    // get receivable transactions for derived accounts
    let (camo_receivable, rpc_failures_2) = get_accounts_receivable(client, &destination_accounts)
        .await?
        .into();
    rpc_failures.merge_with(rpc_failures_2);
    Ok(((camo_receivable, destinations_info), rpc_failures).into())
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
    client: &CoreClient,
    account: &CamoAccount,
    head: Option<[u8; 32]>,
    offset: Option<usize>,
    filter: bool,
) -> RpcResult<RescanData> {
    let ((mut info, new_head), mut rpc_failures) =
        download_historical_notifications(client, account, head, offset)
            .await?
            .into();
    let derived_accounts: Vec<Account> = info.iter().map(|info| &info.account).cloned().collect();
    let (frontiers, receivable) = future::try_join(
        client.download_frontiers(&derived_accounts),
        get_accounts_receivable(client, &derived_accounts),
    )
    .await?;

    let (frontiers, rpc_failures_1) = frontiers.into();
    rpc_failures.merge_with(rpc_failures_1);
    let (receivable, rpc_failures_2) = receivable.into();
    rpc_failures.merge_with(rpc_failures_2);

    if filter {
        // remove info of accounts with no balance AND no pending transactions
        info = filter_worthless(info, &frontiers, &receivable);
    }

    let rescan = RescanData {
        receivable,
        new_frontiers: frontiers,
        derived_info: info,
        new_head,
    };
    Ok((rescan, rpc_failures).into())
}
