use super::{choose_representatives, CoreClient};
use crate::error::CoreClientError;
use crate::frontiers::{FrontierInfo, NewFrontiers};
use crate::rpc::{ClientRpc, RpcFailures, RpcManager, RpcResult, RpcSuccess, WorkManager};
use log::{debug, error, info};
use nanopyrs::{rpc::Receivable, Account, Block, BlockType, Signature};
use std::collections::HashMap;

#[derive(Debug)]
pub struct ReceiveFailure {
    pub err: CoreClientError,
    pub unreceived: Vec<Receivable>,
}
#[derive(Debug)]
pub struct ReceiveResult {
    /// updated frontiers of accounts with successfully-received transactions
    pub successes: RpcSuccess<NewFrontiers>,
    /// transactions which could not be received, and an error that caused it
    pub failures: Result<(), ReceiveFailure>,
}

/// Create a signed `receive` block for the given pending transaction.
///
/// Cached proof-of-work will be used, if there is any.
/// Otherwise, the `work` field is left blank.
fn create_receive_block(
    client: &CoreClient,
    receivable: &Receivable,
    recipient_frontier: &FrontierInfo,
    new_representative: Option<Account>,
) -> Result<Block, CoreClientError> {
    let account = &receivable.recipient;
    let work = recipient_frontier.cached_work().unwrap_or([0; 8]);

    // sanity check balance
    if recipient_frontier
        .block
        .balance
        .checked_add(receivable.amount)
        .is_none()
    {
        return Err(CoreClientError::FrontierBalanceOverflow);
    }

    let previous = if recipient_frontier.is_unopened() {
        [0; 32]
    } else {
        recipient_frontier.block.hash()
    };

    let representative = choose_representatives(
        &client.config,
        recipient_frontier.block.representative.clone(),
        new_representative,
    );

    let block = Block {
        block_type: BlockType::Receive,
        account: account.clone(),
        previous,
        representative,
        balance: recipient_frontier.block.balance + receivable.amount,
        link: receivable.block_hash,
        signature: Signature::default(),
        work,
    };
    client.wallet_db.sign_block(&client.seed, block)
}

/// Get the receivable payments for the given accounts.
///
/// Note that the number of receivable payments per account that can be returned at one time is limited by `ACCOUNTS_RECEIVABLE_BATCH_SIZE`.
///
/// **Does not handle camo payments.**
pub async fn get_accounts_receivable(
    client: &CoreClient,
    accounts: &[Account],
) -> RpcResult<Vec<Receivable>> {
    if accounts.is_empty() {
        return Ok(RpcSuccess::default());
    }

    let (receivable, rpc_failures) = RpcManager()
        .accounts_receivable(
            &client.config,
            accounts,
            client.config.RPC_ACCOUNTS_RECEIVABLE_BATCH_SIZE,
            client.config.NORMAL_DUST_THRESHOLD,
        )
        .await?
        .into();
    Ok((receivable.into_iter().flatten().collect(), rpc_failures).into())
}

/// Receive a single transaction, returning the new frontier of that account (a `receive` block).
/// **Does** cache work for the next block, if enabled.
pub async fn receive_block(
    client: &CoreClient,
    work_client: &mut WorkManager,
    receivable: &Receivable,
) -> RpcResult<NewFrontiers> {
    let frontier = &client
        .frontiers_db
        .account_frontier(&receivable.recipient)
        .ok_or(CoreClientError::AccountNotFound)?;
    let receive_block = create_receive_block(client, receivable, frontier, None)?;
    let (info, rpc_failures) = ClientRpc()
        .auto_publish_unsynced(&client.config, work_client, frontier, receive_block.clone())
        .await?
        .into();
    Ok((vec![info].into(), rpc_failures).into())
}

/// Receive a single transaction, returning the new frontier of that account (a `receive` block).
/// **Does** cache work for the next block, if enabled.
///
/// This is intended to be used internally, where we cannot rely on the DB being synced.
async fn receive_block_unsynced(
    client: &CoreClient,
    work_client: &mut WorkManager,
    receivable: &Receivable,
    frontier: &FrontierInfo,
) -> RpcResult<FrontierInfo> {
    let receive_block = create_receive_block(client, receivable, frontier, None)?;
    ClientRpc()
        .auto_publish_unsynced(&client.config, work_client, frontier, receive_block)
        .await
}

/// Receive a batch of transactions, returning the new frontiers of the accounts (`receive` blocks).
/// **Does** cache work for the next block, if enabled.
///
/// Transactions are processed in batches of size `config::RPC_RECEIVE_TRANSACTIONS_BATCH_SIZE`.
pub async fn receive(
    client: &CoreClient,
    work_client: &mut WorkManager,
    receivables: Vec<Receivable>,
) -> ReceiveResult {
    // Instead of relying on the database,
    // which will become out-of-sync when an account receives more than one transaction,
    // we instead create a mini-database which will be updated and eventually returned by this method.
    let mut frontiers: HashMap<Account, FrontierInfo> = HashMap::new();

    for receivable in &receivables {
        let recipient = &receivable.recipient;
        let frontier = &client.frontiers_db.account_frontier(recipient);

        if let Some(frontier) = frontier {
            frontiers.insert(receivable.recipient.clone(), (*frontier).clone());
        } else {
            let block_hash = hex::encode_upper(receivable.block_hash);
            error!("Attempted to receive transaction {block_hash} to account {recipient} with unknown frontier")
        }
    }

    let mut rpc_failures = RpcFailures::default();
    let mut err: Option<CoreClientError> = None;
    // the hashes of transactions which were NOT successfully received
    let mut unreceived: Vec<Receivable> = vec![];

    // the hashes of transactions which were successfully received
    let mut successfully_received: Vec<[u8; 32]> = vec![];

    for (i, receivable) in receivables.iter().enumerate() {
        let receive_future = receive_block_unsynced(
            client,
            work_client,
            receivable,
            frontiers
                .get(&receivable.recipient)
                .expect("Failed to catch invalid receivable transaction"),
        );

        info!(
            "Receiving transaction {} out of {}...",
            i + 1,
            receivables.len()
        );
        match receive_future.await {
            Ok(s) => {
                successfully_received.push(s.item.block.link);
                frontiers.insert(s.item.block.account.clone(), s.item);
                rpc_failures.merge_with(s.failures);
            }
            // we don't care if the error is overwritten with another error
            Err(e) => err = Some(e),
        }
    }

    // identify which receivable transactions were not successfully received
    for receivable in receivables {
        if !successfully_received.contains(&receivable.block_hash) {
            let block_hash = hex::encode_upper(receivable.block_hash);
            let recipient = &receivable.recipient;
            debug!("Unreceived transaction {block_hash} for {recipient}");

            unreceived.push(receivable)
        }
    }

    // if there is no error, then there should be no unreceived transactions, and vice versa
    assert!(
        err.is_none() == unreceived.is_empty(),
        "broken receive code"
    );

    let frontiers: Vec<FrontierInfo> = frontiers.into_values().collect();
    let unreceived: Result<(), ReceiveFailure> = match err {
        Some(err) => Err(ReceiveFailure { err, unreceived }),
        None => Ok(()),
    };
    ReceiveResult {
        successes: (frontiers.into(), rpc_failures).into(),
        failures: unreceived,
    }
}
