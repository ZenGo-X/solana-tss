use bincode::Options as _;
use curv::elliptic::curves::Point;
use multi_party_eddsa::protocols::aggsig::SignFirstMsg;
use multi_party_eddsa::protocols::{
    aggsig::{self, KeyAgg},
    ExpendedKeyPair,
};
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use solana_client::rpc_client::RpcClient;
use solana_sdk::instruction::Instruction;
use solana_sdk::message::Message;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::transaction::Transaction;
use solana_sdk::{native_token, signature::Signer, system_instruction};
use spl_memo::solana_program::pubkey::Pubkey;
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
            println!("secret key: {}", keypair.to_base58_string());
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
                    let memo_ins =
                        Instruction { program_id: spl_memo::id(), accounts: Vec::new(), data: memo.into_bytes() };
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
        Options::AggregateKeys { mut keys } => {
            keys.sort(); // The order of the keys matter for the aggregate key
            let keys: Vec<_> = keys
                .into_iter()
                .map(|key| {
                    Point::from_bytes(&key.to_bytes()).expect("Should never fail, as these are valid ed25519 pubkeys")
                })
                .collect();
            let aggkey = KeyAgg::key_aggregation_n(&keys, 0);
            let aggpubkey = Pubkey::new(&*aggkey.apk.to_bytes(true));
            println!("The Aggregated PublicKey: {}", aggpubkey);
        }
        Options::AggSendStepOne { keypair } => {
            let extended_kepair = ExpendedKeyPair::create_from_private_key(keypair.secret().to_bytes());
            // we don't really need to pass a message here.
            let (ephemeral, first_msg, second_msg) = aggsig::create_ephemeral_key_and_commit(&extended_kepair, &[]);
            let bincode = bincode::DefaultOptions::new().with_varint_encoding();
            println!("Message 1, send to all other parties: {}", hex::encode(bincode.serialize(&first_msg)?));

            let secret = SecretAggStepOne { ephemeral, second_msg };

            println!(
                "Secret state: keep this a secret, and pass it back to `agg-send-step-two`: {}",
                hex::encode(bincode.serialize(&secret)?)
            );
        }
        Options::AggSendStepTwo { first_messages, secret_state } => {
            let bincode = bincode::DefaultOptions::new().with_varint_encoding();
            let first_messages = first_messages
                .into_iter()
                .map(|msg| {
                    let hex_decoded = hex::decode(msg)
                        .map_err(|error| Error::DeserializationHexFailed { error, field_name: "first_messages" })?;
                    bincode
                        .deserialize(&hex_decoded)
                        .map_err(|error| Error::DeserializationBincodeFailed { error, field_name: "first_messages" })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let secret_state_bytes = hex::decode(secret_state)
                .map_err(|error| Error::DeserializationHexFailed { error, field_name: "secret_state" })?;
            let secret_state: SecretAggStepOne = bincode
                .deserialize(&secret_state_bytes)
                .map_err(|error| Error::DeserializationBincodeFailed { error, field_name: "secret_state" })?;

            println!(
                "Message 2, send to all other parties: {}",
                hex::encode(bincode.serialize(&secret_state.second_msg)?)
            );

            let secret = SecretAggStepTwo { ephemeral: secret_state.ephemeral, first_messages };
            println!(
                "Secret state: keep this a secret, and pass it back to `agg-send-step-three`: {}",
                hex::encode(bincode.serialize(&secret)?)
            );
        }
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct SecretAggStepOne {
    ephemeral: aggsig::EphemeralKey,
    second_msg: aggsig::SignSecondMsg,
}

#[derive(Serialize, Deserialize)]
struct SecretAggStepTwo {
    ephemeral: aggsig::EphemeralKey,
    first_messages: Vec<SignFirstMsg>,
}
