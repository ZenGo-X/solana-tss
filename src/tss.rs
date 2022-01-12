#![allow(non_snake_case)]

use curv::elliptic::curves::{Ed25519, Point, Scalar};
use multi_party_eddsa::protocols::musig2::{self, PrivatePartialNonces, PublicPartialNonces};
use multi_party_eddsa::protocols::ExpandedKeyPair;
use solana_sdk::signature::{Keypair, Signature, Signer, SignerError};
use solana_sdk::{hash::Hash, pubkey::Pubkey, transaction::Transaction};

use crate::serialization::{AggMessage1, Error as DeserializationError, PartialSignature, SecretAggStepOne};
use crate::{create_unsigned_transaction, Error};

/// Create the aggregate public key, pass key=None if you don't care about the coefficient
pub fn key_agg(keys: Vec<Pubkey>, key: Option<Pubkey>) -> Result<musig2::PublicKeyAgg, Error> {
    let convert_keys = |k: Pubkey| {
        Point::from_bytes(&k.to_bytes()).map_err(|e| Error::DeserializationFailed {
            error: DeserializationError::InvalidPoint(e),
            field_name: "keys",
        })
    };
    let keys: Vec<_> = keys.into_iter().map(convert_keys).collect::<Result<_, _>>()?;
    let key = key.map(convert_keys).unwrap_or_else(|| Ok(keys[0].clone()))?;
    musig2::PublicKeyAgg::key_aggregation_n(keys, &key).ok_or(Error::KeyPairIsNotInKeys)
}

/// Generate Message1 which contains nonce, public nonce, and commitment to nonces
pub fn step_one(keypair: Keypair) -> (AggMessage1, SecretAggStepOne) {
    let extended_kepair = ExpandedKeyPair::create_from_private_key(keypair.secret().to_bytes());
    // we don't really need to pass a message here.
    let (private_nonces, public_nonces) = musig2::generate_partial_nonces(&extended_kepair, None);

    (
        AggMessage1 { sender: keypair.pubkey(), public_nonces: public_nonces.clone() },
        SecretAggStepOne { private_nonces, public_nonces },
    )
}

#[allow(clippy::too_many_arguments)]
pub fn step_two(
    keypair: Keypair,
    amount: f64,
    to: Pubkey,
    memo: Option<String>,
    recent_block_hash: Hash,
    keys: Vec<Pubkey>,
    first_messages: Vec<AggMessage1>,
    secret_state: SecretAggStepOne,
) -> Result<PartialSignature, Error> {
    let other_nonces: Vec<_> = first_messages.into_iter().map(|msg1| msg1.public_nonces.R).collect();

    // Generate the aggregate key together with the coefficient of the current keypair
    let aggkey = key_agg(keys, Some(keypair.pubkey()))?;
    let aggpubkey = Pubkey::new(&*aggkey.agg_public_key.to_bytes(true));
    let extended_kepair = ExpandedKeyPair::create_from_private_key(keypair.secret().to_bytes());

    // Create the unsigned transaction
    let mut tx = create_unsigned_transaction(amount, &to, memo, &aggpubkey);

    let signer = PartialSigner {
        signer_private_nonce: secret_state.private_nonces,
        signer_public_nonce: secret_state.public_nonces,
        other_nonces,
        extended_kepair,
        aggregated_pubkey: aggkey,
    };
    // Sign the transaction using a custom `PartialSigner`, this is required to comply with Solana's API.
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
    let aggpubkey = Pubkey::new(&*aggkey.agg_public_key.to_bytes(true));

    // Make sure all the `R`s are the same
    if !signatures[1..].iter().map(|s| &s.0.as_ref()[..32]).all(|s| s == &signatures[0].0.as_ref()[..32]) {
        return Err(Error::MismatchMessages);
    }
    let deserialize_R = |s| {
        Point::from_bytes(s).map_err(|e| Error::DeserializationFailed {
            error: DeserializationError::InvalidPoint(e),
            field_name: "signatures",
        })
    };
    let deserialize_s = |s| {
        Scalar::from_bytes(s).map_err(|e| Error::DeserializationFailed {
            error: DeserializationError::InvalidScalar(e),
            field_name: "signatures",
        })
    };

    let first_sig = musig2::PartialSignature {
        R: deserialize_R(&signatures[0].0.as_ref()[..32])?,
        my_partial_s: deserialize_s(&signatures[0].0.as_ref()[32..])?,
    };

    let partial_sigs: Vec<_> =
        signatures[1..].iter().map(|s| deserialize_s(&s.0.as_ref()[32..])).collect::<Result<_, _>>()?;

    // Add the signatures up
    let full_sig = musig2::aggregate_partial_signatures(&first_sig, &partial_sigs);

    let mut sig_bytes = [0u8; 64];
    sig_bytes[..32].copy_from_slice(&*full_sig.R.to_bytes(true));
    sig_bytes[32..].copy_from_slice(&full_sig.s.to_bytes());
    let sig = Signature::new(&sig_bytes);

    // Create the same transaction again
    let mut tx = create_unsigned_transaction(amount, &to, memo, &aggpubkey);
    // Insert the recent_block_hash and the signature to the right places
    tx.message.recent_blockhash = recent_block_hash;
    assert_eq!(tx.signatures.len(), 1);
    tx.signatures[0] = sig;

    // Make sure the resulting transaction is actually valid.
    if tx.verify().is_err() {
        return Err(Error::InvalidSignature);
    }
    Ok(tx)
}

struct PartialSigner {
    signer_private_nonce: PrivatePartialNonces,
    signer_public_nonce: PublicPartialNonces,
    other_nonces: Vec<[Point<Ed25519>; 2]>,
    extended_kepair: ExpandedKeyPair,
    aggregated_pubkey: musig2::PublicKeyAgg,
}

impl Signer for PartialSigner {
    fn try_pubkey(&self) -> Result<Pubkey, SignerError> {
        Ok(Pubkey::new(&*self.aggregated_pubkey.agg_public_key.to_bytes(true)))
    }

    fn try_sign_message(&self, message: &[u8]) -> Result<Signature, SignerError> {
        let sig = musig2::partial_sign(
            &self.other_nonces,
            self.signer_private_nonce.clone(),
            self.signer_public_nonce.clone(),
            &self.aggregated_pubkey,
            &self.extended_kepair,
            message,
        );
        let mut sig_bytes = [0u8; 64];
        sig_bytes[..32].copy_from_slice(&*sig.R.to_bytes(true));
        sig_bytes[32..].copy_from_slice(&sig.my_partial_s.to_bytes());
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
    use crate::tss::{key_agg, sign_and_broadcast, step_one, step_two};
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
        let mut rng = rand07::thread_rng();
        let keys: Vec<_> = (0..n).map(|_| Keypair::generate(&mut rng)).collect();
        let pubkeys: Vec<_> = keys.iter().map(|k| k.pubkey()).collect();
        // Key Generation
        let aggpubkey = key_agg(pubkeys.clone(), None).unwrap().agg_public_key;
        let aggpubkey_solana = Pubkey::new(&*aggpubkey.to_bytes(true));
        let full_amount = 500_000_000;
        // Get some money in it
        let testnet = TestValidator::with_no_fees(aggpubkey_solana, None, SocketAddrSpace::Unspecified);
        let rpc_client = testnet.get_rpc_client();

        // step 1
        let to = Keypair::generate(&mut rng);
        let (first_msgs, first_secrets): (Vec<_>, Vec<_>) = keys.iter().map(clone_keypair).map(step_one).unzip();

        let recent_block_hash = rpc_client.get_latest_blockhash().unwrap();
        // step 2
        let amount = lamports_to_sol(full_amount / 2);
        let memo = Some("test_roundtrip".to_string());

        let partial_sigs: Vec<_> = keys
            .iter()
            .map(clone_keypair)
            .zip(first_secrets.into_iter())
            .enumerate()
            .map(|(i, (key, secret))| {
                let mut first_msgs: Vec<_> = first_msgs.iter().map(clone_serialize).collect();
                first_msgs.remove(i);
                step_two(key, amount, to.pubkey(), memo.clone(), recent_block_hash, pubkeys.clone(), first_msgs, secret)
                    .unwrap()
            })
            .collect();

        let full_tx = sign_and_broadcast(amount, to.pubkey(), memo, recent_block_hash, pubkeys, partial_sigs).unwrap();
        let sig = rpc_client.send_transaction(&full_tx).unwrap();

        // Wait for confirmation
        rpc_client.confirm_transaction_with_spinner(&sig, &recent_block_hash, rpc_client.commitment()).unwrap();
    }
}
