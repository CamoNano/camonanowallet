use crate::constants::*;
use crate::rpc::Rpc;
use nanopyrs::{camo::CamoVersion, Account};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use zeroize::{Zeroize, ZeroizeOnDrop};

fn default_true() -> bool {
    true
}

#[allow(non_snake_case)]
#[serde_as]
#[derive(Debug, Clone, Zeroize, ZeroizeOnDrop, Serialize, Deserialize)]
pub struct CoreClientConfig {
    /// Amount below which normal transactions will be ignored.
    /// Does not apply to camo payments.
    pub NORMAL_DUST_THRESHOLD: u128,

    /// Limit on the number of normal and camo accounts in the database.
    /// The limit is separate for each account type.
    ///
    /// Does not apply to derived accounts.
    pub DB_NUMBER_OF_ACCOUNTS_LIMIT: usize,

    /// Amount of time, in seconds, for which RPCs will be banned for sending invalid data
    pub RPC_INVALID_DATA_BAN_TIME: u64,
    /// Amount of time, in seconds, for which RPCs will be banned for miscellaneous issues
    pub RPC_FAILURE_BAN_TIME: u64,
    /// Whether or not to use banned RPCs if no unbanned ones are available
    pub RPC_USE_BANNED_NODES_AS_BACKUP: bool,
    /// Number of times to re-attempt a failed RPC command
    pub RPC_RETRY_LIMIT: usize,
    /// Default work difficulty
    pub WORK_DIFFICULTY: u64,

    /// `count` field of `accounts_receivable`
    pub RPC_ACCOUNTS_RECEIVABLE_BATCH_SIZE: usize,
    /// `count` field of `account_history`
    pub RPC_ACCOUNT_HISTORY_BATCH_SIZE: usize,
    /// transactions will be received in batches of this size
    pub RPC_RECEIVE_TRANSACTIONS_BATCH_SIZE: usize,
    /// Enable setting work cache (added in v0.1.1)
    #[serde(default = "default_true")]
    pub ENABLE_WORK_CACHE: bool,

    /// Default version to use for generating `camo_` addresses
    pub DEFAULT_CAMO_VERSIONS: Vec<CamoVersion>,

    /// Representatives for connecting to the Nano network
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub REPRESENTATIVES: Vec<Account>,
    /// RPCs to use for connecting to the Nano network
    pub RPCS: Vec<Rpc>,
}
impl CoreClientConfig {
    pub fn default_with(reps: Vec<Account>, rpcs: Vec<Rpc>) -> Self {
        if reps.is_empty() {
            panic!("no representatives to choose from")
        }

        CoreClientConfig {
            NORMAL_DUST_THRESHOLD: ONE_MICRO_NANO,

            DB_NUMBER_OF_ACCOUNTS_LIMIT: 20,

            RPC_INVALID_DATA_BAN_TIME: ONE_HOUR * 12,
            RPC_FAILURE_BAN_TIME: ONE_MINUTE * 15,
            RPC_USE_BANNED_NODES_AS_BACKUP: true,
            RPC_RETRY_LIMIT: 8,
            WORK_DIFFICULTY: 0xfffffff800000000,

            RPC_ACCOUNTS_RECEIVABLE_BATCH_SIZE: 25,
            RPC_ACCOUNT_HISTORY_BATCH_SIZE: 50,
            RPC_RECEIVE_TRANSACTIONS_BATCH_SIZE: 3,
            ENABLE_WORK_CACHE: true,

            DEFAULT_CAMO_VERSIONS: vec![CamoVersion::One],

            REPRESENTATIVES: reps,
            RPCS: rpcs,
        }
    }

    #[cfg(test)]
    pub(crate) fn test_default() -> CoreClientConfig {
        let mut config = CoreClientConfig::default_with(
            vec![nanopyrs::constants::get_genesis_account()],
            vec![],
        );
        config.WORK_DIFFICULTY = 0;
        config
    }
}
