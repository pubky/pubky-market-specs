use base32::{decode, Alphabet};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "openapi")]
use utoipa::{PartialSchema, ToSchema};

/// A validated Pubky public key in its 52-character z-base-32 form.
///
/// Native targets accept exactly the keys `pubky::PublicKey` accepts for a
/// 52-character input (the decoded 32 bytes must be a valid Ed25519 point)
/// and store the canonical re-encoding, as `pubky-app-specs` does. The wasm
/// target checks length and encoding only, also as `pubky-app-specs` does.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PubkyId {
    z32: String,
}

#[cfg(feature = "openapi")]
impl PartialSchema for PubkyId {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        String::schema()
    }
}

#[cfg(feature = "openapi")]
impl ToSchema for PubkyId {}

impl PubkyId {
    fn validate(s: &str) -> Result<Vec<u8>, String> {
        if s.len() != 52 {
            return Err("Validation Error: the string is not 52 utf chars".to_string());
        }

        decode(Alphabet::Z, s)
            .ok_or_else(|| "Validation Error: invalid public key encoding".to_string())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn try_from(s: &str) -> Result<Self, String> {
        Self::validate(s)?;
        Ok(Self { z32: s.to_string() })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn try_from(s: &str) -> Result<Self, String> {
        let bytes = Self::validate(s)?;
        let bytes: [u8; 32] = bytes.as_slice().try_into().map_err(|_| {
            "Validation Error: Invalid public key length, expected 32 bytes".to_string()
        })?;
        Self::from_bytes(&bytes)
    }

    /// Builds the id from the 32 raw public key bytes, which must be a valid
    /// Ed25519 point.
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self, String> {
        let key = ed25519_dalek::VerifyingKey::from_bytes(bytes).map_err(|_| {
            "Validation Error: Invalid Ed25519 public key: not a valid point on the curve"
                .to_string()
        })?;
        Ok(Self {
            z32: base32::encode(Alphabet::Z, key.as_bytes()),
        })
    }
}

impl fmt::Display for PubkyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.z32)
    }
}

impl std::ops::Deref for PubkyId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.z32
    }
}

impl AsRef<str> for PubkyId {
    fn as_ref(&self) -> &str {
        &self.z32
    }
}

impl Serialize for PubkyId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.z32)
    }
}

impl<'de> Deserialize<'de> for PubkyId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        PubkyId::try_from(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_KEY: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";

    #[test]
    fn test_try_from_valid() {
        let result = PubkyId::try_from(VALID_KEY);
        assert_eq!(result.unwrap().as_ref(), VALID_KEY);
    }

    #[test]
    fn test_try_from_invalid_length() {
        assert_eq!(
            PubkyId::try_from("short").unwrap_err(),
            "Validation Error: the string is not 52 utf chars"
        );
    }

    #[test]
    fn test_try_from_invalid_encoding() {
        // 52 characters but invalid z-base-32 (contains invalid char '0')
        let invalid_key = "0perrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rd0";
        assert_eq!(
            PubkyId::try_from(invalid_key).unwrap_err(),
            "Validation Error: invalid public key encoding"
        );
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn test_from_bytes_matches_signing_key() {
        let signing = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
        let id = PubkyId::from_bytes(signing.verifying_key().as_bytes()).unwrap();
        assert_eq!(id.as_ref().len(), 52);
        assert_eq!(PubkyId::try_from(id.as_ref()).unwrap(), id);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let pubky_id = PubkyId::try_from(VALID_KEY).unwrap();
        let json = serde_json::to_string(&pubky_id).unwrap();
        assert_eq!(json, format!("\"{VALID_KEY}\""));
        let back: PubkyId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, pubky_id);
    }

    #[test]
    fn test_deserialization_rejects_invalid() {
        assert!(serde_json::from_str::<PubkyId>("\"short\"").is_err());
        assert!(serde_json::from_str::<PubkyId>(
            "\"0perrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rd0\""
        )
        .is_err());
    }
}
