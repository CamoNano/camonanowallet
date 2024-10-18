#![warn(unused_crate_dependencies, unsafe_code)]

mod client;
mod config;
mod error;

pub mod constants;
pub mod frontiers;
pub mod rpc;
pub mod wallet;
pub mod work;

pub use client::{CamoPayment, CoreClient, Payment, RescanData};
pub use config::CoreClientConfig;
pub use error::CoreClientError;
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

#[cfg(not(target_arch = "wasm32"))]
mod time {
    pub use std::time::*;
}

#[cfg(target_arch = "wasm32")]
mod time {
    pub use web_time::*;
}