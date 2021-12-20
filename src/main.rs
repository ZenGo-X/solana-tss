use curv::elliptic::curves::{Ed25519, Point, Scalar};
use multi_party_eddsa::protocols::{
    aggsig::{self, KeyAgg},
    ExpendedKeyPair, Signature as AggSiggSignature,
};
use rand::thread_rng;
use solana_client::rpc_client::RpcClient;
use solana_sdk::instruction::Instruction;
use solana_sdk::message::Message;
use solana_sdk::signature::{Signature, SignerError};
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::transaction::Transaction;
use solana_sdk::{native_token, signature::Signer, system_instruction};
use spl_memo::solana_program::pubkey::Pubkey;
use structopt::StructOpt;

use crate::cli::Options;
use crate::error::Error;
use crate::serialization::{
    AggMessage1, AggMessage2, FieldError, PartialSignature, SecretAggStepOne, SecretAggStepTwo, Serialize,
};

mod cli;
mod error;
mod serialization;

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
            let mut tx = create_unsigned_transaction(amount, &to, memo, &keypair.pubkey());
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
            let first_msg = AggMessage1 { sender: keypair.pubkey(), msg: first_msg };
            println!("Message 1, send to all other parties: {}", first_msg.serialize_bs58());

            let secret = SecretAggStepOne { ephemeral, second_msg };

            println!(
                "Secret state: keep this a secret, and pass it back to `agg-send-step-two`: {}",
                secret.serialize_bs58()
            );
        }
        Options::AggSendStepTwo { keypair, first_messages, secret_state } => {
            let first_messages = first_messages
                .into_iter()
                .map(|msg| AggMessage1::deserialize_bs58(msg).with_field("first_message"))
                .collect::<Result<Vec<_>, _>>()?;
            let secret_state = SecretAggStepOne::deserialize_bs58(&secret_state).with_field("secret_state")?;

            let serialized_second_msg =
                AggMessage2 { sender: keypair.pubkey(), msg: secret_state.second_msg }.serialize_bs58();
            let serialized_secret =
                SecretAggStepTwo { ephemeral: secret_state.ephemeral, first_messages }.serialize_bs58();
            println!("Message 2, send to all other parties: {}", serialized_second_msg);
            println!(
                "Secret state: keep this a secret, and pass it back to `agg-send-step-three`: {}",
                serialized_secret
            );
        }
        Options::AggSendStepThree {
            keypair,
            amount,
            to,
            memo,
            recent_block_hash,
            mut keys,
            second_messages,
            secret_state,
        } => {
            let second_messages = second_messages
                .into_iter()
                .map(|msg| AggMessage2::deserialize_bs58(&msg).with_field("second_messages"))
                .collect::<Result<Vec<_>, _>>()?;
            let secret_state: SecretAggStepTwo =
                SecretAggStepTwo::deserialize_bs58(secret_state).with_field("secret_state")?;

            let commitments_are_valid = secret_state.first_messages.into_iter().all(|msg1| {
                second_messages
                    .iter()
                    .find(|msg2| msg1.sender == msg2.sender)
                    .map(|msg2| msg1.verify_commitment(msg2))
                    .unwrap_or(false)
            });
            if !commitments_are_valid {
                return Err(Error::MismatchMessages);
            }

            let all_nonces: Vec<_> = second_messages.iter().map(|msg2| msg2.msg.R.clone()).collect();
            let combined_nonce = aggsig::get_R_tot(&all_nonces);

            keys.sort(); // The order of the keys matter for the aggregate key
            let keys: Vec<_> = keys
                .into_iter()
                .map(|key| {
                    Point::from_bytes(&key.to_bytes()).expect("Should never fail, as these are valid ed25519 pubkeys")
                })
                .collect();
            let aggkey = KeyAgg::key_aggregation_n(&keys, 0);
            let aggpubkey = Pubkey::new(&*aggkey.apk.to_bytes(true));
            let extended_kepair = ExpendedKeyPair::create_from_private_key(keypair.secret().to_bytes());

            let mut tx = create_unsigned_transaction(amount, &to, memo, &aggpubkey);

            let signer = PartialSigner {
                single_nonce: secret_state.ephemeral.r,
                combined_nonce,
                extended_kepair,
                coefficient: aggkey.hash,
                combined_pubkey: aggkey.apk,
            };
            tx.sign(&[&signer], recent_block_hash);
            let sig = tx.signatures[0];

            println!("partial signature: {}", PartialSignature(sig).serialize_bs58());
        }
        Options::AggregateSignaturesAndBroadcast { signatures, amount, to, memo, recent_block_hash, net, mut keys } => {
            keys.sort(); // The order of the keys matter for the aggregate key
            let keys: Vec<_> = keys
                .into_iter()
                .map(|key| {
                    Point::from_bytes(&key.to_bytes()).expect("Should never fail, as these are valid ed25519 pubkeys")
                })
                .collect();
            let aggkey = KeyAgg::key_aggregation_n(&keys, 0);
            let aggpubkey = Pubkey::new(&*aggkey.apk.to_bytes(true));

            let partial_sigs = signatures
                .into_iter()
                .map(|s| PartialSignature::deserialize_bs58(s).with_field("signatures"))
                .map(|s| {
                    s.map(|s| AggSiggSignature {
                        R: Point::from_bytes(&s.0.as_ref()[..32]).unwrap(),
                        s: Scalar::from_bytes(&s.0.as_ref()[32..]).unwrap(),
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            let full_sig = aggsig::add_signature_parts(&partial_sigs);
            let mut sig_bytes = [0u8; 64];
            sig_bytes[..32].copy_from_slice(&*full_sig.R.to_bytes(true));
            sig_bytes[32..].copy_from_slice(&full_sig.s.to_bytes());

            let sig = Signature::new(&sig_bytes);
            let mut tx = create_unsigned_transaction(amount, &to, memo, &aggpubkey);

            tx.signatures.push(sig);
            if tx.verify().is_err() {
                return Err(Error::InvalidSignature);
            }
            println!("Transaction ID: {}", sig);
            let rpc_client = RpcClient::new(net.get_cluster_url().to_string());
            rpc_client
                .confirm_transaction_with_spinner(&sig, &recent_block_hash, rpc_client.commitment())
                .map_err(Error::ConfirmingTransactionFailed)?;
        }
    }
    Ok(())
}

fn create_unsigned_transaction(amount: f64, to: &Pubkey, memo: Option<String>, payer: &Pubkey) -> Transaction {
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

struct PartialSigner {
    single_nonce: Scalar<Ed25519>,
    combined_nonce: Point<Ed25519>,
    extended_kepair: ExpendedKeyPair,
    coefficient: Scalar<Ed25519>,
    combined_pubkey: Point<Ed25519>,
}

impl Signer for PartialSigner {
    fn try_pubkey(&self) -> Result<Pubkey, SignerError> {
        Ok(Pubkey::new(&*self.combined_pubkey.to_bytes(true)))
    }

    fn try_sign_message(&self, message: &[u8]) -> Result<Signature, SignerError> {
        let sig = aggsig::partial_sign(
            &self.single_nonce,
            &self.extended_kepair,
            &self.coefficient,
            &self.combined_nonce,
            &self.combined_pubkey,
            message,
        );
        let mut sig_bytes = [0u8; 64];
        sig_bytes[..32].copy_from_slice(&*sig.R.to_bytes(true));
        sig_bytes[32..].copy_from_slice(&sig.s.to_bytes());
        Ok(Signature::new(&sig_bytes))
    }

    fn is_interactive(&self) -> bool {
        false
    }
}
