use bincode::Error as BincodeError;
use client::{core::CoreClientError, ClientError};
use hex::FromHexError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    ClientError(#[from] ClientError),
    #[error("Wallet not initialized")]
    WalletNotInitialized,
    #[error("Failed to access web_sys::Window")]
    WindowUnavailable,
    #[error("Failed to send alert to web_sys::Window (reason: {0})")]
    AlertError(String),
    #[error("Failed to get web_sys::Storage (reason: {0})")]
    StorageUnavailable(String),
    #[error("Failed to set item in web_sys::Storage (reason: {0})")]
    StorageSetError(String),
    #[error("Failed to get item in web_sys::Storage (reason: {0})")]
    StorageGetError(String),
    #[error("Failed to remove item in web_sys::Storage (reason: {0})")]
    StorageRemoveError(String),
    #[error("The wallet was saved improperly, and is corrupted")]
    StorageMismatch,
}
impl From<CoreClientError> for AppError {
    fn from(value: CoreClientError) -> Self {
        AppError::ClientError(value.into())
    }
}
impl From<FromHexError> for AppError {
    fn from(value: FromHexError) -> Self {
        AppError::ClientError(value.into())
    }
}
impl From<BincodeError> for AppError {
    fn from(value: BincodeError) -> Self {
        AppError::ClientError(value.into())
    }
}
