use nanopyrs::{rpc::RpcError, NanoError};
use thiserror::Error;
use tokio::task::JoinError;

#[derive(Debug, Error)]
pub enum CoreClientError {
    #[error(transparent)]
    NanoError(#[from] NanoError),
    #[error(transparent)]
    RpcError(#[from] RpcError),
    #[error(transparent)]
    JoinError(#[from] JoinError),
    #[error("the given RPC command could not be performed on any known node")]
    RpcCommandFailed,
    #[error("no usable RPC could be found")]
    NoUsableRPCs,
    #[error("invalid seed")]
    InvalidSeed,
    #[error("account not found")]
    AccountNotFound,
    #[error("the number of accounts in the DB has reached the limit")]
    DBAccountLimitReached,
    #[error("not enough coins")]
    NotEnoughCoins,
    #[error("amount below dust threshold")]
    BelowDustThreshold,
    #[error("the blocks database detected a balance overflow")]
    FrontierBalanceOverflow,
    #[error("the blocks database detected an invalid epoch block")]
    InvalidEpochBlock,
}
