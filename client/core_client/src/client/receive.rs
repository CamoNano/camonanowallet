use super::{choose_representatives, CoreClient};
use crate::error::CoreClientError;
use crate::frontiers::{FrontierInfo, NewFrontiers};
use crate::rpc::{ClientRpc, RpcManager, RpcResult, RpcSuccess};
use crate::workserver::WorkClient;
use nanopyrs::{rpc::Receivable, Account, Block, BlockType, Signature};

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

/// Receive a single transaction, returning the new frontier of that account (the `receive` block).
/// **Does** cache work for the next block, if enabled.
pub async fn receive_single(
    client: &CoreClient,
    work_client: &mut WorkClient,
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
