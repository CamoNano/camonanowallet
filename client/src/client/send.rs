use super::{choose_representatives, Client};
use crate::error::ClientError;
use crate::frontiers::{FrontierInfo, NewFrontiers};
use crate::rpc::{RpcFailures, RpcResult};
use futures::future;
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
    client: &Client,
    recipient: &CamoAccount,
    sender_key: &Key,
) -> Result<(SecretBytes<32>, Notification), ClientError> {
    let frontier = client
        .frontiers_db
        .account_frontier(&sender_key.to_account())
        .ok_or(ClientError::AccountNotFound)?
        .block
        .hash();
    Ok(recipient.sender_ecdh(sender_key, frontier))
}

/// Create a signed `send` block with the given parameters.
///
/// Cached proof-of-work will be used, if there is any.
/// Otherwise, the `work` field is left blank.
fn create_send_block(
    client: &Client,
    payment: Payment,
    sender_frontier: &FrontierInfo,
) -> Result<Block, ClientError> {
    let work = sender_frontier.cached_work().unwrap_or([0; 8]);
    let frontier = &sender_frontier.block;

    // sanity check balance
    if payment.amount > frontier.balance {
        return Err(ClientError::NotEnoughCoins);
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
pub async fn send(client: &Client, payment: Payment) -> RpcResult<NewFrontiers> {
    let frontier = &client
        .frontiers_db
        .account_frontier(&payment.sender)
        .ok_or(ClientError::AccountNotFound)?;
    let send_block = create_send_block(client, payment, frontier)?;
    let (info, rpc_failures) = client
        .rpc()
        .auto_publish_unsynced(&client.config, frontier, send_block.clone())
        .await?
        .into();
    Ok((vec![info].into(), rpc_failures).into())
}

/// publish both blocks: notification first, to minimize damage if an error occurs
async fn camo_auto_publish_blocks(
    client: &Client,
    notification_block: Block,
    send_block: Block,
) -> RpcResult<(FrontierInfo, FrontierInfo)> {
    let mut rpc_failures = RpcFailures::default();
    let (notification_frontier, notification_failures) = client
        .rpc()
        .publish(&client.config, notification_block)
        .await?
        .into();
    let (send_frontier, send_failures) = client
        .rpc()
        .publish(&client.config, send_block)
        .await?
        .into();
    rpc_failures.merge_with(notification_failures);
    rpc_failures.merge_with(send_failures);
    Ok(((notification_frontier, send_frontier), rpc_failures).into())
}

/// Send to a `camo_` account, were the sender and notifier are identical
async fn _send_camo_same(client: &Client, payment: CamoPayment) -> RpcResult<NewFrontiers> {
    assert!(
        payment.sender == payment.notifier,
        "broken send_camo code: _send_camo_same used for non-identical sender and notifier"
    );

    let sender_frontier = &client
        .frontiers_db
        .account_frontier(&payment.sender)
        .ok_or(ClientError::AccountNotFound)?;
    let sender_key = client
        .wallet_db
        .find_key(&client.seed, &payment.sender)
        .ok_or(ClientError::AccountNotFound)?;

    let (shared_secret, notification) = sender_ecdh(client, &payment.recipient, &sender_key)?;
    let Notification::V1(notification) = &notification;
    let derived = payment.recipient.derive_account(&shared_secret);

    info!("Creating sender block...");
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
    let (sender_frontier, mut rpc_failures) = client
        .rpc()
        .auto_publish_unsynced(&client.config, sender_frontier, send_block)
        .await?
        .into();

    info!("Creating notifier block...");
    let notification_block = create_send_block(
        client,
        Payment {
            sender: payment.notifier.clone(),
            amount: payment.notification_amount,
            recipient: notification.recipient.clone(),
            new_representative: Some(notification.representative_payload.clone()),
        },
        &sender_frontier,
    )?;
    let (sender_frontier, rpc_failures_2) = client
        .rpc()
        .auto_publish_unsynced(&client.config, &sender_frontier, notification_block)
        .await?
        .into();
    rpc_failures.merge_with(rpc_failures_2);

    Ok((vec![sender_frontier].into(), rpc_failures).into())
}

/// Send to a `camo_` account.
pub async fn send_camo(client: &Client, payment: CamoPayment) -> RpcResult<NewFrontiers> {
    let config = &client.config;

    if payment.notifier == payment.sender {
        return _send_camo_same(client, payment).await;
    }

    let mut rpc_failures = RpcFailures::default();

    let sender_frontier = &client
        .frontiers_db
        .account_frontier(&payment.sender)
        .ok_or(ClientError::AccountNotFound)?;
    let notifier_frontier = &client
        .frontiers_db
        .account_frontier(&payment.notifier)
        .ok_or(ClientError::AccountNotFound)?;

    // ensure that we have work for both blocks
    let (notification_work, send_work) = future::try_join(
        client.rpc().get_work(config, notifier_frontier),
        client.rpc().get_work(config, sender_frontier),
    )
    .await?;
    let (notification_work, work_failures_1) = notification_work.into();
    let (send_work, work_failures_2) = send_work.into();
    rpc_failures.merge_with(work_failures_1);
    rpc_failures.merge_with(work_failures_2);

    info!("Creating sender block...");
    let sender_key = client
        .wallet_db
        .find_key(&client.seed, &payment.sender)
        .ok_or(ClientError::AccountNotFound)?;

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

    let internal_rpc = client.rpc().internal();
    // cache work for future transactions
    let future = future::try_join3(
        internal_rpc.work_generate(config, notification_block.hash(), None),
        internal_rpc.work_generate(config, send_block.hash(), None),
        camo_auto_publish_blocks(client, notification_block, send_block),
    );
    let (notifier_work, sender_work, publish_success) = future.await?;

    let (notifier_work, work_result_1) = notifier_work.into();
    let (sender_work, work_result_2) = sender_work.into();
    let ((mut notification_frontier, mut send_frontier), publish_failures) = publish_success.into();
    rpc_failures.merge_with(publish_failures);
    rpc_failures.merge_with(work_result_1);
    rpc_failures.merge_with(work_result_2);

    notification_frontier.set_work(config, notifier_work);
    send_frontier.set_work(config, sender_work);

    let frontiers: NewFrontiers = vec![notification_frontier, send_frontier].into();
    Ok((frontiers, rpc_failures).into())
}
