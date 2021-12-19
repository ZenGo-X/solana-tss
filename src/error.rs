use bs58::decode::Error as Bs58Error;
use solana_client::client_error::ClientError;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum Error {
    WrongNetwork(String),
    BadBase58(Bs58Error),
    SignatureError(ed25519_dalek::SignatureError),
    AirdropFailed(ClientError),
    RecentHashFailed(ClientError),
    ConfirmingTransactionFailed(ClientError),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongNetwork(net) => write!(f, "Unrecognized network: {}, please select Mainnet/Testnet/Devnet", net),
            Self::BadBase58(e) => write!(f, "Based58 Error: {}", e),
            Self::SignatureError(e) => write!(f, "SignatureError: {}", e),
            Self::AirdropFailed(e) => write!(f, "Failed asking for an airdrop: {}", e),
            Self::RecentHashFailed(e) => write!(f, "Failed recieving the latest hash: {}", e),
            Self::ConfirmingTransactionFailed(e) => {
                write!(f, "Failed confirming transaction: {}", e)
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
        Self::SignatureError(e)
    }
}

impl std::error::Error for Error {}
