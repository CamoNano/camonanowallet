# CLI Wallet

A command-line Camo Nano wallet. This wallet is targeted towards more advanced users.

## Installation

This wallet can be downloaded from the [releases page](https://github.com/CamoNano/camonanowallet/releases), or compiled from source.

Antivirus software may raise a false alarm when trying to use this wallet.

### Compiling from Source

#### Prerequisites

Ensure that [Rust](https://www.rust-lang.org/tools/install) and [git](https://github.com/git-guides/install-git) are installed on your system before proceeding.

For Debian-based systems, you may have to install some additional dependencies:
```
sudo apt install libssl-dev pkg-config
```

#### Compiling

Clone the repository:
```
git clone https://github.com/CamoNano/camonanowallet.git
cd camonanowallet/cli_client
```

Compile using Cargo:
```
cargo build --release
```

Run:
```
./target/release/camonano
```

If you experience an error while attempting to compile from source, please open an issue so that it can be resolved.

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

Generate a new normal or Camo account:
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

Send coins to a normal or Camo account:
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