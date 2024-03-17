use super::{get_current_time, wrapped::Rpc, RpcFailure, RpcFailures, RpcResult, RpcSuccess};
use crate::config::ClientConfig;
use crate::error::ClientError;
use log::{trace, warn};
use nanopyrs::rpc::{AccountInfo, BlockInfo, Receivable};
use nanopyrs::{Account, Block};
use rand::prelude::{thread_rng, SliceRandom};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

macro_rules! wrap_rpc_methods {
    ( $($func:ident(&self, config: &ClientConfig, $($arg:ident: $type:ty),*) -> $return: ty)* ) => {
        $(
            #[doc = concat!("See `nanopyrs::rpc::Rpc::", stringify!($func), "()` for documentation")]
            pub async fn $func(&self, config: &ClientConfig, $($arg: $type),*) -> $return {
                let command = stringify!($func);
                for _ in 0..config.RPC_RETRY_LIMIT {
                    let mut failures = vec!();
                    for w_rpc in self.get_usable_rpcs(config, command)? {
                        let response = w_rpc.rpc.$func($($arg),*).await;
                        let url = w_rpc.get_url();

                        trace!("RPC request ({}) to {}: {:?}", command, url, response.raw_request);
                        trace!("RPC response ({}) from {}: {:?}", command, url, response.raw_response);

                        if let Err(err) = &response.result {
                            trace!("Error ({command}) from {url}: {err}");
                        }
                        // successful request (break)
                        if let Ok(item) = response.result {
                            trace!("Success ({command}) from {url}");
                            return Ok(RpcSuccess{
                                item,
                                failures: RpcFailures(failures)
                            })
                        }
                        // unsuccessful request (continue)
                        failures.push(RpcFailure{
                            err: response.result.unwrap_err(),
                            url: w_rpc.get_url().to_string()
                        });
                    }
                    warn!("Failed to execute RPC command '{command}'. Trying again...")
                }
                // unsuccessful request (all RPC's failed)
                Err(ClientError::RpcCommandFailed)
            }
        )*
    };
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcManager();
impl RpcManager {
    pub fn get_usable_rpcs(
        &self,
        config: &ClientConfig,
        command: &str,
    ) -> Result<Vec<Rpc>, ClientError> {
        let current_time = get_current_time();

        let mut rpcs = config.RPCS.clone();
        rpcs.shuffle(&mut thread_rng());
        rpcs.sort_by_key(|rpc| {
            if rpc.is_banned(current_time) {
                rpc.banned_until
            } else {
                0
            }
        });

        let rpcs = rpcs
            .into_iter()
            .filter(|rpc| rpc.commands.supports(command));
        let rpcs = match config.RPC_USE_BANNED_NODES_AS_BACKUP {
            true => rpcs.collect(),
            false => rpcs.filter(|rpc| !rpc.is_banned(current_time)).collect(),
        };

        Ok(rpcs)
    }

    pub fn handle_failures(&self, config: &mut ClientConfig, failures: RpcFailures) {
        let _config = config.clone();
        for failure in failures.0 {
            config
                .RPCS
                .iter_mut()
                .find(|w_rpc| w_rpc.get_url() == failure.url)
                .expect("broken RpcManager code: unknown RPC URL")
                .handle_err(&_config, &failure.err);
        }
    }

    wrap_rpc_methods!(
        account_balance(&self, config: &ClientConfig, account: &Account) -> RpcResult<u128>
        account_history(&self, config: &ClientConfig, account: &Account, count: usize, head: Option<[u8; 32]>, offset: Option<usize>) -> RpcResult<Vec<Block>>
        account_info(&self, config: &ClientConfig, account: &Account) -> RpcResult<Option<AccountInfo>>
        account_representative(&self, config: &ClientConfig, account: &Account) -> RpcResult<Option<Account>>
        accounts_balances(&self, config: &ClientConfig, accounts: &[Account]) -> RpcResult<Vec<u128>>
        accounts_frontiers(&self, config: &ClientConfig, accounts: &[Account]) -> RpcResult<Vec<Option<[u8; 32]>>>
        accounts_receivable(&self, config: &ClientConfig, accounts: &[Account], count: usize, threshold: u128) -> RpcResult<Vec<Vec<Receivable>>>
        accounts_representatives(&self, config: &ClientConfig, accounts: &[Account]) -> RpcResult<Vec<Option<Account>>>
        block_info(&self, config: &ClientConfig, hash: [u8; 32]) -> RpcResult<Option<BlockInfo>>
        blocks_info(&self, config: &ClientConfig, hashes: &[[u8; 32]]) -> RpcResult<Vec<Option<BlockInfo>>>
        process(&self, config: &ClientConfig, block: &Block) -> RpcResult<[u8; 32]>
        work_generate(&self, config: &ClientConfig, hash: [u8; 32], custom_difficulty: Option<[u8; 8]>) -> RpcResult<[u8; 8]>
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ClientConfig;
    use crate::rpc::{get_current_time, Rpc, RpcCommands};
    use nanopyrs::rpc::RpcError;

    fn fake_rpc(url: &str) -> Rpc {
        let commands = RpcCommands {
            account_balance: true,
            account_history: true,
            account_info: true,
            account_representative: true,
            accounts_balances: true,
            accounts_frontiers: true,
            accounts_receivable: true,
            accounts_representatives: true,
            block_info: true,
            blocks_info: true,
            process: true,
            work_generate: true,
        };
        Rpc::new(commands, url, None).unwrap()
    }

    fn fake_failures(url: &str) -> RpcFailures {
        RpcFailures(vec![RpcFailure {
            err: RpcError::InvalidData,
            url: url.into(),
        }])
    }

    #[test]
    fn ban() {
        let mut rpc_1 = fake_rpc("https://example.com");
        let mut rpc_2 = fake_rpc("https://example2.com");

        assert!(!rpc_1.is_banned(get_current_time()));
        rpc_1.ban_for_seconds(1000);
        assert!(rpc_1.is_banned(get_current_time()));

        assert!(!rpc_2.is_banned(get_current_time()));
        rpc_2.handle_err(&ClientConfig::test_default(), &RpcError::InvalidData);
        assert!(rpc_2.is_banned(get_current_time()));
    }

    #[test]
    fn get_usable_rpcs_banned() {
        let mut config = ClientConfig::test_default();
        config.RPC_USE_BANNED_NODES_AS_BACKUP = true;

        let rpc_1 = fake_rpc("https://example3.com");
        let rpc_2 = fake_rpc("https://example4.com");
        config.RPCS = vec![rpc_1, rpc_2];
        let rpcs = RpcManager();

        // neither are banned
        let usable = rpcs.get_usable_rpcs(&config, "accounts_frontiers").unwrap();
        // (order is random)
        assert!(usable.len() == 2);

        // one is banned
        let failure = fake_failures("https://example3.com");
        rpcs.handle_failures(&mut config, failure);
        let usable = rpcs.get_usable_rpcs(&config, "accounts_frontiers").unwrap();
        assert!(usable.len() == 2);
        assert!(usable[0].get_url() == "https://example4.com");
        assert!(usable[1].get_url() == "https://example3.com");

        // both are banned (use banned as backup)
        let failure = fake_failures("https://example4.com");
        rpcs.handle_failures(&mut config, failure);
        config.RPCS[1].banned_until += 100;
        let usable = rpcs.get_usable_rpcs(&config, "accounts_frontiers").unwrap();
        assert!(usable.len() == 2);
        assert!(usable[0].get_url() == "https://example3.com");
        assert!(usable[1].get_url() == "https://example4.com");

        // both are banned (don't use banned as backup)
        config.RPC_USE_BANNED_NODES_AS_BACKUP = false;
        let usable = rpcs.get_usable_rpcs(&config, "accounts_frontiers").unwrap();
        assert!(usable.is_empty());
    }

    #[test]
    fn get_usable_rpcs_commands() {
        let mut config = ClientConfig::test_default();
        config.RPC_USE_BANNED_NODES_AS_BACKUP = true;

        let mut rpc_1 = fake_rpc("https://example5.com");
        rpc_1.commands.account_balance = false;
        rpc_1.commands.account_info = false;
        let mut rpc_2 = fake_rpc("https://example6.com");
        rpc_2.commands.account_history = false;
        rpc_2.commands.account_info = false;
        config.RPCS = vec![rpc_1, rpc_2];
        let rpcs = RpcManager();

        let usable = rpcs.get_usable_rpcs(&config, "accounts_frontiers").unwrap();
        assert!(usable.len() == 2);

        let usable = rpcs.get_usable_rpcs(&config, "account_balance").unwrap();
        assert!(usable.len() == 1);
        assert!(usable[0].get_url() == "https://example6.com");

        let usable = rpcs.get_usable_rpcs(&config, "account_history").unwrap();
        assert!(usable.len() == 1);
        assert!(usable[0].get_url() == "https://example5.com");

        let usable = rpcs.get_usable_rpcs(&config, "account_info").unwrap();
        assert!(usable.is_empty());
    }
}
