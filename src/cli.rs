use std::str::FromStr;

use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, keypair_from_seed};
use structopt::StructOpt;

use crate::error::Error;

#[derive(Debug, StructOpt)]
#[structopt(name = "solana-tss", about = "A PoC for managing a Solana TSS wallet.")]
pub enum Options {
    /// Generate a pair of keys.
    Generate,
    /// Check the balance of an address.
    Balance {
        /// The address to check the balance of
        address: Pubkey,
        /// Choose the desired netwrok: Mainnet/Testnet/Devnet
        #[structopt(default_value = "testnet")]
        net: Network,
    },
    /// Request an airdrop from a faucet.
    Airdrop {
        /// Address of the recipient
        to: Pubkey,
        /// The amount of SOL you want to send.
        amount: f64,
        /// Choose the desired netwrok: Mainnet/Testnet/Devnet
        #[structopt(default_value = "testnet")]
        net: Network,
    },
    /// Send a transaction using a single private key.
    SendSingle {
        /// A Base58 secret key
        #[structopt(parse(try_from_str = parse_keypair_bs58))]
        keypair: Keypair,
        /// The amount of SOL you want to send.
        amount: f64,
        /// Address of the recipient
        to: Pubkey,
        /// Choose the desired netwrok: Mainnet/Testnet/Devnet
        #[structopt(default_value = "testnet")]
        net: Network,
        /// Add a memo to the transaction
        memo: Option<String>,
    },
}

#[derive(Debug)]
pub enum Network {
    Mainnet,
    Testnet,
    Devnet,
}

impl Network {
    pub fn get_cluster_url(&self) -> &'static str {
        match self {
            Self::Mainnet => "https://api.mainnet-beta.solana.com",
            Self::Testnet => "https://api.testnet.solana.com",
            Self::Devnet => "https://api.devnet.solana.com",
        }
    }
}

impl FromStr for Network {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mainnet" | "Mainnet" => Ok(Self::Mainnet),
            "testnet" | "Testnet" => Ok(Self::Testnet),
            "devnet" | "Devnet" => Ok(Self::Devnet),
            _ => Err(Error::WrongNetwork(s.to_string())),
        }
    }
}

fn parse_keypair_bs58(s: &str) -> Result<Keypair, Error> {
    let decoded = bs58::decode(s).into_vec()?;
    Ok(keypair_from_seed(&decoded).map_err(Error::SignatureError)?)
}
