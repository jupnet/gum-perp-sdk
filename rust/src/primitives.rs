use {
    serde::{Deserialize, Deserializer, Serialize, Serializer},
    std::{fmt, str::FromStr},
    thiserror::Error,
};

const BASE58_MAX_32: usize = 44;
const BASE58_MAX_64: usize = 88;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ParseBytes32Error {
    #[error("base58 string decoded to the wrong size")]
    WrongSize,
    #[error("invalid base58 string: {0}")]
    Invalid(String),
}

#[derive(Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Pubkey([u8; 32]);

impl Pubkey {
    pub const fn from_bytes_const(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn new_unique() -> Self {
        Self(rand::random())
    }

    pub fn to_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl From<[u8; 32]> for Pubkey {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl AsRef<[u8]> for Pubkey {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Display for Pubkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_base58_32(f, &self.0)
    }
}

impl fmt::Debug for Pubkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl FromStr for Pubkey {
    type Err = ParseBytes32Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        decode_base58_32(s).map(Self)
    }
}

impl Serialize for Pubkey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Pubkey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Hash([u8; 32]);

impl Hash {
    pub const fn from_bytes_const(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn to_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl From<[u8; 32]> for Hash {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl AsRef<[u8]> for Hash {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_base58_32(f, &self.0)
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl FromStr for Hash {
    type Err = ParseBytes32Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        decode_base58_32(s).map(Self)
    }
}

impl Serialize for Hash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Hash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ParseSignatureError {
    #[error("invalid signature type")]
    InvalidType,
    #[error("base58 signature decoded to the wrong size")]
    WrongSize,
    #[error("invalid base58 signature: {0}")]
    Invalid(String),
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct TypedSignature {
    signature_type: u8,
    signature: Vec<u8>,
}

impl TypedSignature {
    pub fn new(
        signature_type: u8,
        signature: impl Into<Vec<u8>>,
    ) -> Result<Self, ParseSignatureError> {
        let signature = signature.into();
        if signature_type == 0 && signature.len() != 64 {
            return Err(ParseSignatureError::WrongSize);
        }
        Ok(Self {
            signature_type,
            signature,
        })
    }

    pub fn from_ed25519(signature: [u8; 64]) -> Self {
        Self {
            signature_type: 0,
            signature: signature.to_vec(),
        }
    }

    pub fn signature_type(&self) -> u8 {
        self.signature_type
    }

    pub fn signature_bytes(&self) -> &[u8] {
        &self.signature
    }
}

impl Default for TypedSignature {
    fn default() -> Self {
        Self::from_ed25519([0; 64])
    }
}

impl fmt::Display for TypedSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}",
            self.signature_type,
            bs58::encode(&self.signature).into_string()
        )
    }
}

impl fmt::Debug for TypedSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl FromStr for TypedSignature {
    type Err = ParseSignatureError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((ty, sig)) = s.split_once(':') {
            let signature_type = ty
                .parse::<u8>()
                .map_err(|_| ParseSignatureError::InvalidType)?;
            let mut signature = [0_u8; 64];
            let n = bs58::decode(sig)
                .onto(&mut signature)
                .map_err(|err| ParseSignatureError::Invalid(err.to_string()))?;
            if signature_type == 0 && n != 64 {
                return Err(ParseSignatureError::WrongSize);
            }
            return Ok(Self {
                signature_type,
                signature: signature[..n].to_vec(),
            });
        }

        if s.len() > BASE58_MAX_64 {
            return Err(ParseSignatureError::WrongSize);
        }
        let mut signature = [0_u8; 64];
        let n = bs58::decode(s)
            .onto(&mut signature)
            .map_err(|err| ParseSignatureError::Invalid(err.to_string()))?;
        if n != 64 {
            return Err(ParseSignatureError::WrongSize);
        }
        Ok(Self::from_ed25519(signature))
    }
}

impl Serialize for TypedSignature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for TypedSignature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Error)]
pub enum SignerError {
    #[error("signer pubkey unavailable: {0}")]
    Pubkey(String),
    #[error("signing failed: {0}")]
    Sign(String),
}

pub trait Signer {
    fn try_pubkey(&self) -> Result<Pubkey, SignerError>;
    fn try_sign_message(&self, message: &[u8]) -> Result<TypedSignature, SignerError>;

    fn pubkey(&self) -> Pubkey {
        self.try_pubkey().unwrap_or_default()
    }

    fn sign_message(&self, message: &[u8]) -> TypedSignature {
        self.try_sign_message(message).unwrap_or_default()
    }
}

impl<T: Signer + ?Sized> Signer for &T {
    fn try_pubkey(&self) -> Result<Pubkey, SignerError> {
        (**self).try_pubkey()
    }

    fn try_sign_message(&self, message: &[u8]) -> Result<TypedSignature, SignerError> {
        (**self).try_sign_message(message)
    }
}

#[cfg(feature = "ed25519")]
pub mod keypair {
    use super::{Pubkey, Signer, SignerError, TypedSignature};
    use ed25519_dalek::{Signer as DalekSigner, SigningKey};
    use rand_core::OsRng;

    #[derive(Debug)]
    pub struct Keypair(SigningKey);

    impl Keypair {
        pub fn new() -> Self {
            Self(SigningKey::generate(&mut OsRng))
        }

        pub fn from_bytes(bytes: &[u8]) -> Result<Self, SignerError> {
            let secret: [u8; 32] = if bytes.len() == 32 {
                bytes
                    .try_into()
                    .map_err(|_| SignerError::Pubkey("invalid secret key".to_string()))?
            } else if bytes.len() == 64 {
                bytes[..32]
                    .try_into()
                    .map_err(|_| SignerError::Pubkey("invalid keypair".to_string()))?
            } else {
                return Err(SignerError::Pubkey(
                    "expected 32-byte secret or 64-byte keypair".to_string(),
                ));
            };
            Ok(Self(SigningKey::from_bytes(&secret)))
        }

        pub fn to_bytes(&self) -> [u8; 64] {
            let mut out = [0_u8; 64];
            out[..32].copy_from_slice(&self.0.to_bytes());
            out[32..].copy_from_slice(&self.0.verifying_key().to_bytes());
            out
        }

        pub fn pubkey(&self) -> Pubkey {
            Pubkey::from(self.0.verifying_key().to_bytes())
        }
    }

    impl Default for Keypair {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Signer for Keypair {
        fn try_pubkey(&self) -> Result<Pubkey, SignerError> {
            Ok(self.pubkey())
        }

        fn try_sign_message(&self, message: &[u8]) -> Result<TypedSignature, SignerError> {
            let sig = self.0.sign(message);
            Ok(TypedSignature::from_ed25519(sig.to_bytes()))
        }
    }
}

fn write_base58_32(f: &mut fmt::Formatter<'_>, bytes: &[u8; 32]) -> fmt::Result {
    let mut out = [0_u8; BASE58_MAX_32];
    let len = bs58::encode(bytes)
        .onto(&mut out[..])
        .map_err(|_| fmt::Error)?;
    let s = std::str::from_utf8(&out[..len]).map_err(|_| fmt::Error)?;
    f.write_str(s)
}

fn decode_base58_32(s: &str) -> Result<[u8; 32], ParseBytes32Error> {
    if s.len() > BASE58_MAX_32 {
        return Err(ParseBytes32Error::WrongSize);
    }
    let mut bytes = [0_u8; 32];
    let n = bs58::decode(s)
        .onto(&mut bytes)
        .map_err(|err| ParseBytes32Error::Invalid(err.to_string()))?;
    if n != 32 {
        return Err(ParseBytes32Error::WrongSize);
    }
    Ok(bytes)
}
