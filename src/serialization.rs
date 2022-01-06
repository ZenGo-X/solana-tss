use std::fmt::{Display, Formatter};

use curv::elliptic::curves::{DeserializationError, Ed25519, Point, PointFromBytesError, Scalar};
use multi_party_eddsa::protocols::musig2::PartialNonces;
use solana_sdk::signature::Signature;
use spl_memo::solana_program::pubkey::Pubkey;

#[derive(Debug)]
pub enum Error {
    InputTooShort { expected: usize, found: usize },
    BadBase58(bs58::decode::Error),
    InvalidPoint(PointFromBytesError),
    InvalidScalar(DeserializationError),
    WrongTag { expected: Tag, found: Tag },
}

// TODO: Also add a magic tag that is equal to all messages.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tag {
    AggMessage1 = 0,
    AggMessage2 = 1,
    PartialSignature = 2,
    SecretAggStepOne = 3,
    Unknown,
}

impl From<u8> for Tag {
    fn from(t: u8) -> Self {
        match t {
            _ if t == Tag::AggMessage1 as u8 => Tag::AggMessage1,
            _ if t == Tag::AggMessage2 as u8 => Tag::AggMessage2,
            _ if t == Tag::PartialSignature as u8 => Tag::PartialSignature,
            _ if t == Tag::SecretAggStepOne as u8 => Tag::SecretAggStepOne,
            _ => Tag::Unknown,
        }
    }
}

impl Display for Tag {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Tag::AggMessage1 => f.write_str("Aggregate Message1"),
            Tag::AggMessage2 => f.write_str("Aggregate Message2"),
            Tag::PartialSignature => f.write_str("Partial Signature"),
            Tag::SecretAggStepOne => f.write_str("Secret State Aggregate1"),
            Tag::Unknown => f.write_str("Unknown"),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InputTooShort { expected, found } => {
                write!(f, "Input too short, expected: {}, found: {}", expected, found)
            }
            Self::BadBase58(e) => write!(f, "Invalid base58: {}", e),
            Self::InvalidPoint(e) => write!(f, "Invalid Ed25519 Point: {}", e),
            Self::InvalidScalar(e) => write!(f, "Invalid Ed25519 Scalar: {}", e),
            Self::WrongTag { expected, found } => {
                write!(f, "Expected to find message: {}, instead found: {}", expected, found)
            }
        }
    }
}
impl std::error::Error for Error {}

impl From<PointFromBytesError> for Error {
    fn from(e: PointFromBytesError) -> Self {
        Self::InvalidPoint(e)
    }
}

impl From<DeserializationError> for Error {
    fn from(e: DeserializationError) -> Self {
        Self::InvalidScalar(e)
    }
}

pub trait FieldError<T> {
    fn with_field(self, field_name: &'static str) -> Result<T, crate::Error>;
}
impl<T: Serialize> FieldError<T> for Result<T, Error> {
    fn with_field(self, field_name: &'static str) -> Result<T, crate::Error> {
        self.map_err(|error| crate::Error::DeserializationFailed { error, field_name })
    }
}

pub trait Serialize: Sized {
    fn serialize_bs58(&self) -> String {
        let mut vec = Vec::with_capacity(self.size_hint());
        self.serialize(&mut vec);
        bs58::encode(vec).into_string()
    }
    fn serialize(&self, append_to: &mut Vec<u8>);
    fn deserialize_bs58(s: impl AsRef<[u8]>) -> Result<Self, Error> {
        let out = bs58::decode(s).into_vec().map_err(Error::BadBase58)?;
        Self::deserialize(&out)
    }
    // TODO: Use a cursor instead, more fail safe.
    fn deserialize(b: &[u8]) -> Result<Self, Error>;
    fn size_hint(&self) -> usize;
}

#[derive(Debug, PartialEq)]
pub struct AggMessage1 {
    pub public_nonces: [Point<Ed25519>; 2],
    pub sender: Pubkey,
}

impl Serialize for AggMessage1 {
    fn serialize(&self, append_to: &mut Vec<u8>) {
        append_to.reserve(self.size_hint());
        append_to.push(Tag::AggMessage1 as u8);
        append_to.extend(&*self.public_nonces[0].to_bytes(true));
        append_to.extend(&*self.public_nonces[1].to_bytes(true));
        append_to.extend(self.sender.to_bytes());
    }
    fn deserialize(b: &[u8]) -> Result<Self, Error> {
        if b.len() < 1 + 32 + 32 + 32 {
            return Err(Error::InputTooShort { expected: 1 + 32 + 32 + 32, found: b.len() });
        }
        let tag = Tag::from(b[0]);
        if tag != Tag::AggMessage1 {
            return Err(Error::WrongTag { expected: Tag::AggMessage1, found: tag });
        }
        let public_nonces = [Point::from_bytes(&b[1..32 + 1])?, Point::from_bytes(&b[1 + 32..64 + 1])?];
        let sender = Pubkey::new(&b[64 + 1..64 + 32 + 1]);
        Ok(Self { public_nonces, sender })
    }
    fn size_hint(&self) -> usize {
        1 + 32 + 32 + 32
    }
}

#[derive(Debug, PartialEq)]
pub struct PartialSignature(pub Signature);

impl Serialize for PartialSignature {
    fn serialize(&self, append_to: &mut Vec<u8>) {
        append_to.reserve(self.size_hint());
        append_to.push(Tag::PartialSignature as u8);

        append_to.extend(self.0.as_ref());
    }
    fn deserialize(b: &[u8]) -> Result<Self, Error> {
        if b.len() < 1 + 64 {
            return Err(Error::InputTooShort { expected: 1 + 64, found: b.len() });
        }
        let tag = Tag::from(b[0]);
        if tag != Tag::PartialSignature {
            return Err(Error::WrongTag { expected: Tag::PartialSignature, found: tag });
        }
        Ok(PartialSignature(Signature::new(&b[1..1 + 64])))
    }
    fn size_hint(&self) -> usize {
        1 + 64
    }
}

#[derive(Debug)]
pub struct SecretAggStepOne {
    pub ephemeral: PartialNonces,
}

impl PartialEq for SecretAggStepOne {
    fn eq(&self, other: &Self) -> bool {
        self.ephemeral.r.eq(&other.ephemeral.r) && self.ephemeral.R.eq(&self.ephemeral.R)
    }
}

impl Serialize for SecretAggStepOne {
    fn serialize(&self, append_to: &mut Vec<u8>) {
        append_to.reserve(self.size_hint());
        append_to.push(Tag::SecretAggStepOne as u8);

        append_to.extend(&*self.ephemeral.r[0].to_bytes());
        append_to.extend(&*self.ephemeral.r[1].to_bytes());
        append_to.extend(&*self.ephemeral.R[0].to_bytes(true));
        append_to.extend(&*self.ephemeral.R[1].to_bytes(true));
    }
    fn deserialize(b: &[u8]) -> Result<Self, Error> {
        if b.len() < 1 + 64 + 64 {
            return Err(Error::InputTooShort { expected: 1 + 64 + 64, found: b.len() });
        }

        let tag = Tag::from(b[0]);
        if tag != Tag::SecretAggStepOne {
            return Err(Error::WrongTag { expected: Tag::SecretAggStepOne, found: tag });
        }
        let r = [Scalar::from_bytes(&b[1..1 + 32])?, Scalar::from_bytes(&b[1 + 32..1 + 64])?];
        #[allow(non_snake_case)]
        let R = [Point::from_bytes(&b[1 + 64..1 + 64 + 32])?, Point::from_bytes(&b[1 + 96..1 + 96 + 32])?];
        Ok(Self { ephemeral: PartialNonces { R, r } })
    }
    fn size_hint(&self) -> usize {
        1 + 64 + 64
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use multi_party_eddsa::protocols::{musig2, ExpandedKeyPair};
    use solana_sdk::signature::Signature;
    use spl_memo::solana_program::pubkey::Pubkey;

    use crate::serialization::{AggMessage1, PartialSignature, SecretAggStepOne, Serialize};

    #[derive(PartialEq, Debug)]
    struct PanicEq<T: PartialEq + Debug>(T);

    impl<T: PartialEq + Debug> Eq for PanicEq<T> {
        fn assert_receiver_is_total_eq(&self) {}
    }

    #[test]
    fn test_agg_msg1() {
        let mut msg = [0u8; 32];
        let mut sender = [0u8; 32];
        let mut rng = rand08::thread_rng();
        for i in 0..u8::MAX {
            sender.fill(i);
            msg.fill(i);
            let partial_nonces = musig2::generate_partial_nonces(&ExpandedKeyPair::create(), Some(&msg), &mut rng);
            let aggmsg1 = AggMessage1 { public_nonces: partial_nonces.R, sender: Pubkey::new(&sender) };
            let serialized = aggmsg1.serialize_bs58();
            let deserialized = AggMessage1::deserialize_bs58(serialized).unwrap();
            assert_eq!(PanicEq(aggmsg1), PanicEq(deserialized));
        }
    }

    #[test]
    fn test_agg_partial_signature() {
        let mut signature = [0u8; 64];
        for i in 0..u8::MAX {
            signature.fill(i);
            let partial_sig = PartialSignature(Signature::new(&signature));
            let serialized = partial_sig.serialize_bs58();
            let deserialized = PartialSignature::deserialize_bs58(serialized).unwrap();
            assert_eq!(PanicEq(partial_sig), PanicEq(deserialized));
        }
    }

    #[test]
    fn test_serialize_secret_agg1() {
        let mut rng = rand08::thread_rng();
        let mut data = [0u8; 32];
        for i in 0..u8::MAX {
            data.fill(i);
            let partial_nonces = musig2::generate_partial_nonces(&ExpandedKeyPair::create(), Some(&data), &mut rng);
            let secret_agg1 = SecretAggStepOne { ephemeral: partial_nonces };
            let serialized = secret_agg1.serialize_bs58();
            let deserialized = SecretAggStepOne::deserialize_bs58(serialized).unwrap();
            assert_eq!(PanicEq(secret_agg1), PanicEq(deserialized));
        }
    }
}
