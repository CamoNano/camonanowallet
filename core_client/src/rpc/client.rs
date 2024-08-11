use super::RpcManager;
use crate::config::CoreClientConfig;
use crate::error::CoreClientError;
use crate::frontiers::{FrontierInfo, FrontiersDB, NewFrontiers};
use crate::rpc::{RpcFailures, RpcResult, RpcSuccess};
use futures::future;
use nanopyrs::{Account, Block};
use serde::{Deserialize, Serialize};
use std::iter::zip;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientRpc();
impl ClientRpc {
    pub fn internal(&self) -> RpcManager {
        RpcManager()
    }

    /// Get work for this frontier, either cached locally or from an RPC
    pub async fn get_work(
        &self,
        config: &CoreClientConfig,
        frontier: &FrontierInfo,
    ) -> RpcResult<[u8; 8]> {
        if let Some(work) = frontier.cached_work() {
            return Ok((work, RpcFailures::default()).into());
        }
        self.internal()
            .work_generate(config, frontier.work_hash(), None)
            .await
    }

    /// Publish a block to the network
    pub async fn publish(
        &self,
        config: &CoreClientConfig,
        block: Block,
    ) -> RpcResult<FrontierInfo> {
        let (_, failures) = self.internal().process(config, &block).await?.into();
        let info = FrontierInfo::new(block, None);
        Ok((info, failures).into())
    }

    /// Download the frontiers of the given accounts.
    pub async fn download_frontiers(
        &self,
        config: &CoreClientConfig,
        frontiers_db: &FrontiersDB,
        accounts: &[Account],
    ) -> RpcResult<NewFrontiers> {
        let mut new_frontiers = NewFrontiers::default();
        if accounts.is_empty() {
            return Ok(RpcSuccess {
                item: new_frontiers,
                failures: RpcFailures::default(),
            });
        }

        let (raw_hashes, mut failures) = self
            .internal()
            .accounts_frontiers(config, accounts)
            .await?
            .into();

        let mut hashes: Vec<[u8; 32]> = Vec::new();
        for (hash, account) in zip(raw_hashes, accounts) {
            if let Some(hash) = hash {
                hashes.push(hash)
            } else {
                new_frontiers
                    .new
                    .push(FrontierInfo::new_unopened(account.clone()))
            }
        }
        let hashes_to_download = frontiers_db.filter_known_hashes(&hashes);

        let frontiers = if hashes_to_download.is_empty() {
            vec![]
        } else {
            let (frontiers, failures_2) = self
                .internal()
                .blocks_info(config, &hashes_to_download)
                .await?
                .into();
            failures.merge_with(failures_2);
            frontiers.into_iter().flatten().collect()
        };

        new_frontiers.merge_with(frontiers.into());
        frontiers_db.check_new(&new_frontiers)?;

        Ok(RpcSuccess {
            item: new_frontiers,
            failures,
        })
    }

    /// Get work for a block, and publish it to the network.
    /// Does **not** cache work for the next block.
    ///
    /// This is intended to be used internally, where we cannot rely on the DB being synced.
    pub(crate) async fn get_work_and_publish_unsynced(
        &self,
        config: &CoreClientConfig,
        frontier: &FrontierInfo,
        mut block: Block,
    ) -> RpcResult<FrontierInfo> {
        let mut failures = RpcFailures::default();

        let (work, failures_work) = self.get_work(config, frontier).await?.into();
        block.work = work;
        failures.merge_with(failures_work);

        let (info, failures_publish) = self.publish(config, block).await?.into();
        failures.merge_with(failures_publish);

        Ok((info, failures).into())
    }

    /// Get work for a block, and publish it to the network.
    /// Does **not** cache work for the next block.
    pub async fn get_work_and_publish(
        &self,
        config: &CoreClientConfig,
        frontiers_db: &FrontiersDB,
        block: Block,
    ) -> RpcResult<FrontierInfo> {
        let frontier = frontiers_db
            .account_frontier(&block.account)
            .ok_or(CoreClientError::AccountNotFound)?;
        self.get_work_and_publish_unsynced(config, frontier, block)
            .await
    }

    /// Get work for a block, and publish it to the network.
    /// Also cache work for the next block, if enabled.
    ///
    /// This is intended to be used internally, where we cannot rely on the DB being synced.
    pub(crate) async fn auto_publish_unsynced(
        &self,
        config: &CoreClientConfig,
        frontier: &FrontierInfo,
        block: Block,
    ) -> RpcResult<FrontierInfo> {
        if !config.ENABLE_WORK_CACHE {
            return self
                .get_work_and_publish_unsynced(config, frontier, block)
                .await;
        }

        let mut failures = RpcFailures::default();

        let (cache_work_success, publish_success) = future::try_join(
            // get work for next transaction to cache
            self.internal().work_generate(config, block.hash(), None),
            // publish this block
            self.get_work_and_publish_unsynced(config, frontier, block),
        )
        .await?;

        let (mut info, failures_publish) = publish_success.into();
        let (cached_work, failures_cache_work) = cache_work_success.into();
        failures.merge_with(failures_publish);
        failures.merge_with(failures_cache_work);

        info.set_work(config, cached_work);
        Ok((info, failures).into())
    }

    /// Get work for a block, and publish it to the network.
    /// Also cache work for the next block, if enabled.
    pub async fn auto_publish(
        &self,
        config: &CoreClientConfig,
        frontiers_db: &FrontiersDB,
        block: Block,
    ) -> RpcResult<FrontierInfo> {
        let frontier = frontiers_db
            .account_frontier(&block.account)
            .ok_or(CoreClientError::AccountNotFound)?;
        self.auto_publish_unsynced(config, frontier, block).await
    }

    /// Handle the given RPC failures, adjusting future RPC selections as necessary
    pub fn handle_failures(&mut self, config: &mut CoreClientConfig, failures: RpcFailures) {
        self.internal().handle_failures(config, failures)
    }
}
