use super::{choose_representatives, CoreClient};
use crate::error::CoreClientError;
use crate::frontiers::{FrontierInfo, NewFrontiers};
use crate::rpc::{ClientRpc, RpcFailures, RpcResult};
use crate::work::WorkManager;
use log::info;
use nanopyrs::{
    camo::{CamoAccount, Notification},
    Account, Block, BlockType, Key, SecretBytes, Signature,
};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Debug, Clone, Zeroize, ZeroizeOnDrop, Serialize, Deserialize)]
pub struct Payment {
    pub sender: Account,
    pub amount: u128,
    pub recipient: Account,
    pub new_representative: Option<Account>,
}

#[derive(Debug, Clone, Zeroize, ZeroizeOnDrop, Serialize, Deserialize)]
pub struct CamoPayment {
    pub sender: Account,
    pub sender_amount: u128,

    pub notifier: Account,
    pub notification_amount: u128,

    pub recipient: CamoAccount,
}

pub(super) fn sender_ecdh(
    client: &CoreClient,
    recipient: &CamoAccount,
    sender_key: &Key,
) -> Result<(SecretBytes<32>, Notification), CoreClientError> {
    let frontier = client
        .frontiers_db
        .account_frontier(&sender_key.to_account())
        .ok_or(CoreClientError::AccountNotFound)?
        .block
        .hash();
    Ok(recipient.sender_ecdh(sender_key, frontier))
}

/// Create a signed `send` block with the given parameters.
///
/// Cached proof-of-work will be used, if there is any.
/// Otherwise, the `work` field is left blank.
fn create_send_block(
    client: &CoreClient,
    payment: Payment,
    sender_frontier: &FrontierInfo,
) -> Result<Block, CoreClientError> {
    if payment.sender == payment.recipient {
        return Err(CoreClientError::InvalidPayment);
    }

    let work = sender_frontier.cached_work().unwrap_or([0; 8]);
    let frontier = &sender_frontier.block;

    // sanity check balance
    if payment.amount > frontier.balance {
        return Err(CoreClientError::NotEnoughCoins);
    }

    let previous = if sender_frontier.is_unopened() {
        // should never occur with a send block, but just in case
        [0; 32]
    } else {
        sender_frontier.block.hash()
    };

    let representative = choose_representatives(
        &client.config,
        sender_frontier.block.representative.clone(),
        payment.new_representative.clone(),
    );

    let block = Block {
        block_type: BlockType::Send,
        account: payment.sender.clone(),
        previous,
        representative,
        balance: frontier.balance - payment.amount,
        link: payment.recipient.compressed.to_bytes(),
        signature: Signature::default(),
        work,
    };
    client.wallet_db.sign_block(&client.seed, block)
}

/// Send to a `nano_` account.
/// **Does** cache work for the next block, if enabled.
pub async fn send(
    client: &CoreClient,
    work_client: &mut WorkManager,
    payment: Payment,
) -> RpcResult<NewFrontiers> {
    if payment.sender == payment.recipient {
        return Err(CoreClientError::InvalidPayment);
    }

    let frontier = &client
        .frontiers_db
        .account_frontier(&payment.sender)
        .ok_or(CoreClientError::AccountNotFound)?;
    let send_block = create_send_block(client, payment, frontier)?;
    let (info, rpc_failures) = ClientRpc()
        .auto_publish_unsynced(&client.config, work_client, frontier, send_block)
        .await?
        .into();
    Ok((vec![info].into(), rpc_failures).into())
}

/// Publish both blocks: Notification first, to minimize damage if an error occurs.
/// **Does not** cache work for the next block.
async fn camo_auto_publish_blocks(
    client: &CoreClient,
    notification_block: Block,
    send_block: Block,
) -> RpcResult<(FrontierInfo, FrontierInfo)> {
    let mut rpc_failures = RpcFailures::default();
    let (notification_frontier, notification_failures) = ClientRpc()
        .publish(&client.config, notification_block)
        .await?
        .into();
    let (send_frontier, send_failures) = ClientRpc()
        .publish(&client.config, send_block)
        .await?
        .into();
    rpc_failures.merge_with(notification_failures);
    rpc_failures.merge_with(send_failures);
    Ok(((notification_frontier, send_frontier), rpc_failures).into())
}

/// Send to a `camo_` account, where the sender and notifier the same account.
/// **Does** cache work for the next block, if enabled.
async fn _send_camo_same(
    client: &CoreClient,
    work_client: &mut WorkManager,
    payment: CamoPayment,
) -> RpcResult<NewFrontiers> {
    assert!(
        payment.sender == payment.notifier,
        "broken send_camo code: _send_camo_same used for non-identical sender and notifier"
    );

    let sender_frontier = &client
        .frontiers_db
        .account_frontier(&payment.sender)
        .ok_or(CoreClientError::AccountNotFound)?;

    let total_amount = payment.notification_amount + payment.sender_amount;
    if sender_frontier.block.balance < total_amount {
        return Err(CoreClientError::NotEnoughCoins);
    }

    let sender_key = client
        .wallet_db
        .find_key(&client.seed, &payment.sender)
        .ok_or(CoreClientError::AccountNotFound)?;

    let (shared_secret, notification) = sender_ecdh(client, &payment.recipient, &sender_key)?;
    let Notification::V1(notification) = &notification;
    let derived = payment.recipient.derive_account(&shared_secret);

    let send_block = create_send_block(
        client,
        Payment {
            sender: payment.sender.clone(),
            amount: payment.sender_amount,
            recipient: derived,
            new_representative: None,
        },
        sender_frontier,
    )?;

    let notify_block = create_send_block(
        client,
        Payment {
            sender: payment.notifier.clone(),
            amount: payment.notification_amount,
            recipient: notification.recipient.clone(),
            new_representative: Some(notification.representative_payload.clone()),
        },
        sender_frontier,
    )?;

    // Publish both blocks: Notification first, to minimize damage if an error occurs
    info!("Creating notifier transaction (this might take a while)...");
    let (sender_frontier, mut rpc_failures) = ClientRpc()
        .auto_publish_unsynced(&client.config, work_client, sender_frontier, notify_block)
        .await?
        .into();
    info!("Creating sender transaction (this might take a while)...");
    let (sender_frontier, rpc_failures_2) = ClientRpc()
        .auto_publish_unsynced(&client.config, work_client, &sender_frontier, send_block)
        .await?
        .into();
    rpc_failures.merge_with(rpc_failures_2);

    Ok((vec![sender_frontier].into(), rpc_failures).into())
}

/// Send to a `camo_` account.
/// **Does** cache work for the next block, if enabled.
pub async fn send_camo(
    client: &CoreClient,
    work_client: &mut WorkManager,
    payment: CamoPayment,
) -> RpcResult<NewFrontiers> {
    if payment.sender == payment.recipient.signer_account() {
        return Err(CoreClientError::InvalidPayment);
    }
    if payment.notifier == payment.recipient.signer_account() {
        return Err(CoreClientError::InvalidPayment);
    }

    let config = &client.config;

    if payment.notifier == payment.sender {
        return _send_camo_same(client, work_client, payment).await;
    }

    let mut rpc_failures = RpcFailures::default();

    let sender_frontier = &client
        .frontiers_db
        .account_frontier(&payment.sender)
        .ok_or(CoreClientError::AccountNotFound)?;
    let notifier_frontier = &client
        .frontiers_db
        .account_frontier(&payment.notifier)
        .ok_or(CoreClientError::AccountNotFound)?;

    // ensure that we have work for both blocks
    let notification_work = ClientRpc().get_work(config, work_client, notifier_frontier)?;
    let send_work = ClientRpc().get_work(config, work_client, sender_frontier)?;
    let (notification_work, work_failures_1) = notification_work.into();
    let (send_work, work_failures_2) = send_work.into();
    rpc_failures.merge_with(work_failures_1);
    rpc_failures.merge_with(work_failures_2);

    info!("Creating sender block...");
    let sender_key = client
        .wallet_db
        .find_key(&client.seed, &payment.sender)
        .ok_or(CoreClientError::AccountNotFound)?;

    let (shared_secret, notification) = sender_ecdh(client, &payment.recipient, &sender_key)?;
    let Notification::V1(notification) = &notification;

    // calculate masked account, and create send block
    let derived = payment.recipient.derive_account(&shared_secret);
    let mut send_block = create_send_block(
        client,
        Payment {
            sender: payment.sender.clone(),
            amount: payment.sender_amount,
            recipient: derived,
            new_representative: None,
        },
        sender_frontier,
    )?;
    send_block.work = send_work;

    info!("Creating notifier block...");
    let mut notification_block = create_send_block(
        client,
        Payment {
            sender: payment.notifier.clone(),
            amount: payment.notification_amount,
            recipient: notification.recipient.clone(),
            new_representative: Some(notification.representative_payload.clone()),
        },
        notifier_frontier,
    )?;
    notification_block.work = notification_work;

    // cache work for future transactions
    if config.ENABLE_WORK_CACHE {
        work_client.request_work(config, notification_block.hash());
        work_client.request_work(config, send_block.hash());
    }

    let publish_success = camo_auto_publish_blocks(client, notification_block, send_block).await?;

    let ((notification_frontier, send_frontier), publish_failures) = publish_success.into();
    rpc_failures.merge_with(publish_failures);

    let frontiers = vec![notification_frontier, send_frontier].into();

    Ok((frontiers, rpc_failures).into())
}
