use bs58::decode::Error as Bs58Error;
use solana_client::client_error::ClientError;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum Error {
    WrongNetwork(String),
    BadBase58(Bs58Error),
    WrongKeyPair(ed25519_dalek::SignatureError),
    AirdropFailed(ClientError),
    RecentHashFailed(ClientError),
    ConfirmingTransactionFailed(ClientError),
    BalaceFailed(ClientError),
    SendTransactionFailed(ClientError),
    SerializationFailed(bincode::Error),
    DeserializationHexFailed { error: hex::FromHexError, field_name: &'static str },
    DeserializationBincodeFailed { error: bincode::Error, field_name: &'static str },
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongNetwork(net) => write!(f, "Unrecognized network: {}, please select Mainnet/Testnet/Devnet", net),
            Self::BadBase58(e) => write!(f, "Based58 Error: {}", e),
            Self::WrongKeyPair(e) => write!(f, "Failed deserializing keypair: {}", e),
            Self::AirdropFailed(e) => write!(f, "Failed asking for an airdrop: {}", e),
            Self::RecentHashFailed(e) => write!(f, "Failed recieving the latest hash: {}", e),
            Self::ConfirmingTransactionFailed(e) => {
                write!(f, "Failed confirming transaction: {}", e)
            }
            Self::BalaceFailed(e) => write!(f, "Failed checking balance: {}", e),
            Self::SendTransactionFailed(e) => write!(f, "Failed sending transaction: {}", e),
            Self::SerializationFailed(e) => write!(f, "Failed serializing an object: {}", e),
            Self::DeserializationHexFailed { error, field_name } => {
                write!(f, "Failed deserializing hex of {}: {}", field_name, error)
            }
            Self::DeserializationBincodeFailed { error, field_name } => {
                write!(f, "Failed deserializing {}: {}", field_name, error)
            }
        }
    }
}

impl From<Bs58Error> for Error {
    fn from(e: Bs58Error) -> Self {
        Self::BadBase58(e)
    }
}

impl From<ed25519_dalek::SignatureError> for Error {
    fn from(e: ed25519_dalek::SignatureError) -> Self {
        Self::WrongKeyPair(e)
    }
}

impl From<bincode::Error> for Error {
    fn from(e: bincode::Error) -> Self {
        Self::SerializationFailed(e)
    }
}

impl std::error::Error for Error {}
