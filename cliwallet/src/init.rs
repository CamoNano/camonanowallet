use super::error::CliError;
use super::logging::{LevelFilter, Logger};
use super::storage::{
    config_location, delete_wallet, get_wallet_names, init_files, load_wallet, save_config,
    save_wallet, wallet_exists,
};
use super::CliClient;
use clap::{Args, Parser, Subcommand};
use client::{
    core::{nanopyrs, SecretBytes, WalletSeed},
    types::Hex32Bytes as Seed,
    ClientConfig, ClientError,
};
use nanopyrs::hashes::blake2b256;
use zeroize::Zeroize;

pub fn prompt_password() -> Result<SecretBytes<32>, ClientError> {
    let mut password = rpassword::prompt_password("Enter password: ")
        .map_err(|err| ClientError::FailedToReadPassword(err.to_string()))?;
    let key = blake2b256(password.as_bytes());
    password.zeroize();
    Ok(key)
}

pub fn prompt_confirmed_password() -> Result<SecretBytes<32>, CliError> {
    let mut key = prompt_password()?;
    println!("Please confirm your password.");
    let mut key_2 = prompt_password()?;
    while key != key_2 {
        println!("Passwords do not match! Please retry.");
        key = prompt_password()?;
        println!("Please confirm password.");
        key_2 = prompt_password()?;
    }
    Ok(key)
}

#[derive(Parser, Debug)]
#[command(version)]
pub struct Init {
    /// Command to execute
    #[command(subcommand)]
    command: InitType,
    /// Levels: 'off', 'error', 'warn', 'info', 'debug', 'trace'
    #[arg(long, default_value_t = LevelFilter::Info)]
    log: LevelFilter,
}
impl Init {
    pub fn execute(self) -> Result<(Option<CliClient>, Logger), CliError> {
        let client = match self.command {
            InitType::New(args) => args.execute(),
            InitType::Import(args) => args.execute(),
            InitType::Load(args) => args.execute(),
            InitType::Delete(args) => args.execute(),
            InitType::List(args) => args.execute(),
            InitType::Config(args) => args.execute(),
        }?;

        // load files to ensure they've been created
        init_files()?;

        Ok((client, self.log.into()))
    }
}

#[derive(Debug, Clone, Subcommand)]
enum InitType {
    /// Create a new wallet
    New(NewArgs),
    /// Import a seed as a new wallet
    Import(ImportArgs),
    /// Load a wallet from file
    Load(LoadArgs),
    /// Delete a wallet file
    Delete(DeleteArgs),
    /// List all wallet files
    List(ListArgs),
    /// Show the location of the configuration file
    Config(ConfigArgs),
}

#[derive(Debug, Clone, Args)]
struct NewArgs {
    /// Name of the wallet that will be created
    name: String,
}
impl NewArgs {
    fn execute(self) -> Result<Option<CliClient>, CliError> {
        if wallet_exists(&self.name)? {
            return Err(CliError::WalletAlreadyExists);
        }

        let key = prompt_confirmed_password()?;

        let seed = WalletSeed::from(rand::random::<[u8; 32]>());
        println!("seed: {}", seed.as_hex());

        let cli_client = CliClient::new(seed, self.name, key)?;
        save_wallet(&cli_client, &cli_client.name, &cli_client.key)?;
        Ok(Some(cli_client))
    }
}

#[derive(Debug, Clone, Args)]
struct ImportArgs {
    /// Name of the wallet that will be created
    name: String,
    /// The 64-character hexadecimal seed to be imported
    seed: Seed,
}
impl ImportArgs {
    fn execute(self) -> Result<Option<CliClient>, CliError> {
        if wallet_exists(&self.name)? {
            return Err(CliError::WalletAlreadyExists);
        }

        let key = prompt_confirmed_password()?;
        let seed = WalletSeed::from(self.seed.0);

        let cli_client = CliClient::new(seed, self.name, key)?;
        save_wallet(&cli_client, &cli_client.name, &cli_client.key)?;
        Ok(Some(cli_client))
    }
}

#[derive(Debug, Clone, Args)]
struct LoadArgs {
    /// Name of the wallet
    name: String,
}
impl LoadArgs {
    fn execute(self) -> Result<Option<CliClient>, CliError> {
        if !wallet_exists(&self.name)? {
            return Err(CliError::WalletNotFound);
        }

        let client = load_wallet(&self.name, prompt_password()?)?;
        Ok(Some(client))
    }
}

#[derive(Debug, Clone, Args)]
struct DeleteArgs {
    /// Name of the wallet
    name: String,
}
impl DeleteArgs {
    fn execute(self) -> Result<Option<CliClient>, CliError> {
        if !wallet_exists(&self.name)? {
            return Err(CliError::WalletAlreadyExists);
        }

        delete_wallet(&self.name, &prompt_password()?)?;
        Ok(None)
    }
}

#[derive(Debug, Clone, Args)]
struct ListArgs {}
impl ListArgs {
    fn execute(self) -> Result<Option<CliClient>, CliError> {
        let wallets = get_wallet_names()?;
        if wallets.is_empty() {
            println!("No wallets have been created yet")
        }
        for wallet in wallets {
            println!("{wallet}")
        }
        Ok(None)
    }
}

#[derive(Debug, Clone, Args)]
struct ConfigArgs {
    /// Reset the configuration file
    #[arg(short, long, default_value_t = false)]
    reset: bool,
}
impl ConfigArgs {
    fn execute(self) -> Result<Option<CliClient>, CliError> {
        if self.reset {
            save_config(ClientConfig::default())?
        }

        println!("Path: {}", config_location()?);
        Ok(None)
    }
}
