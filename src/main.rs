use clap::Parser;
use solana_client::rpc_client::RpcClient;
use solana_sdk::instruction::Instruction;
use solana_sdk::message::Message;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::transaction::Transaction;
use solana_sdk::{native_token, signature::Signer, system_instruction};
use spl_memo::solana_program::pubkey::Pubkey;

use crate::cli::Options;
use crate::error::Error;
use crate::serialization::Serialize;

mod cli;
mod error;
mod serialization;
mod tss;

fn main() -> Result<(), Error> {
    let opts = Options::parse();
    match opts {
        Options::Generate => {
            let keypair = Keypair::generate(&mut rand07::thread_rng());
            println!("secret share: {}", keypair.to_base58_string());
            println!("public share: {}", keypair.pubkey());
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
            let mut tx = create_unsigned_transaction(amount, &to, memo, &keypair.pubkey());
            let recent_hash = rpc_client.get_latest_blockhash().map_err(Error::RecentHashFailed)?;
            tx.sign(&[&keypair], recent_hash);
            let sig = rpc_client.send_transaction(&tx).map_err(Error::SendTransactionFailed)?;
            println!("Transaction ID: {}", sig);
            rpc_client
                .confirm_transaction_with_spinner(&sig, &recent_hash, rpc_client.commitment())
                .map_err(Error::ConfirmingTransactionFailed)?;
        }
        Options::RecentBlockHash { net } => {
            let rpc_client = RpcClient::new(net.get_cluster_url().to_string());
            let recent_hash = rpc_client.get_latest_blockhash().map_err(Error::RecentHashFailed)?;
            println!("recent block hash: {}", recent_hash);
        }
        Options::AggregateKeys { keys } => {
            let aggkey = tss::key_agg(keys, None)?;
            let aggpubkey = Pubkey::new(&*aggkey.agg_public_key.to_bytes(true));
            println!("The Aggregated Public Key: {}", aggpubkey);
        }
        Options::AggSendStepOne { keypair } => {
            let (first_msg, secret) = tss::step_one(keypair);

            println!("Message 1: {} (send to all other parties)", first_msg.serialize_bs58());
            println!(
                "Secret state: {} (keep this a secret, and pass it back to `agg-send-step-two`)",
                secret.serialize_bs58()
            );
        }
        Options::AggSendStepTwo {
            keypair,
            amount,
            to,
            memo,
            recent_block_hash,
            keys,
            first_messages,
            secret_state,
        } => {
            let sig = tss::step_two(keypair, amount, to, memo, recent_block_hash, keys, first_messages, secret_state)?;
            println!("Partial signature: {}", sig.serialize_bs58());
        }
        Options::AggregateSignaturesAndBroadcast { signatures, amount, to, memo, recent_block_hash, net, keys } => {
            let tx = tss::sign_and_broadcast(amount, to, memo, recent_block_hash, keys, signatures)?;
            let rpc_client = RpcClient::new(net.get_cluster_url().to_string());
            let sig = rpc_client.send_transaction(&tx).map_err(Error::SendTransactionFailed)?;
            println!("Transaction ID: {}", sig);
            rpc_client
                .confirm_transaction_with_spinner(&sig, &recent_block_hash, rpc_client.commitment())
                .map_err(Error::ConfirmingTransactionFailed)?;
        }
    }
    Ok(())
}

pub fn create_unsigned_transaction(amount: f64, to: &Pubkey, memo: Option<String>, payer: &Pubkey) -> Transaction {
    let amount = native_token::sol_to_lamports(amount);
    let transfer_ins = system_instruction::transfer(payer, to, amount);
    let msg = match memo {
        None => Message::new(&[transfer_ins], Some(payer)),
        Some(memo) => {
            let memo_ins = Instruction { program_id: spl_memo::id(), accounts: Vec::new(), data: memo.into_bytes() };
            Message::new(&[transfer_ins, memo_ins], Some(payer))
        }
    };
    Transaction::new_unsigned(msg)
}
