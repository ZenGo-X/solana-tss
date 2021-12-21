use std::fmt::{Display, Formatter};

use bs58::decode::Error as Bs58Error;
use solana_client::client_error::ClientError;

use crate::serialization::Error as DeserializationError;

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
    DeserializationFailed { error: DeserializationError, field_name: &'static str },
    MismatchMessages,
    InvalidSignature,
    KeyPairIsNotInKeys,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongNetwork(net) => write!(f, "Unrecognized network: {}, please select Mainnet/Testnet/Devnet", net),
            Self::BadBase58(e) => write!(f, "Based58 Error: {}", e),
            Self::WrongKeyPair(e) => write!(f, "Failed deserializing keypair: {}", e),
            Self::AirdropFailed(e) => write!(f, "Failed asking for an airdrop: {}", e),
            Self::RecentHashFailed(e) => write!(f, "Failed recieving the latest hash: {}", e),
            Self::ConfirmingTransactionFailed(e) => write!(f, "Failed confirming transaction: {}", e),
            Self::BalaceFailed(e) => write!(f, "Failed checking balance: {}", e),
            Self::SendTransactionFailed(e) => write!(f, "Failed sending transaction: {}", e),
            Self::DeserializationFailed { error, field_name } => {
                write!(f, "Failed deserializing {}: {}", field_name, error)
            }
            Self::MismatchMessages => write!(f, "There is a mismatch between first_messages and second_messages"),
            Self::InvalidSignature => write!(f, "The resulting signature doesn't match the transaction"),
            Self::KeyPairIsNotInKeys => write!(f, "The provided keypair is not in the list of pubkeys"),
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

impl std::error::Error for Error {}
