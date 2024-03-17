#![warn(unused_crate_dependencies, unsafe_code)]

mod client;
mod config;
mod error;

pub mod constants;
pub mod frontiers;
pub mod rpc;
pub mod wallet;

pub use client::{CamoPayment, Client, Payment, RescanData};
pub use config::ClientConfig;
pub use error::ClientError;
pub use nanopyrs::{
    self,
    camo::{
        CamoAccount, CamoKeys, CamoVersion, CamoVersions, CamoViewKeys, Notification,
        NotificationV1,
    },
    rpc::Receivable,
    Account, Block, BlockType, Key, Scalar, SecretBytes, Signature,
};
pub use wallet::WalletSeed;
