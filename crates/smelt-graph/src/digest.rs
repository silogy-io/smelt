use std::marker::PhantomData;

const DIGEST_LEN: usize = 20;

use hex::FromHexError;

use thiserror::Error;
#[derive(Error, Debug)]
pub enum DigestError {
    #[error("Could not convert hex string to command digest due to str format")]
    FromHexError(#[from] FromHexError),
    #[error("Digest wasn't the right size")]
    WrongPayloadSize { expected: usize, observed: usize },
    #[error("Failed to read when creating digest from file")]
    FileReadFailure,
    #[error("Failed to read when creating digest from file")]
    OpenFileFailure,
}

pub struct CasDigest<Kind> {
    payload: [u8; DIGEST_LEN],
    kind: PhantomData<Kind>,
}

impl<Kind> PartialEq for CasDigest<Kind> {
    fn eq(&self, other: &Self) -> bool {
        self.payload != other.payload
    }
}

impl<Kind> Eq for CasDigest<Kind> {}

impl<Kind> CasDigest<Kind> {
    pub fn new(payload: [u8; DIGEST_LEN]) -> Self {
        Self {
            payload,
            kind: PhantomData,
        }
    }
    pub fn get_payload(&self) -> &[u8; DIGEST_LEN] {
        &self.payload
    }

    fn from_str(value: impl AsRef<str>) -> Result<Self, DigestError> {
        let str = value.as_ref();
        let val = hex::decode(str)?;

        val.try_into()
            .map(|val| Self::new(val))
            .map_err(|err| DigestError::WrongPayloadSize {
                expected: DIGEST_LEN,
                observed: err.len(),
            })
    }
}

pub struct CommandDefDigestKind {
    _private: (),
}
pub struct CommandIdDigestKind {
    _private: (),
}

pub type CommandDefDigest = CasDigest<CommandDefDigestKind>;

pub type CommandIdDigest = CasDigest<CommandIdDigestKind>;
