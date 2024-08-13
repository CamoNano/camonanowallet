use super::RpcManager;
use crate::config::CoreClientConfig;
use crate::error::CoreClientError;
use crate::frontiers::{FrontierInfo, FrontiersDB, NewFrontiers};
use crate::rpc::{workserver::WorkClient, RpcFailures, RpcResult, RpcSuccess};
use log::warn;
use nanopyrs::{Account, Block};
use std::iter::zip;

#[derive(Debug)]
pub struct ClientRpc();
impl ClientRpc {
    /// Get cached work for this frontier, either cached locally or from an RPC
    pub async fn get_work(
        &self,
        config: &CoreClientConfig,
        work_client: &mut WorkClient,
        frontier: &FrontierInfo,
    ) -> RpcResult<[u8; 8]> {
        if let Some(work) = frontier.cached_work() {
            return Ok((work, RpcFailures::default()).into());
        }

        if let Some(work) = work_client.get_result(frontier.cache_work_hash()) {
            return work.rpc_result;
        }

        let work_request = work_client.request_work(config, frontier.cache_work_hash());
        if work_request.is_ok() {
            work_client
                .wait_on(frontier.cache_work_hash())
                .await
                .rpc_result
        } else {
            // Contingency plan
            warn!("Lost connection to WorkServer, using RpcManager for work generation");
            RpcManager()
                .work_generate(config, frontier.cache_work_hash(), None)
                .await
        }
    }

    /// Publish a block to the network
    pub async fn publish(
        &self,
        config: &CoreClientConfig,
        block: Block,
    ) -> RpcResult<FrontierInfo> {
        let (_, failures) = RpcManager().process(config, &block).await?.into();
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

        let (raw_hashes, mut failures) = RpcManager()
            .accounts_frontiers(config, accounts)
            .await?
            .into();

        let mut hashes: Vec<[u8; 32]> = Vec::new();
        for (hash, account) in zip(raw_hashes, accounts) {
            if let Some(hash) = hash {
                hashes.push(hash)
            } else {
                let new = FrontierInfo::new_unopened(account.clone());
                let existing_block = frontiers_db
                    .account_frontier(account)
                    .map(|frontier| &frontier.block);

                if existing_block != Some(&new.block) {
                    new_frontiers.new.push(new)
                }
            }
        }
        let hashes_to_download = frontiers_db.filter_known_hashes(&hashes);

        let frontiers = if hashes_to_download.is_empty() {
            vec![]
        } else {
            let (frontiers, failures_2) = RpcManager()
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
    /// **Does not** cache work for the next block.
    ///
    /// This is intended to be used internally, where we cannot rely on the DB being synced.
    pub(crate) async fn get_work_and_publish_unsynced(
        &self,
        config: &CoreClientConfig,
        work_client: &mut WorkClient,
        frontier: &FrontierInfo,
        mut block: Block,
    ) -> RpcResult<FrontierInfo> {
        let mut failures = RpcFailures::default();

        let (work, failures_work) = self.get_work(config, work_client, frontier).await?.into();
        block.work = work;
        failures.merge_with(failures_work);

        let (info, failures_publish) = self.publish(config, block).await?.into();
        failures.merge_with(failures_publish);

        Ok((info, failures).into())
    }

    /// Get work for a block, and publish it to the network.
    /// **Does not** cache work for the next block.
    pub async fn get_work_and_publish(
        &self,
        config: &CoreClientConfig,
        work_client: &mut WorkClient,
        frontiers_db: &FrontiersDB,
        block: Block,
    ) -> RpcResult<FrontierInfo> {
        let frontier = frontiers_db
            .account_frontier(&block.account)
            .ok_or(CoreClientError::AccountNotFound)?;
        self.get_work_and_publish_unsynced(config, work_client, frontier, block)
            .await
    }

    /// Get work for a block, and publish it to the network.
    /// **Does** cache work for the next block, if enabled.
    ///
    /// This is intended to be used internally, where we cannot rely on the DB being synced.
    pub(crate) async fn auto_publish_unsynced(
        &self,
        config: &CoreClientConfig,
        work_client: &mut WorkClient,
        frontier: &FrontierInfo,
        block: Block,
    ) -> RpcResult<FrontierInfo> {
        let block_hash = block.hash();
        let result = self
            .get_work_and_publish_unsynced(config, work_client, frontier, block)
            .await;

        // Cache work for next block
        if config.ENABLE_WORK_CACHE {
            work_client.request_work(config, block_hash)?;
        }
        result
    }

    /// Get work for a block, and publish it to the network.
    /// **Does** cache work for the next block, if enabled.
    pub async fn auto_publish(
        &self,
        config: &CoreClientConfig,
        work_client: &mut WorkClient,
        frontiers_db: &FrontiersDB,
        block: Block,
    ) -> RpcResult<FrontierInfo> {
        let frontier = frontiers_db
            .account_frontier(&block.account)
            .ok_or(CoreClientError::AccountNotFound)?;
        self.auto_publish_unsynced(config, work_client, frontier, block)
            .await
    }

    /// Handle the given RPC failures, adjusting future RPC selections as necessary
    pub fn handle_failures(&mut self, config: &mut CoreClientConfig, failures: RpcFailures) {
        RpcManager().handle_failures(config, failures)
    }
}
