use client::{core::CoreClientError, ClientError};
use confy::ConfyError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    ClientError(#[from] ClientError),
    #[error(transparent)]
    DiskError(#[from] ConfyError),
    #[error("The given wallet name is invalid")]
    InvalidWalletName,
    #[error("No wallet of the given name could be found")]
    WalletNotFound,
    #[error("A wallet of the same name already exists")]
    WalletAlreadyExists,
}
impl From<CoreClientError> for CliError {
    fn from(value: CoreClientError) -> Self {
        CliError::ClientError(value.into())
    }
}
