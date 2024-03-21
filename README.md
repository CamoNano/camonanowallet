## Camo Nano Wallet

A command-line, privacy-oriented wallet for the [Nano](https://nano.org/) cryptocurrency.

This wallet is targeted towards more advanced users, and is intended to act as a reference implementation for Camo Nano. See below for more details. The wallet also can be quite slow at times, though this is due to having a primitive work-getting system. The Camo Nano protocol is not the limiting factor.

## Installation

This wallet can be downloaded from the [releases page](https://github.com/expiredhotdog/camonanowallet/releases), or compiled from source.

Antivirus software may raise a false alarm when trying to use this wallet.

### Compiling from Source

Ensure that [Rust](https://www.rust-lang.org/tools/install), [git](https://github.com/git-guides/install-git) and some other dependencies are installed on your system before proceeding.
```
sudo apt install cargo git libssl-dev pkg-config
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Clone the repository:
```
git clone https://github.com/expiredhotdog/camonanowallet.git
cd camonanowallet
```

Compile using Cargo:
```
cargo build --release
```

Run:
```
./target/release/camonano
```

## Usage

For more details and options, use the `help` command:

```
camonano help
```
...or
```
> help
```
...or
```
> help <COMMAND>
```

### Opening a wallet

Create a new wallet:
```
camonano new <NAME>
```

... or load an existing one:
```
camonano load <NAME>
```

You will be prompted for a password when creating/loading a wallet.

### Basic Wallet Usage

Generate a new normal or camo account:
```
> account <INDEX>
> account <INDEX> -c
```

Refresh the wallet:
```
> refresh
```

List any receivable transactions:
```
> receive
```

Send coins to a normal or camo account:
```
> send <FROM> <AMOUNT> <TO>
> send_camo <FROM> <AMOUNT> <TO> -a
```

Display a detailed breakdown of the wallet's balance:
```
> balance
```

Exit the program:
```
> quit
```

The wallet is saved automatically after each command finishes.

### Configuration

Run `camonano config` to display the path to the configuration file. The RPCs and representatives that the software will use, among other things, are located there.

## Camo Accounts

This wallet supports "camo" accounts, which are a privacy tool for Nano, inspired by Monero.

Like most cryptocurrencies, Nano offers little in the way of privacy. With a normal Nano account, everyone can see its entire transaction history, including its current and past balances, who it has received coins from, and who it has sent coins to.

Camo accounts do not have a publically visible transaction history, allowing users to make private payments to each other. When using a camo account, no one except for you can know how many coins you've received, or from whom.

Note that camo accounts are a **custom, experimental, and non-standard feature** of this wallet, and are generally not supported by other wallets or the wider Nano ecosystem. If you would like to see this feature implemented in other wallets, then try to encourage other developers to support them. See [here](https://crates.io/crates/nanopyrs) for a Rust Nano library which supports camo accounts, and [here](https://github.com/expiredhotdog/nanopyrs/blob/main/CAMO-PROTOCOL.md) for documentation of the Camo Nano protocol.

## Security

Wallet files are encrypted using AES-256. The software is designed to wipe the wallet's sensitive data from memory after usage.

This software has *not* been professionally audited. I cannot guarantee that this software is perfect. Use at your own risk.

## Licensing

This software is open source and licensed under the MIT license. See the `LICENSE` file for more details.
