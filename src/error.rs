use super::storage::StorageError;
use clap::Error as ClapError;
use client::nanopyrs::NanoError;
use client::ClientError;
use confy::ConfyError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    NanoError(#[from] NanoError),
    #[error(transparent)]
    ClapError(#[from] ClapError),
    #[error(transparent)]
    ClientError(#[from] ClientError),
    #[error("invalid arguments")]
    InvalidArguments,
    #[error("invalid amount")]
    AmountBelowDustThreshold,
    #[error(transparent)]
    StorageError(#[from] StorageError),
    #[error("failed to read password: {0}")]
    FailedToReadPassword(String),
}
impl From<ConfyError> for CliError {
    fn from(value: ConfyError) -> Self {
        let storage_err: StorageError = value.into();
        storage_err.into()
    }
}
