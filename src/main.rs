use rand::thread_rng;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{native_token, signature::Signer, system_instruction};
use solana_sdk::instruction::Instruction;
use solana_sdk::message::Message;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::transaction::Transaction;
use structopt::StructOpt;

use crate::cli::Options;
use crate::error::Error;

mod cli;
mod error;

fn main() -> Result<(), Error> {
    let opts = Options::from_args();
    let mut rng = thread_rng();
    match opts {
        Options::Generate => {
            let keypair = Keypair::generate(&mut rng);
            println!("secret key: {}", bs58::encode(keypair.secret()).into_string());
            println!("public key: {}", keypair.pubkey());
        }
        Options::Balance { address, net } => {
            let rpc_client = RpcClient::new(net.get_cluster_url().to_string());
            let balance = rpc_client.get_balance(&address).map_err(Error::BalaceFailed)?;
            println!("The balance of {} is: {}", address, balance);
        }
        Options::Airdrop { to, amount, net } => {
            // TODO: Check balance before and after, and if didn't change verify with get_signature_statuses_with_history
            let rpc_client = RpcClient::new(net.get_cluster_url().to_string());
            let amount = native_token::sol_to_lamports(amount);
            let sig = rpc_client.request_airdrop(&to, amount).map_err(Error::AirdropFailed)?;
            println!("Airdrop transaction ID: {}", sig);
            let recent_hash = rpc_client.get_latest_blockhash().map_err(Error::RecentHashFailed)?;
            rpc_client
                .confirm_transaction_with_spinner(&sig, &recent_hash, rpc_client.commitment())
                .map_err(Error::ConfirmingTransactionFailed)?;
        }
        Options::SendSingle { keypair, amount, to, net, memo } => {
            let rpc_client = RpcClient::new(net.get_cluster_url().to_string());
            let amount = native_token::sol_to_lamports(amount);
            let transfer_ins = system_instruction::transfer(&keypair.pubkey(), &to, amount);
            let msg = match memo {
                None => Message::new(&[transfer_ins], Some(&keypair.pubkey())),
                Some(memo) => {
                    let memo_ins = Instruction {
                        program_id: spl_memo::id(),
                        accounts: Vec::new(),
                        data: memo.into_bytes(),
                    };
                    Message::new(&[transfer_ins, memo_ins], Some(&keypair.pubkey()))
                }
            };
            let mut tx = Transaction::new_unsigned(msg);
            let recent_hash = rpc_client.get_latest_blockhash().map_err(Error::RecentHashFailed)?;
            tx.sign(&[&keypair], recent_hash);
            let sig = rpc_client.send_transaction(&tx).map_err(Error::SendTransactionFailed)?;
            println!("Transaction ID: {}", sig);
            rpc_client
                .confirm_transaction_with_spinner(&sig, &recent_hash, rpc_client.commitment())
                .map_err(Error::ConfirmingTransactionFailed)?;
        }
    }
    Ok(())
}
