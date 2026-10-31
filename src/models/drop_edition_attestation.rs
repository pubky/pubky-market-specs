//! The drop edition attestation: a durable, offline-verifiable compact JWS
//! (RFC 7515, `alg: EdDSA` per RFC 8037) embedded in a private order receipt
//! record's `editionAttestation` field. It attests that one order bought
//! edition `edition` out of `of` total units of the seller's drop.
//!
//! This module is the normative reference for third-party verifiers. The
//! verification recipe:
//!
//! 1. Parse the compact serialization (`header.payload.signature`, all
//!    base64url without padding) with [`PubkyMarketDropEditionAttestation::parse`].
//!    Parsing is closed-world: the header must be exactly
//!    `{"alg":"EdDSA","typ":"pubky-drop-edition+v1"}` and the claim set must
//!    be exactly the `v: 1` claims — unknown claims are rejected.
//! 2. Verify the Ed25519 signature with
//!    [`PubkyMarketDropEditionAttestation::verify_signature`]. The issuer key
//!    needs no discovery: the `iss` claim is a pubky, i.e. the z-base-32
//!    encoding of the Ed25519 verification key itself.
//! 3. Check the claim bindings against the order receipt record with
//!    [`PubkyMarketDropEditionAttestation::verify_binding`]: `receipt`, `buyer`,
//!    and `seller` must equal the record's receipt id and parties, and
//!    `drop`/`edition`/`of` must equal the record's optional drop display
//!    object when the record carries it.
//! 4. Accept as *verified* only when `iss` is on the verifier's own attestor
//!    trust list. The signature proves key possession, never legitimacy —
//!    trust in `iss` remains the caller's policy decision.
//!
//! Issuance is deterministic per receipt, following the same doctrine as the
//! receipt attestation: the claims serialize in a fixed field order, `iat`
//! is derived from the receipt (the epoch seconds of its payment instant),
//! never from the signing wall clock, and Ed25519 signing is deterministic —
//! so a given receipt always yields the same compact JWS, byte for byte.

use crate::{
    models::marketplace::{validate_entity_id, validate_pubky, validate_uuid, MAX_SAFE_INTEGER},
    models::marketplace_attestation::{attestation_error, base64url_decode, zbase32_decode_pubky},
    models::order_receipt::PubkyMarketOrderReceipt,
    traits::Validatable,
};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

/// The exact JOSE `typ` value of a v1 drop edition attestation.
pub const DROP_EDITION_ATTESTATION_TYP: &str = "pubky-drop-edition+v1";
/// The only claim-set version this module accepts.
pub const DROP_EDITION_ATTESTATION_VERSION: i64 = 1;

// The spec field's own bounds (mirrors order_receipt.rs).
const MIN_ATTESTATION_LENGTH: usize = 32;
const MAX_ATTESTATION_LENGTH: usize = 4_096;

/// The protected JOSE header of a drop edition attestation. Closed-world:
/// both fields are mandatory and no other field is tolerated.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct AttestationHeader {
    alg: String,
    typ: String,
}

/// The `v: 1` claim set. Closed-world (`deny_unknown_fields`): verifiers
/// reject attestations carrying claims this version does not define.
///
/// The serde field order below is normative for issuers: claims serialize
/// in exactly this order, and together with Ed25519's deterministic
/// signatures that makes issuance deterministic — a given receipt always
/// yields the same compact JWS.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketDropEditionAttestationClaims {
    /// Claim-set version; always `1`.
    pub v: i64,
    /// Attestor pubky (z-base-32 of the Ed25519 verification key).
    pub iss: String,
    /// Buyer pubky; must equal the receipt record's `buyerPubky`.
    pub buyer: String,
    /// Seller pubky (the drop owner); must equal the receipt record's
    /// `sellerPubky`.
    pub seller: String,
    /// The drop's entity id; must equal the record's `drop.dropId`.
    pub drop: String,
    /// This order's edition number, 1-based; must equal the record's
    /// `drop.edition`.
    pub edition: i64,
    /// The drop's `totalQuantity` at issuance; never below `edition`. Must
    /// equal the record's `drop.of`.
    pub of: i64,
    /// Receipt UUID (lowercase hyphenated); must equal the record's
    /// `receiptId`.
    pub receipt: String,
    /// Epoch seconds. Issuance is deterministic per receipt — same doctrine
    /// as the receipt attestation: `iat` is derived from the receipt's
    /// payment instant, never from the signing wall clock.
    pub iat: i64,
}

/// A parsed (structurally valid) drop edition attestation. Construction via
/// [`Self::parse`] guarantees the header and claims are well-formed; it does
/// NOT imply the signature was checked — call [`Self::verify_signature`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PubkyMarketDropEditionAttestation {
    pub claims: PubkyMarketDropEditionAttestationClaims,
    /// The exact bytes the signature covers: `header_b64 || '.' || payload_b64`.
    signing_input: String,
    signature: [u8; 64],
}

impl PubkyMarketDropEditionAttestation {
    /// Parses and structurally validates a compact JWS drop edition
    /// attestation. Rejects unknown versions, unknown claims, unknown header
    /// fields, and every claim-format violation. Does not verify the
    /// signature.
    pub fn parse(compact: &str) -> Result<Self, String> {
        let length = compact.chars().count();
        if !(MIN_ATTESTATION_LENGTH..=MAX_ATTESTATION_LENGTH).contains(&length) {
            return Err(format!(
                "Attestation Error: compact JWS must be {MIN_ATTESTATION_LENGTH}-{MAX_ATTESTATION_LENGTH} characters"
            ));
        }
        if !compact
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._~-".contains(c))
        {
            return Err(
                "Attestation Error: compact JWS must only contain characters [A-Za-z0-9._~-]"
                    .into(),
            );
        }

        let mut parts = compact.split('.');
        let (header_b64, payload_b64, signature_b64) =
            match (parts.next(), parts.next(), parts.next(), parts.next()) {
                (Some(header), Some(payload), Some(signature), None) => {
                    (header, payload, signature)
                }
                _ => {
                    return Err(
                        "Attestation Error: compact JWS must have exactly three segments".into(),
                    )
                }
            };

        let header_bytes = base64url_decode(header_b64)
            .ok_or("Attestation Error: header is not valid base64url")?;
        let header: AttestationHeader = serde_json::from_slice(&header_bytes)
            .map_err(|e| format!("Attestation Error: invalid header: {e}"))?;
        if header.alg != "EdDSA" {
            return Err("Attestation Error: alg must be EdDSA".into());
        }
        if header.typ != DROP_EDITION_ATTESTATION_TYP {
            return Err(format!(
                "Attestation Error: typ must be {DROP_EDITION_ATTESTATION_TYP}"
            ));
        }

        let payload_bytes = base64url_decode(payload_b64)
            .ok_or("Attestation Error: payload is not valid base64url")?;
        let claims: PubkyMarketDropEditionAttestationClaims =
            serde_json::from_slice(&payload_bytes)
                .map_err(|e| format!("Attestation Error: invalid claims: {e}"))?;
        validate_claims(&claims)?;

        let signature_bytes = base64url_decode(signature_b64)
            .ok_or("Attestation Error: signature is not valid base64url")?;
        let signature: [u8; 64] = signature_bytes
            .try_into()
            .map_err(|_| "Attestation Error: signature must be 64 bytes".to_string())?;

        Ok(Self {
            claims,
            signing_input: format!("{header_b64}.{payload_b64}"),
            signature,
        })
    }

    /// Verifies the Ed25519 signature against the issuer key carried in the
    /// `iss` claim (a pubky is the z-base-32 encoding of the verification
    /// key — no key server or issuer round-trip is involved).
    pub fn verify_signature(&self) -> Result<(), String> {
        let key_bytes = zbase32_decode_pubky(&self.claims.iss)
            .ok_or("Attestation Error: iss does not decode as a pubky")?;
        let verifying_key = VerifyingKey::from_bytes(&key_bytes)
            .map_err(|_| "Attestation Error: iss is not a valid Ed25519 key".to_string())?;
        let signature = Signature::from_bytes(&self.signature);
        verifying_key
            .verify_strict(self.signing_input.as_bytes(), &signature)
            .map_err(|_| "Attestation Error: signature verification failed".to_string())
    }

    /// Checks that the claims cover exactly this order receipt record:
    /// `receipt` equals the record's `receiptId`, `buyer` and `seller` equal
    /// the record's parties, and — when the record carries the optional
    /// `drop` display object — `drop`, `edition`, and `of` equal that
    /// object's fields. Any mismatch means the attestation does not cover
    /// the receipt.
    pub fn verify_binding(&self, record: &PubkyMarketOrderReceipt) -> Result<(), String> {
        if self.claims.receipt != record.receipt_id {
            return Err("Attestation Error: receipt does not match the receipt id".into());
        }
        if self.claims.buyer != record.buyer_pubky {
            return Err("Attestation Error: buyer does not match the receipt buyer".into());
        }
        if self.claims.seller != record.seller_pubky {
            return Err("Attestation Error: seller does not match the receipt seller".into());
        }
        if let Some(drop) = &record.drop {
            if self.claims.drop != drop.drop_id {
                return Err("Attestation Error: drop does not match the receipt drop".into());
            }
            if self.claims.edition != drop.edition {
                return Err(
                    "Attestation Error: edition does not match the receipt drop edition".into(),
                );
            }
            if self.claims.of != drop.of {
                return Err("Attestation Error: of does not match the receipt drop total".into());
            }
        }
        Ok(())
    }

    /// The full local verification recipe for one drop-order receipt
    /// record: requires the record's `editionAttestation` AND `drop` object
    /// to both be present, parses the embedded attestation, verifies the
    /// signature, and checks the bindings. Trust in `iss` remains the
    /// caller's policy decision.
    pub fn verify_edition_for_order_receipt(
        record: &PubkyMarketOrderReceipt,
    ) -> Result<Self, String> {
        let compact = record
            .edition_attestation
            .as_deref()
            .ok_or("Attestation Error: the receipt carries no editionAttestation")?;
        if record.drop.is_none() {
            return Err("Attestation Error: the receipt carries no drop object".into());
        }
        let attestation = Self::parse(compact)?;
        attestation.verify_signature()?;
        attestation.verify_binding(record)?;
        Ok(attestation)
    }
}

fn validate_claims(claims: &PubkyMarketDropEditionAttestationClaims) -> Result<(), String> {
    if claims.v != DROP_EDITION_ATTESTATION_VERSION {
        return Err(format!(
            "Attestation Error: v must be {DROP_EDITION_ATTESTATION_VERSION}"
        ));
    }
    validate_pubky(&claims.iss, "iss").map_err(attestation_error)?;
    validate_pubky(&claims.buyer, "buyer").map_err(attestation_error)?;
    validate_pubky(&claims.seller, "seller").map_err(attestation_error)?;
    validate_entity_id(&claims.drop, "drop").map_err(attestation_error)?;

    if !(1..=MAX_SAFE_INTEGER).contains(&claims.edition) {
        return Err("Attestation Error: edition must be a positive safe integer".into());
    }
    if claims.of < claims.edition {
        return Err("Attestation Error: of must not be below the edition number".into());
    }
    if !(1..=MAX_SAFE_INTEGER).contains(&claims.of) {
        return Err("Attestation Error: of must be a positive safe integer".into());
    }

    validate_uuid(&claims.receipt, "receipt").map_err(attestation_error)?;

    // iat is deterministic per receipt (derived from the receipt's payment
    // instant, same doctrine as the receipt attestation), never the signing
    // wall clock; structurally it must be a positive epoch-seconds value.
    if !(1..=MAX_SAFE_INTEGER).contains(&claims.iat) {
        return Err("Attestation Error: iat must be a positive safe integer".into());
    }
    Ok(())
}

impl Validatable for PubkyMarketDropEditionAttestationClaims {
    fn validate(&self, _id: Option<&str>) -> Result<(), String> {
        validate_claims(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::marketplace::PubkyMarketMoney;
    use crate::models::marketplace_attestation::base64url_encode;
    use crate::models::order_receipt::{PubkyMarketOrderReceiptDrop, PubkyMarketOrderReceiptRole};
    use base32::{encode as base32_encode, Alphabet};
    use ed25519_dalek::{Signer, SigningKey};

    const BUYER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
    const SELLER: &str = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";
    const RECEIPT_ID: &str = "a7fc7d5d-0b2a-4083-b278-47193f8fe536";
    const ORDER_ID: &str = "0e9c2c4a-91d6-4a4e-8db3-2f14c1e8b7aa";
    const DROP_ID: &str = "spring-drop-01";
    // 2026-01-02T03:04:05Z
    const PAID_AT: &str = "2026-01-02T03:04:05Z";
    const PAID_AT_EPOCH_SECONDS: i64 = 1_767_323_045;

    fn attestor_key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    fn attestor_pubky(key: &SigningKey) -> String {
        base32_encode(Alphabet::Z, key.verifying_key().as_bytes())
    }

    fn valid_claims(iss: &str) -> PubkyMarketDropEditionAttestationClaims {
        PubkyMarketDropEditionAttestationClaims {
            v: 1,
            iss: iss.to_string(),
            buyer: BUYER.to_string(),
            seller: SELLER.to_string(),
            drop: DROP_ID.to_string(),
            edition: 7,
            of: 500,
            receipt: RECEIPT_ID.to_string(),
            iat: PAID_AT_EPOCH_SECONDS,
        }
    }

    fn sign(claims: &PubkyMarketDropEditionAttestationClaims, key: &SigningKey) -> String {
        let header = serde_json::json!({ "alg": "EdDSA", "typ": DROP_EDITION_ATTESTATION_TYP });
        let header_b64 = base64url_encode(serde_json::to_vec(&header).unwrap().as_slice());
        let payload_b64 = base64url_encode(serde_json::to_vec(claims).unwrap().as_slice());
        let signing_input = format!("{header_b64}.{payload_b64}");
        let signature = key.sign(signing_input.as_bytes());
        format!(
            "{signing_input}.{}",
            base64url_encode(&signature.to_bytes())
        )
    }

    fn matching_receipt(edition_attestation: &str) -> PubkyMarketOrderReceipt {
        let mut receipt = PubkyMarketOrderReceipt::new(
            BUYER.to_string(),
            1,
            PAID_AT.to_string(),
            PAID_AT.to_string(),
            PubkyMarketOrderReceiptRole::Buyer,
            RECEIPT_ID.to_string(),
            ORDER_ID.to_string(),
            BUYER.to_string(),
            SELLER.to_string(),
            PubkyMarketMoney {
                amount_minor: 12_000,
                currency: "USD".to_string(),
                exponent: 2,
            },
            PAID_AT.to_string(),
            "a".repeat(64),
        );
        receipt.edition_attestation = Some(edition_attestation.to_string());
        receipt.drop = Some(PubkyMarketOrderReceiptDrop {
            drop_id: DROP_ID.to_string(),
            edition: 7,
            of: 500,
        });
        receipt
    }

    #[test]
    fn test_valid_attestation_full_recipe() {
        let key = attestor_key();
        let iss = attestor_pubky(&key);
        let claims = valid_claims(&iss);
        let jws = sign(&claims, &key);
        let receipt = matching_receipt(&jws);

        let attestation =
            PubkyMarketDropEditionAttestation::verify_edition_for_order_receipt(&receipt)
                .expect("recipe verifies");
        assert_eq!(attestation.claims, claims);
        // The record's own field validation also accepts the JWS charset.
        assert!(receipt.validate(Some(RECEIPT_ID)).is_ok());
    }

    #[test]
    fn test_recipe_requires_both_optional_fields() {
        let key = attestor_key();
        let jws = sign(&valid_claims(&attestor_pubky(&key)), &key);

        let mut no_attestation = matching_receipt(&jws);
        no_attestation.edition_attestation = None;
        assert!(
            PubkyMarketDropEditionAttestation::verify_edition_for_order_receipt(&no_attestation)
                .is_err()
        );

        let mut no_drop = matching_receipt(&jws);
        no_drop.drop = None;
        assert!(
            PubkyMarketDropEditionAttestation::verify_edition_for_order_receipt(&no_drop).is_err()
        );
    }

    #[test]
    fn test_deterministic_issuance() {
        // Fixed claim order + deterministic Ed25519: signing the same
        // receipt twice yields the identical compact JWS.
        let key = attestor_key();
        let claims = valid_claims(&attestor_pubky(&key));
        assert_eq!(sign(&claims, &key), sign(&claims, &key));
    }

    #[test]
    fn test_wrong_key_fails_signature() {
        let key = attestor_key();
        let other_key = SigningKey::from_bytes(&[9u8; 32]);
        // Claims name the honest attestor, but the signature is forged with
        // another key.
        let claims = valid_claims(&attestor_pubky(&key));
        let jws = sign(&claims, &other_key);
        let attestation =
            PubkyMarketDropEditionAttestation::parse(&jws).expect("structurally valid");
        assert!(attestation.verify_signature().is_err());
    }

    #[test]
    fn test_tampered_payload_fails_signature() {
        let key = attestor_key();
        let claims = valid_claims(&attestor_pubky(&key));
        let jws = sign(&claims, &key);

        // Re-sign nothing: swap in a payload claiming a rarer edition while
        // keeping the original signature bytes.
        let mut tampered_claims = claims.clone();
        tampered_claims.edition = 1;
        let parts: Vec<&str> = jws.split('.').collect();
        let tampered_payload =
            base64url_encode(serde_json::to_vec(&tampered_claims).unwrap().as_slice());
        let tampered = format!("{}.{}.{}", parts[0], tampered_payload, parts[2]);

        let attestation =
            PubkyMarketDropEditionAttestation::parse(&tampered).expect("structurally valid");
        assert!(attestation.verify_signature().is_err());
    }

    /// Signs the mutated claims and asserts they no longer bind to the
    /// otherwise-matching record, even though the signature stays honest.
    fn assert_binding_fails(claims: &PubkyMarketDropEditionAttestationClaims, key: &SigningKey) {
        let jws = sign(claims, key);
        let receipt = matching_receipt(&jws);
        let attestation =
            PubkyMarketDropEditionAttestation::parse(&jws).expect("structurally valid");
        attestation.verify_signature().expect("honest signature");
        assert!(
            attestation.verify_binding(&receipt).is_err(),
            "binding must fail for mutated claims: {claims:?}"
        );
    }

    #[test]
    fn test_mismatched_bindings_fail_one_by_one() {
        let key = attestor_key();
        let iss = attestor_pubky(&key);

        // buyer/seller swapped relative to the record.
        let mut claims = valid_claims(&iss);
        claims.buyer = SELLER.to_string();
        claims.seller = BUYER.to_string();
        assert_binding_fails(&claims, &key);

        let mut claims = valid_claims(&iss);
        claims.receipt = "b7fc7d5d-0b2a-4083-b278-47193f8fe536".to_string();
        assert_binding_fails(&claims, &key);

        let mut claims = valid_claims(&iss);
        claims.drop = "another-drop".to_string();
        assert_binding_fails(&claims, &key);

        let mut claims = valid_claims(&iss);
        claims.edition = 8;
        assert_binding_fails(&claims, &key);

        let mut claims = valid_claims(&iss);
        claims.of = 501;
        assert_binding_fails(&claims, &key);
    }

    #[test]
    fn test_unknown_version_rejected() {
        let key = attestor_key();
        let mut claims = valid_claims(&attestor_pubky(&key));
        claims.v = 2;
        let jws = sign(&claims, &key);
        assert!(PubkyMarketDropEditionAttestation::parse(&jws).is_err());
    }

    #[test]
    fn test_unknown_claim_rejected() {
        let key = attestor_key();
        let claims = valid_claims(&attestor_pubky(&key));
        let mut payload = serde_json::to_value(&claims).unwrap();
        payload["surprise"] = serde_json::json!(true);
        let header = serde_json::json!({ "alg": "EdDSA", "typ": DROP_EDITION_ATTESTATION_TYP });
        let header_b64 = base64url_encode(serde_json::to_vec(&header).unwrap().as_slice());
        let payload_b64 = base64url_encode(serde_json::to_vec(&payload).unwrap().as_slice());
        let signing_input = format!("{header_b64}.{payload_b64}");
        let signature = key.sign(signing_input.as_bytes());
        let jws = format!(
            "{signing_input}.{}",
            base64url_encode(&signature.to_bytes())
        );
        assert!(PubkyMarketDropEditionAttestation::parse(&jws).is_err());
    }

    #[test]
    fn test_wrong_header_rejected() {
        let key = attestor_key();
        let claims = valid_claims(&attestor_pubky(&key));
        for header in [
            serde_json::json!({ "alg": "ES256", "typ": DROP_EDITION_ATTESTATION_TYP }),
            serde_json::json!({ "alg": "EdDSA", "typ": "jwt" }),
            // The receipt attestation's typ is not this typ.
            serde_json::json!({ "alg": "EdDSA", "typ": "pubky-order-receipt+v1" }),
            serde_json::json!({ "alg": "EdDSA", "typ": DROP_EDITION_ATTESTATION_TYP, "kid": "1" }),
        ] {
            let header_b64 = base64url_encode(serde_json::to_vec(&header).unwrap().as_slice());
            let payload_b64 = base64url_encode(serde_json::to_vec(&claims).unwrap().as_slice());
            let signing_input = format!("{header_b64}.{payload_b64}");
            let signature = key.sign(signing_input.as_bytes());
            let jws = format!(
                "{signing_input}.{}",
                base64url_encode(&signature.to_bytes())
            );
            assert!(PubkyMarketDropEditionAttestation::parse(&jws).is_err());
        }
    }

    #[test]
    fn test_claim_format_violations_rejected() {
        let key = attestor_key();
        let iss = attestor_pubky(&key);

        let mut bad_iss = valid_claims(&iss);
        bad_iss.iss = "not-a-pubky".to_string();
        assert!(validate_claims(&bad_iss).is_err());

        let mut bad_drop = valid_claims(&iss);
        bad_drop.drop = "has space".to_string();
        assert!(validate_claims(&bad_drop).is_err());

        let mut bad_receipt = valid_claims(&iss);
        bad_receipt.receipt = "a7fc7d5d0b2a4083b27847193f8fe536".to_string();
        assert!(validate_claims(&bad_receipt).is_err());

        let mut zero_edition = valid_claims(&iss);
        zero_edition.edition = 0;
        assert!(validate_claims(&zero_edition).is_err());

        // of below edition contradicts "edition out of of".
        let mut small_of = valid_claims(&iss);
        small_of.edition = 5;
        small_of.of = 4;
        assert!(validate_claims(&small_of).is_err());

        let mut zero_iat = valid_claims(&iss);
        zero_iat.iat = 0;
        assert!(validate_claims(&zero_iat).is_err());

        assert!(validate_claims(&valid_claims(&iss)).is_ok());
    }

    #[test]
    fn test_malformed_compact_forms_rejected() {
        assert!(PubkyMarketDropEditionAttestation::parse("").is_err());
        assert!(PubkyMarketDropEditionAttestation::parse(&"a".repeat(64)).is_err());
        assert!(PubkyMarketDropEditionAttestation::parse(&format!(
            "{}.{}",
            "a".repeat(32),
            "b".repeat(32)
        ))
        .is_err());
        assert!(PubkyMarketDropEditionAttestation::parse(&format!(
            "{}.{}.{}.{}",
            "a".repeat(16),
            "b".repeat(16),
            "c".repeat(16),
            "d".repeat(16)
        ))
        .is_err());
        // Padding is not tolerated in unpadded base64url.
        assert!(PubkyMarketDropEditionAttestation::parse(&format!(
            "{}=.{}.{}",
            "a".repeat(15),
            "b".repeat(16),
            "c".repeat(86)
        ))
        .is_err());
    }

    #[test]
    fn test_size_within_spec_field_bounds() {
        let key = attestor_key();
        let claims = valid_claims(&attestor_pubky(&key));
        let jws = sign(&claims, &key);
        let length = jws.chars().count();
        assert!(
            (MIN_ATTESTATION_LENGTH..=MAX_ATTESTATION_LENGTH).contains(&length),
            "JWS length {length} outside the spec field bounds"
        );
    }
}
