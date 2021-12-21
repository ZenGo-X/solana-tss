use curv::elliptic::curves::{Ed25519, Point, Scalar};
use multi_party_eddsa::protocols::{aggsig, ExpendedKeyPair, Signature as AggSiggSignature};
use solana_sdk::signature::{Keypair, Signature, Signer, SignerError};
use solana_sdk::{hash::Hash, pubkey::Pubkey, transaction::Transaction};

use crate::serialization::{AggMessage1, AggMessage2, PartialSignature, SecretAggStepOne, SecretAggStepTwo};
use crate::{create_unsigned_transaction, Error};

pub fn key_agg(mut keys: Vec<Pubkey>, key: Option<Pubkey>) -> Result<aggsig::KeyAgg, Error> {
    keys.sort(); // The order of the keys matter for the aggregate key
    let index = key.map(|k| keys.binary_search(&k)).unwrap_or(Ok(0)).map_err(|_| Error::KeyPairIsNotInKeys)?;
    let keys: Vec<_> = keys
        .into_iter()
        .map(|key| Point::from_bytes(&key.to_bytes()).expect("Should never fail, as these are valid ed25519 pubkeys"))
        .collect();
    Ok(aggsig::KeyAgg::key_aggregation_n(&keys, index))
}

pub fn step_one(keypair: Keypair) -> (AggMessage1, SecretAggStepOne) {
    let extended_kepair = ExpendedKeyPair::create_from_private_key(keypair.secret().to_bytes());
    // we don't really need to pass a message here.
    let (ephemeral, first_msg, second_msg) = aggsig::create_ephemeral_key_and_commit(&extended_kepair, &[]);

    (AggMessage1 { sender: keypair.pubkey(), msg: first_msg }, SecretAggStepOne { ephemeral, second_msg })
}

pub fn step_two(
    keypair: Keypair,
    first_messages: Vec<AggMessage1>,
    secret_state: SecretAggStepOne,
) -> (AggMessage2, SecretAggStepTwo) {
    (
        AggMessage2 { sender: keypair.pubkey(), msg: secret_state.second_msg },
        SecretAggStepTwo { ephemeral: secret_state.ephemeral, first_messages },
    )
}

pub fn step_three(
    keypair: Keypair,
    amount: f64,
    to: Pubkey,
    memo: Option<String>,
    recent_block_hash: Hash,
    keys: Vec<Pubkey>,
    second_messages: Vec<AggMessage2>,
    secret_state: SecretAggStepTwo,
) -> Result<PartialSignature, Error> {
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

    let all_nonces: Vec<_> =
        second_messages.iter().map(|msg2| msg2.msg.R.clone()).chain([secret_state.ephemeral.R]).collect();
    let combined_nonce = aggsig::get_R_tot(&all_nonces);

    let aggkey = key_agg(keys, Some(keypair.pubkey()))?;
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
    Ok(PartialSignature(sig))
}

pub fn sign_and_broadcast(
    amount: f64,
    to: Pubkey,
    memo: Option<String>,
    recent_block_hash: Hash,
    keys: Vec<Pubkey>,
    signatures: Vec<PartialSignature>,
) -> Result<Transaction, Error> {
    let aggkey = key_agg(keys, None)?;
    let aggpubkey = Pubkey::new(&*aggkey.apk.to_bytes(true));

    let partial_sigs: Vec<_> = signatures
        .into_iter()
        .map(|s| AggSiggSignature {
            R: Point::from_bytes(&s.0.as_ref()[..32]).unwrap(),
            s: Scalar::from_bytes(&s.0.as_ref()[32..]).unwrap(),
        })
        .collect();

    let full_sig = aggsig::add_signature_parts(&partial_sigs);
    let mut sig_bytes = [0u8; 64];
    sig_bytes[..32].copy_from_slice(&*full_sig.R.to_bytes(true));
    sig_bytes[32..].copy_from_slice(&full_sig.s.to_bytes());

    let sig = Signature::new(&sig_bytes);
    let mut tx = create_unsigned_transaction(amount, &to, memo, &aggpubkey);
    tx.message.recent_blockhash = recent_block_hash;
    assert_eq!(tx.signatures.len(), 1);
    tx.signatures[0] = sig;

    if tx.verify().is_err() {
        return Err(Error::InvalidSignature);
    }
    Ok(tx)
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

#[cfg(test)]
mod tests {
    use crate::native_token::lamports_to_sol;
    use crate::serialization::Serialize;
    use crate::tss::{key_agg, sign_and_broadcast, step_one, step_three, step_two};
    use rand::thread_rng;
    use solana_sdk::pubkey::Pubkey;
    use solana_sdk::signature::{Keypair, Signer};
    use solana_streamer::socket::SocketAddrSpace;
    use solana_test_validator::TestValidator;

    fn clone_keypair(k: &Keypair) -> Keypair {
        Keypair::from_bytes(&k.to_bytes()).unwrap()
    }
    fn clone_serialize<T: Serialize>(t: &T) -> T {
        let mut v = Vec::new();
        t.serialize(&mut v);
        T::deserialize(&v).unwrap()
    }
    #[test]
    fn test_roundtrip() {
        let n = 5;
        let mut rng = thread_rng();
        let keys: Vec<_> = (0..n).map(|_| Keypair::generate(&mut rng)).collect();
        let pubkeys: Vec<_> = keys.iter().map(|k| k.pubkey()).collect();
        // Key Generation
        let aggpubkey = key_agg(pubkeys.clone(), None).unwrap().apk;
        let aggpubkey_solana = Pubkey::new(&*aggpubkey.to_bytes(true));
        let full_amount = 500_000_000;
        // Get some money in it
        let testnet = TestValidator::with_no_fees(aggpubkey_solana, None, SocketAddrSpace::Unspecified);
        let rpc_client = testnet.get_rpc_client();

        // step 1
        let to = Keypair::generate(&mut rng);
        let (first_msgs, first_secrets): (Vec<_>, Vec<_>) = keys.iter().map(clone_keypair).map(step_one).unzip();

        // step 2
        let (second_msgs, second_secrets): (Vec<_>, Vec<_>) = keys
            .iter()
            .map(clone_keypair)
            .zip(first_secrets.into_iter())
            .enumerate()
            .map(|(i, (key, secret))| {
                let mut first_msgs: Vec<_> = first_msgs.iter().map(clone_serialize).collect();
                first_msgs.remove(i);
                step_two(key, first_msgs, secret)
            })
            .unzip();

        let recent_block_hash = rpc_client.get_latest_blockhash().unwrap();
        // step 3
        let amount = lamports_to_sol(full_amount / 2);
        let memo = Some("test_roundtrip".to_string());

        let partial_sigs: Vec<_> = keys
            .iter()
            .map(clone_keypair)
            .zip(second_secrets.into_iter())
            .enumerate()
            .map(|(i, (key, secret))| {
                let mut second_msgs: Vec<_> = second_msgs.iter().map(clone_serialize).collect();
                second_msgs.remove(i);
                step_three(
                    key,
                    amount,
                    to.pubkey(),
                    memo.clone(),
                    recent_block_hash,
                    pubkeys.clone(),
                    second_msgs,
                    secret,
                )
                .unwrap()
            })
            .collect();

        let full_tx = sign_and_broadcast(amount, to.pubkey(), memo, recent_block_hash, pubkeys, partial_sigs).unwrap();
        let sig = rpc_client.send_transaction(&full_tx).unwrap();

        // Wait for confirmation
        rpc_client.confirm_transaction_with_spinner(&sig, &recent_block_hash, rpc_client.commitment()).unwrap();
    }
}
