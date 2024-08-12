use core_client::nanopyrs::NanoError;
use core_client::CoreClientError;
use thiserror::Error;

use aes_gcm::Error as AESError;
use argon2::Error as Argon2Error;
use bincode::Error as BincodeError;
use hex::FromHexError;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error(transparent)]
    NanoError(#[from] NanoError),
    #[error(transparent)]
    CoreClientError(#[from] CoreClientError),
    #[error("Invalid arguments")]
    InvalidArguments,
    #[error("Invalid amount")]
    AmountBelowDustThreshold,
    #[error("Invalid hex value: {0}")]
    InvalidHex(#[from] FromHexError),
    #[error("Error while serializing/deserializing data: {0}")]
    SerializationError(#[from] BincodeError),
    #[error("Error while deriving encryption key from password: {0}")]
    Argon2Error(Argon2Error),
    #[error("Error while encrypting/decrypting data: {0}")]
    EncryptionError(AESError),
    #[error("Invalid password for wallet: {0}")]
    InvalidPassword(AESError),
    #[error("Failed to read password: {0}")]
    FailedToReadPassword(String),
}
impl From<Argon2Error> for ClientError {
    fn from(value: Argon2Error) -> Self {
        ClientError::Argon2Error(value)
    }
}
