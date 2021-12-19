use std::error::Error;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use ed25519_dalek::SecretKey;
use structopt::StructOpt;
use bs58::decode::Error as Bs58Error;
use solana_sdk::pubkey::Pubkey;

#[derive(Debug, StructOpt)]
#[structopt(name = "solana-tss", about = "A PoC for managing a Solana TSS wallet.")]
pub enum Options {
    /// Generate a pair of keys.
    Generate,
    /// Check the balance of an address.
    Balance {
        /// The address to check the balance of
        address: Pubkey,
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
        #[structopt(parse(try_from_str = parse_private_key_bs58))]
        keypair: SecretKey,
        /// The amount of SOL you want to send.
        amount: f64,
        /// Address of the recipient
        to: Pubkey,
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
    type Err = CliError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mainnet" | "Mainnet" => Ok(Self::Mainnet),
            "testnet" | "Testnet" => Ok(Self::Testnet),
            "devnet" | "Devnet" => Ok(Self::Devnet),
            _ => Err(CliError::WrongNetwork(s.to_string()))
        }
    }
}

fn parse_private_key_bs58(s: &str) -> Result<SecretKey, CliError> {
   let decoded = bs58::decode(s).into_vec()?;
    Ok(SecretKey::from_bytes(&decoded)?)
}

#[derive(Debug)]
pub enum CliError {
    WrongNetwork(String),
    BadBase58(Bs58Error),
    SignatureError(ed25519_dalek::SignatureError),
    AirdropFailed()
}

impl Display for CliError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::WrongNetwork(net) => write!(f, "Unrecognized network: {}, please select Mainnet/Testnet/Devnet", net),
            CliError::BadBase58(e) => write!(f, "Based58 Error: {}", e),
            CliError::SignatureError(e) => write!(f, "SignatureError: {}", e),
        }
    }
}

impl From<Bs58Error> for CliError {
    fn from(e: Bs58Error) -> Self {
        Self::BadBase58(e)
    }
}

impl From<ed25519_dalek::SignatureError> for CliError {
    fn from(e: ed25519_dalek::SignatureError) -> Self {
        Self::SignatureError(e)
    }
}

impl Error for CliError {}
