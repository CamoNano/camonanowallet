use crate::config::ClientConfig;
use crate::error::ClientError;
use crate::rpc::get_ban_expiration;
use log::debug;
use nanopyrs::rpc::{debug::DebugRpc, RpcError};
use serde::{Deserialize, Serialize};
use std::cmp::max;
use std::fmt::Debug;
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Debug, Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop, Serialize, Deserialize)]
pub struct RpcCommands {
    pub account_balance: bool,
    pub account_history: bool,
    pub account_info: bool,
    pub account_representative: bool,
    pub accounts_balances: bool,
    pub accounts_frontiers: bool,
    pub accounts_receivable: bool,
    pub accounts_representatives: bool,
    pub block_info: bool,
    pub blocks_info: bool,
    pub process: bool,
    pub work_generate: bool,
}
impl RpcCommands {
    /// Will panic if given an invalid command
    pub fn supports(&self, command: &str) -> bool {
        match command {
            "account_balance" => self.account_balance,
            "account_history" => self.account_history,
            "account_info" => self.account_info,
            "account_representative" => self.account_representative,
            "accounts_balances" => self.accounts_balances,
            "accounts_frontiers" => self.accounts_frontiers,
            "accounts_receivable" => self.accounts_receivable,
            "accounts_representatives" => self.accounts_representatives,
            "block_info" => self.block_info,
            "blocks_info" => self.blocks_info,
            "process" => self.process,
            "work_generate" => self.work_generate,
            _ => panic!("broken RPC code: invalid RPC method: '{}'", command),
        }
    }
}

#[derive(Debug, Clone, Zeroize, ZeroizeOnDrop)]
pub struct Rpc {
    pub commands: RpcCommands,
    pub banned_until: u64,
    #[zeroize(skip)]
    pub rpc: DebugRpc,
}
impl Rpc {
    fn _new(
        commands: RpcCommands,
        url: &str,
        proxy: impl Into<Option<String>>,
        banned_until: u64,
    ) -> Result<Rpc, ClientError> {
        Ok(Rpc {
            commands,
            rpc: DebugRpc::new(url, proxy)?,
            banned_until,
        })
    }

    pub fn new(
        commands: RpcCommands,
        url: &str,
        proxy: impl Into<Option<String>>,
    ) -> Result<Rpc, ClientError> {
        Rpc::_new(commands, url, proxy, 0)
    }

    pub fn ban_for_seconds(&mut self, ban_time: u64) {
        self.banned_until = max(self.banned_until, get_ban_expiration(ban_time));
    }

    pub fn is_banned(&self, current_time: u64) -> bool {
        self.banned_until > current_time
    }

    pub fn get_url(&self) -> &str {
        self.rpc.get_url()
    }

    pub fn get_proxy(&self) -> Option<&str> {
        self.rpc.get_proxy()
    }

    pub fn get_rpc(&self) -> &DebugRpc {
        &self.rpc
    }

    pub(super) fn handle_err(&mut self, config: &ClientConfig, err: &RpcError) {
        let seconds = match err {
            RpcError::InvalidData => config.RPC_INVALID_DATA_BAN_TIME,
            _ => config.RPC_FAILURE_BAN_TIME,
        };
        debug!(
            "Banning {} for {} seconds: {}",
            self.get_url(),
            seconds,
            err
        );
        self.ban_for_seconds(seconds);
    }
}
impl Serialize for Rpc {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        WrappedRpcSerde {
            commands: self.commands.clone(),
            url: self.get_url().to_owned(),
            proxy: self.get_proxy().map(|proxy| proxy.to_owned()),
            banned_until: self.banned_until,
        }
        .serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for Rpc {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let rpc = WrappedRpcSerde::deserialize(deserializer)?;
        let rpc = Rpc::_new(rpc.commands, &rpc.url, rpc.proxy, rpc.banned_until);
        Ok(rpc.expect("could not deserialize WrappedRpcSerde"))
    }
}

#[derive(Serialize, Deserialize)]
struct WrappedRpcSerde {
    commands: RpcCommands,
    url: String,
    proxy: Option<String>,
    banned_until: u64,
}
