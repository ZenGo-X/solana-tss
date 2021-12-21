use crate::{
    create_unsigned_transaction, AggMessage1, AggMessage2, Error, FieldError, PartialSignature, Pubkey,
    SecretAggStepOne, SecretAggStepTwo, Serialize,
};
use curv::elliptic::curves::{Ed25519, Point, Scalar};
use multi_party_eddsa::protocols::{aggsig, ExpendedKeyPair, Signature as AggSiggSignature};
use solana_sdk::hash::Hash;
use solana_sdk::signature::{Keypair, Signature, Signer, SignerError};
use solana_sdk::transaction::Transaction;

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
    first_messages: Vec<String>,
    secret_state: String,
) -> Result<(AggMessage2, SecretAggStepTwo), Error> {
    let first_messages = first_messages
        .into_iter()
        .map(|msg| AggMessage1::deserialize_bs58(msg).with_field("first_message"))
        .collect::<Result<Vec<_>, _>>()?;
    let secret_state = SecretAggStepOne::deserialize_bs58(&secret_state).with_field("secret_state")?;

    Ok((
        AggMessage2 { sender: keypair.pubkey(), msg: secret_state.second_msg },
        SecretAggStepTwo { ephemeral: secret_state.ephemeral, first_messages },
    ))
}

pub fn step_three(
    keypair: Keypair,
    amount: f64,
    to: Pubkey,
    memo: Option<String>,
    recent_block_hash: Hash,
    keys: Vec<Pubkey>,
    second_messages: Vec<String>,
    secret_state: String,
) -> Result<PartialSignature, Error> {
    let second_messages = second_messages
        .into_iter()
        .map(|msg| AggMessage2::deserialize_bs58(&msg).with_field("second_messages"))
        .collect::<Result<Vec<_>, _>>()?;
    let secret_state: SecretAggStepTwo = SecretAggStepTwo::deserialize_bs58(secret_state).with_field("secret_state")?;

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
    keys: Vec<Pubkey>,
    signatures: Vec<String>,
) -> Result<Transaction, Error> {
    let aggkey = key_agg(keys, None)?;
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
