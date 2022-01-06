use std::str::FromStr;

use solana_sdk::hash::Hash;
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use structopt::StructOpt;

use crate::error::Error;
use crate::serialization::{AggMessage1, PartialSignature, SecretAggStepOne, Serialize};

#[derive(Debug, StructOpt)]
#[structopt(name = "solana-tss", about = "A PoC for managing a Solana TSS wallet.")]
pub enum Options {
    /// Generate a pair of keys.
    #[structopt(display_order = 1)]
    Generate,
    /// Check the balance of an address.
    #[structopt(display_order = 2)]
    Balance {
        /// The address to check the balance of
        address: Pubkey,
        /// Choose the desired network: Mainnet/Testnet/Devnet
        #[structopt(default_value = "testnet", long)]
        net: Network,
    },
    /// Request an airdrop from a faucet.
    #[structopt(display_order = 3)]
    Airdrop {
        /// Address of the recipient
        #[structopt(long)]
        to: Pubkey,
        /// The amount of SOL you want to send.
        #[structopt(long)]
        amount: f64,
        /// Choose the desired network: Mainnet/Testnet/Devnet
        #[structopt(default_value = "testnet", long)]
        net: Network,
    },
    /// Send a transaction using a single private key.
    #[structopt(display_order = 4)]
    SendSingle {
        /// A Base58 secret key
        #[structopt(parse(try_from_str = parse_keypair_bs58), long)]
        keypair: Keypair,
        /// The amount of SOL you want to send.
        #[structopt(long)]
        amount: f64,
        /// Address of the recipient
        #[structopt(long)]
        to: Pubkey,
        /// Choose the desired network: Mainnet/Testnet/Devnet
        #[structopt(default_value = "testnet", long)]
        net: Network,
        /// Add a memo to the transaction
        #[structopt(long)]
        memo: Option<String>,
    },
    /// Print the hash of a recent block, can be used to pass to the `agg-send` steps
    #[structopt(display_order = 8)]
    RecentBlockHash {
        /// Choose the desired network: Mainnet/Testnet/Devnet
        #[structopt(default_value = "testnet", long)]
        net: Network,
    },
    /// Aggregate a list of addresses into a single address that they can all sign on together
    #[structopt(display_order = 5)]
    AggregateKeys {
        /// List of addresses
        #[structopt(min_values = 2, required = true)]
        keys: Vec<Pubkey>,
    },
    /// Start aggregate signing
    #[structopt(display_order = 6)]
    AggSendStepOne {
        /// A Base58 secret key of the party signing
        #[structopt(parse(try_from_str = parse_keypair_bs58))]
        keypair: Keypair,
    },
    /// Step 2 of aggregate signing, you should pass in the secret data from step 1.
    /// It's important that all parties pass in exactly the same transaction details (amount,to,net,memo,recent_block_hash)
    #[structopt(display_order = 9)]
    AggSendStepTwo {
        /// A Base58 secret key of the party signing
        #[structopt(parse(try_from_str = parse_keypair_bs58), long)]
        keypair: Keypair,
        /// The amount of SOL you want to send.
        #[structopt(long)]
        amount: f64,
        /// Address of the recipient
        #[structopt(long)]
        to: Pubkey,
        /// Add a memo to the transaction
        #[structopt(long)]
        memo: Option<String>,
        /// A hash of a recent block, can be obtained by calling `recent-block-hash`, all parties *must* pass in the same hash.
        #[structopt(long)]
        recent_block_hash: Hash,
        /// List of addresses that are part of this
        #[structopt(long, required = true, min_values = 2)]
        keys: Vec<Pubkey>,
        /// A list of all the first messages received in step 1
        #[structopt(long, required = true, min_values = 1, empty_values = false, parse(try_from_str = Serialize::deserialize_bs58))]
        first_messages: Vec<AggMessage1>,
        /// The secret state received in step 2.
        #[structopt(long, empty_values = false, parse(try_from_str = Serialize::deserialize_bs58))]
        secret_state: SecretAggStepOne,
    },
    /// Aggregate all the partial signatures together into a full signature, and send the transaction to Solana
    #[structopt(display_order = 10)]
    AggregateSignaturesAndBroadcast {
        // A list of all partial signatures produced in step three.
        #[structopt(long, required = true, min_values = 2, empty_values = false, parse(try_from_str = Serialize::deserialize_bs58))]
        signatures: Vec<PartialSignature>,
        /// The amount of SOL you want to send.
        #[structopt(long)]
        amount: f64,
        /// Address of the recipient
        #[structopt(long)]
        to: Pubkey,
        /// Add a memo to the transaction
        #[structopt(long, empty_values = false)]
        memo: Option<String>,
        /// A hash of a recent block, can be obtained by calling `recent-block-hash`, all parties *must* pass in the same hash.
        #[structopt(long)]
        recent_block_hash: Hash,
        /// List of addresses that are part of this
        /// Choose the desired network: Mainnet/Testnet/Devnet
        #[structopt(default_value = "testnet", long)]
        net: Network,
        /// List of addresses
        #[structopt(long, required = true, min_values = 2)]
        keys: Vec<Pubkey>,
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
    Ok(Keypair::from_bytes(&decoded)?)
}
