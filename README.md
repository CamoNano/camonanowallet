# Camo Nano Wallet

A privacy-oriented wallet for the [Nano](https://nano.org/) cryptocurrency.
There is currently only a CLI version of this wallet.

This wallet is intended to act as a reference implementation for Camo Nano. See below for more details.

### CLI Wallet

See [`./cliwallet`](./cliwallet/) for the CLI version of this wallet.

## Camo Nano

This wallet supports "Camo" accounts, which are a privacy tool for Nano, inspired by Monero.

Like most cryptocurrencies, Nano offers little in the way of privacy. With a normal Nano account, everyone can see its entire transaction history, including its current and past balances, who it has received coins from, and who it has sent coins to.

Camo accounts do not have a publically visible transaction history, allowing users to make private payments to each other. When using a Camo account, no one except for you can know how many coins you've received, or from whom.

Note that Camo Nano is a **custom, experimental, and non-standard protocol**, and is generally not supported by other wallets or the wider Nano ecosystem. If you would like to see this feature implemented in other wallets, encourage other developers to support them. See [here](https://crates.io/crates/nanopyrs) for a Rust Nano library which supports Camo Nano, and [here](https://github.com/CamoNano/nanopyrs/blob/main/CAMO-PROTOCOL.md) for documentation of the Camo Nano protocol.

## Security

Wallet data is encrypted using AES-256. The software is designed to wipe the wallet's sensitive data from memory after usage.

This software has *not* been professionally audited. I cannot guarantee that this software is perfect. Use at your own risk.

## Licensing

All software in this repository is open source and licensed under the MIT license. See the `LICENSE` file for more details.