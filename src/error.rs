use std::error::Error as StdError;
use std::fmt::{Display, Formatter};

use bs58::decode::Error as Bs58Error;
use solana_client::client_error::ClientError;

#[derive(Debug)]
pub enum Error {
    WrongNetwork(String),
    BadBase58(Bs58Error),
    SignatureError(Box<dyn StdError>),
    AirdropFailed(ClientError),
    RecentHashFailed(ClientError),
    ConfirmingTransactionFailed(ClientError),
    BalaceFailed(ClientError),
    SendTransactionFailed(ClientError),
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
            Self::BalaceFailed(e) => write!(f, "Failed checking balance: {}", e),
            Self::SendTransactionFailed(e) => write!(f, "Failed sending transaction: {}", e),
        }
    }
}

impl From<Bs58Error> for Error {
    fn from(e: Bs58Error) -> Self {
        Self::BadBase58(e)
    }
}

impl StdError for Error {}
