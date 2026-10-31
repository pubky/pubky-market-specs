//! The order receipt attestation: a durable, offline-verifiable compact JWS
//! (RFC 7515, `alg: EdDSA` per RFC 8037) embedded in a private order receipt
//! record's `receiptAttestation` field.
//!
//! This module is the normative reference for third-party verifiers. The
//! verification recipe:
//!
//! 1. Parse the compact serialization (`header.payload.signature`, all
//!    base64url without padding) with [`PubkyMarketOrderReceiptAttestation::parse`].
//!    Parsing is closed-world: the header must be exactly
//!    `{"alg":"EdDSA","typ":"pubky-order-receipt+v1"}` and the claim set must
//!    be exactly the `v: 1` claims — unknown claims are rejected.
//! 2. Verify the Ed25519 signature with
//!    [`PubkyMarketOrderReceiptAttestation::verify_signature`]. The issuer key
//!    needs no discovery: the `iss` claim is a pubky, i.e. the z-base-32
//!    encoding of the Ed25519 verification key itself.
//! 3. Check the claim bindings against the receipt record with
//!    [`PubkyMarketOrderReceiptAttestation::verify_binding`]: `buyer`, `seller`,
//!    `order`, and `receipt` must equal the record's parties and ids;
//!    v1 `total_minor`/`currency`/`exponent` its `total`; v2
//!    `merchandise_total` its `total`; `paid_at` its `paidAt`.
//! 4. Accept as *verified* only when `iss` is on the verifier's own attestor
//!    trust list. The signature proves key possession, never legitimacy —
//!    trust in `iss` remains the caller's policy decision.
//!
//! Unlike the PUBLIC purchase attestation (which redacts the order behind an
//! attestor-salted `order_ref` hash because it travels inside a
//! world-readable review), the `order` claim here is the RAW order UUID:
//! receipts are private documents under `/priv/` that only the trade parties
//! hold, so there is no third-party observer to protect the linkage from —
//! and the raw id is exactly what makes the receipt actionable against the
//! service (disputes, exports, audits) after the operator disappears.
//!
//! Issuance is deterministic per receipt: the claims serialize in a fixed
//! field order, `paid_at` is the exact UTC instant of payment confirmation /
//! receipt creation, `iat` is the epoch seconds of that same instant (not
//! "now" at signing time), and Ed25519 signing is deterministic — so a given
//! receipt always yields the same compact JWS, byte for byte.

use crate::{
    models::marketplace::{parse_rfc3339_millis, validate_pubky, validate_uuid, PubkyMarketMoney},
    models::marketplace_attestation::{attestation_error, base64url_decode, zbase32_decode_pubky},
    models::order_receipt::PubkyMarketOrderReceipt,
    traits::Validatable,
};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

/// The exact JOSE `typ` value of a v1 order receipt attestation.
pub const ORDER_RECEIPT_ATTESTATION_TYP: &str = "pubky-order-receipt+v1";
pub const ORDER_RECEIPT_ATTESTATION_V2_TYP: &str = "pubky-order-receipt+v2";
/// The only v1 claim-set version this module accepts.
pub const ORDER_RECEIPT_ATTESTATION_VERSION: i64 = 1;
pub const ORDER_RECEIPT_ATTESTATION_V2_VERSION: i64 = 2;

// The spec field's own bounds (mirrors order_receipt.rs).
const MIN_ATTESTATION_LENGTH: usize = 32;
const MAX_ATTESTATION_LENGTH: usize = 4_096;

/// The protected JOSE header of an order receipt attestation. Closed-world:
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
///
/// `order` is the raw order UUID (not a salted ref like the public purchase
/// attestation's `order_ref`): receipts are private documents, so the
/// linkage needs no redaction and the raw id keeps the receipt actionable
/// after the service operator disappears.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketOrderReceiptAttestationClaims {
    /// Claim-set version; always `1`.
    pub v: i64,
    /// Attestor pubky (z-base-32 of the Ed25519 verification key).
    pub iss: String,
    /// Buyer pubky; must equal the receipt record's `buyerPubky`.
    pub buyer: String,
    /// Seller pubky; must equal the receipt record's `sellerPubky`.
    pub seller: String,
    /// Raw order UUID (lowercase hyphenated); must equal the record's
    /// `orderId`.
    pub order: String,
    /// Receipt UUID (lowercase hyphenated); must equal the record's
    /// `receiptId`.
    pub receipt: String,
    /// Order total in integer minor units; must equal the record total's
    /// `amountMinor`. Positive.
    pub total_minor: i64,
    /// Uppercase asset code; must equal the record total's `currency`.
    pub currency: String,
    /// Decimal places between minor and major units; must equal the record
    /// total's `exponent`.
    pub exponent: i64,
    /// ISO-8601 UTC datetime (`Z` offset) — the exact instant of payment
    /// confirmation / receipt creation. Must equal the record's `paidAt`.
    pub paid_at: String,
    /// Epoch seconds of the `paid_at` instant. Issuance is deterministic
    /// per receipt, so `iat` is derived from the receipt, never from the
    /// signing wall clock.
    pub iat: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketOrderReceiptAttestationV2Claims {
    pub v: i64,
    pub iss: String,
    pub buyer: String,
    pub seller: String,
    pub order: String,
    pub receipt: String,
    pub settlement_total: PubkyMarketMoneyObject,
    pub merchandise_total: PubkyMarketMoneyObject,
    pub paid_at: String,
    pub iat: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketMoneyObject {
    pub amount_minor: u64,
    pub currency: String,
    pub exponent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Claims {
    V1(PubkyMarketOrderReceiptAttestationClaims),
    V2(PubkyMarketOrderReceiptAttestationV2Claims),
}

/// A parsed (structurally valid) order receipt attestation. Construction via
/// [`Self::parse`] guarantees the header and claims are well-formed; it does
/// NOT imply the signature was checked — call [`Self::verify_signature`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PubkyMarketOrderReceiptAttestation {
    claims: Claims,
    /// The exact bytes the signature covers: `header_b64 || '.' || payload_b64`.
    signing_input: String,
    signature: [u8; 64],
}

impl PubkyMarketOrderReceiptAttestation {
    /// Parses and structurally validates a compact JWS order receipt
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
        if header.typ != ORDER_RECEIPT_ATTESTATION_TYP
            && header.typ != ORDER_RECEIPT_ATTESTATION_V2_TYP
        {
            return Err(format!(
                "Attestation Error: typ must be {ORDER_RECEIPT_ATTESTATION_TYP} or {ORDER_RECEIPT_ATTESTATION_V2_TYP}"
            ));
        }

        let payload_bytes = base64url_decode(payload_b64)
            .ok_or("Attestation Error: payload is not valid base64url")?;
        let claims = if header.typ == ORDER_RECEIPT_ATTESTATION_TYP {
            let claims: PubkyMarketOrderReceiptAttestationClaims =
                serde_json::from_slice(&payload_bytes)
                    .map_err(|e| format!("Attestation Error: invalid claims: {e}"))?;
            validate_claims(&claims)?;
            if claims.v != ORDER_RECEIPT_ATTESTATION_VERSION {
                return Err("Attestation Error: typ/v mismatch".into());
            }
            Claims::V1(claims)
        } else {
            let claims: PubkyMarketOrderReceiptAttestationV2Claims =
                serde_json::from_slice(&payload_bytes)
                    .map_err(|e| format!("Attestation Error: invalid v2 claims: {e}"))?;
            validate_v2_claims(&claims)?;
            if claims.v != ORDER_RECEIPT_ATTESTATION_V2_VERSION {
                return Err("Attestation Error: typ/v mismatch".into());
            }
            Claims::V2(claims)
        };

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

    pub fn v1_claims(&self) -> Option<&PubkyMarketOrderReceiptAttestationClaims> {
        match &self.claims {
            Claims::V1(claims) => Some(claims),
            Claims::V2(_) => None,
        }
    }

    pub fn v2_claims(&self) -> Option<&PubkyMarketOrderReceiptAttestationV2Claims> {
        match &self.claims {
            Claims::V1(_) => None,
            Claims::V2(claims) => Some(claims),
        }
    }

    /// Verifies the Ed25519 signature against the issuer key carried in the
    /// `iss` claim (a pubky is the z-base-32 encoding of the verification
    /// key — no key server or issuer round-trip is involved).
    pub fn verify_signature(&self) -> Result<(), String> {
        let issuer = match &self.claims {
            Claims::V1(claims) => &claims.iss,
            Claims::V2(claims) => &claims.iss,
        };
        let key_bytes = zbase32_decode_pubky(issuer)
            .ok_or("Attestation Error: iss does not decode as a pubky")?;
        let verifying_key = VerifyingKey::from_bytes(&key_bytes)
            .map_err(|_| "Attestation Error: iss is not a valid Ed25519 key".to_string())?;
        let signature = Signature::from_bytes(&self.signature);
        verifying_key
            .verify_strict(self.signing_input.as_bytes(), &signature)
            .map_err(|_| "Attestation Error: signature verification failed".to_string())
    }

    /// Checks that the claims cover exactly this receipt record: `buyer`,
    /// `seller`, `order`, and `receipt` equal the record's parties and ids;
    /// v1 `total_minor`, `currency`, and `exponent`, or v2
    /// `merchandise_total`, equal the record's `total`; `settlement_total` is
    /// signed but intentionally has no corresponding record field; `paid_at`
    /// equals the record's `paidAt`. Any mismatch means the attestation does
    /// not cover the receipt.
    pub fn verify_binding(&self, record: &PubkyMarketOrderReceipt) -> Result<(), String> {
        if let Claims::V2(claims) = &self.claims {
            if claims.buyer != record.buyer_pubky
                || claims.seller != record.seller_pubky
                || claims.order != record.order_id
                || claims.receipt != record.receipt_id
            {
                return Err(
                    "Attestation Error: v2 party or id binding does not match the receipt".into(),
                );
            }
            let record_amount = u64::try_from(record.total.amount_minor).map_err(|_| {
                "Attestation Error: record total amountMinor is not a valid unsigned amount"
            })?;
            let record_exponent = u8::try_from(record.total.exponent)
                .map_err(|_| "Attestation Error: record total exponent is not a valid u8")?;
            if claims.merchandise_total.amount_minor != record_amount
                || claims.merchandise_total.currency != record.total.currency
                || claims.merchandise_total.exponent != record_exponent
                || claims.paid_at != record.paid_at
            {
                return Err(
                    "Attestation Error: v2 merchandise total or paid_at does not match the receipt"
                        .into(),
                );
            }
            return Ok(());
        }
        let Claims::V1(claims) = &self.claims else {
            unreachable!("v2 claims returned from the v2 binding branch");
        };
        if claims.buyer != record.buyer_pubky {
            return Err("Attestation Error: buyer does not match the receipt buyer".into());
        }
        if claims.seller != record.seller_pubky {
            return Err("Attestation Error: seller does not match the receipt seller".into());
        }
        if claims.order != record.order_id {
            return Err("Attestation Error: order does not match the receipt order".into());
        }
        if claims.receipt != record.receipt_id {
            return Err("Attestation Error: receipt does not match the receipt id".into());
        }
        if claims.total_minor != record.total.amount_minor {
            return Err("Attestation Error: total_minor does not match the receipt total".into());
        }
        if claims.currency != record.total.currency {
            return Err("Attestation Error: currency does not match the receipt total".into());
        }
        if claims.exponent != record.total.exponent {
            return Err("Attestation Error: exponent does not match the receipt total".into());
        }
        if claims.paid_at != record.paid_at {
            return Err("Attestation Error: paid_at does not match the receipt paidAt".into());
        }
        Ok(())
    }

    /// The full local verification recipe for one receipt record: parse the
    /// record's embedded attestation, verify the signature, and check the
    /// bindings. Trust in `iss` remains the caller's policy decision.
    pub fn verify_for_order_receipt(record: &PubkyMarketOrderReceipt) -> Result<Self, String> {
        let attestation = Self::parse(&record.receipt_attestation)?;
        attestation.verify_signature()?;
        attestation.verify_binding(record)?;
        Ok(attestation)
    }
}

fn validate_claims(claims: &PubkyMarketOrderReceiptAttestationClaims) -> Result<(), String> {
    if claims.v != ORDER_RECEIPT_ATTESTATION_VERSION {
        return Err(format!(
            "Attestation Error: v must be {ORDER_RECEIPT_ATTESTATION_VERSION}"
        ));
    }
    validate_pubky(&claims.iss, "iss").map_err(attestation_error)?;
    validate_pubky(&claims.buyer, "buyer").map_err(attestation_error)?;
    validate_pubky(&claims.seller, "seller").map_err(attestation_error)?;
    validate_uuid(&claims.order, "order").map_err(attestation_error)?;
    validate_uuid(&claims.receipt, "receipt").map_err(attestation_error)?;

    // total_minor/currency/exponent follow exactly the PubkyMarketMoney rules.
    let total = PubkyMarketMoney {
        amount_minor: claims.total_minor,
        currency: claims.currency.clone(),
        exponent: claims.exponent,
    };
    total
        .validate_positive("total")
        .map_err(attestation_error)?;

    // paid_at is the exact UTC instant of payment confirmation / receipt
    // creation. UTC (`Z`) is required — one canonical serialization per
    // instant is what keeps issuance deterministic.
    let paid_at_millis = parse_rfc3339_millis(&claims.paid_at)
        .ok()
        .filter(|_| claims.paid_at.ends_with('Z'))
        .ok_or("Attestation Error: paid_at must be an ISO-8601 UTC datetime (Z offset)")?;

    // iat is the epoch seconds of the same instant, never the signing wall
    // clock: issuance is deterministic per receipt.
    if claims.iat != paid_at_millis.div_euclid(1_000) {
        return Err(
            "Attestation Error: iat must be the epoch seconds of the paid_at instant".into(),
        );
    }
    Ok(())
}

fn validate_v2_claims(claims: &PubkyMarketOrderReceiptAttestationV2Claims) -> Result<(), String> {
    if claims.v != ORDER_RECEIPT_ATTESTATION_V2_VERSION {
        return Err("Attestation Error: v must be 2".into());
    }
    validate_pubky(&claims.iss, "iss").map_err(attestation_error)?;
    validate_pubky(&claims.buyer, "buyer").map_err(attestation_error)?;
    validate_pubky(&claims.seller, "seller").map_err(attestation_error)?;
    validate_uuid(&claims.order, "order").map_err(attestation_error)?;
    validate_uuid(&claims.receipt, "receipt").map_err(attestation_error)?;
    for (name, money) in [
        ("settlement_total", &claims.settlement_total),
        ("merchandise_total", &claims.merchandise_total),
    ] {
        let amount_minor = i64::try_from(money.amount_minor)
            .map_err(|_| format!("Attestation Error: {name}.amount_minor is too large"))?;
        PubkyMarketMoney {
            amount_minor,
            currency: money.currency.clone(),
            exponent: i64::from(money.exponent),
        }
        .validate_positive(name)
        .map_err(attestation_error)?;
    }
    let paid_at_millis = parse_rfc3339_millis(&claims.paid_at)
        .ok()
        .filter(|_| claims.paid_at.ends_with('Z'))
        .ok_or("Attestation Error: paid_at must be an ISO-8601 UTC datetime (Z offset)")?;
    if claims.iat != paid_at_millis.div_euclid(1_000) {
        return Err(
            "Attestation Error: iat must be the epoch seconds of the paid_at instant".into(),
        );
    }
    Ok(())
}

impl Validatable for PubkyMarketOrderReceiptAttestationClaims {
    fn validate(&self, _id: Option<&str>) -> Result<(), String> {
        validate_claims(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::marketplace_attestation::base64url_encode;
    use crate::models::order_receipt::PubkyMarketOrderReceiptRole;
    use base32::{encode as base32_encode, Alphabet};
    use ed25519_dalek::{Signer, SigningKey};

    const BUYER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
    const SELLER: &str = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";
    const RECEIPT_ID: &str = "a7fc7d5d-0b2a-4083-b278-47193f8fe536";
    const ORDER_ID: &str = "0e9c2c4a-91d6-4a4e-8db3-2f14c1e8b7aa";
    // 2026-01-02T03:04:05Z
    const PAID_AT: &str = "2026-01-02T03:04:05Z";
    const PAID_AT_EPOCH_SECONDS: i64 = 1_767_323_045;

    fn attestor_key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    fn attestor_pubky(key: &SigningKey) -> String {
        base32_encode(Alphabet::Z, key.verifying_key().as_bytes())
    }

    fn valid_claims(iss: &str) -> PubkyMarketOrderReceiptAttestationClaims {
        PubkyMarketOrderReceiptAttestationClaims {
            v: 1,
            iss: iss.to_string(),
            buyer: BUYER.to_string(),
            seller: SELLER.to_string(),
            order: ORDER_ID.to_string(),
            receipt: RECEIPT_ID.to_string(),
            total_minor: 12_000,
            currency: "USD".to_string(),
            exponent: 2,
            paid_at: PAID_AT.to_string(),
            iat: PAID_AT_EPOCH_SECONDS,
        }
    }

    fn sign(claims: &PubkyMarketOrderReceiptAttestationClaims, key: &SigningKey) -> String {
        let header = serde_json::json!({ "alg": "EdDSA", "typ": ORDER_RECEIPT_ATTESTATION_TYP });
        let header_b64 = base64url_encode(serde_json::to_vec(&header).unwrap().as_slice());
        let payload_b64 = base64url_encode(serde_json::to_vec(claims).unwrap().as_slice());
        let signing_input = format!("{header_b64}.{payload_b64}");
        let signature = key.sign(signing_input.as_bytes());
        format!(
            "{signing_input}.{}",
            base64url_encode(&signature.to_bytes())
        )
    }

    fn matching_receipt(attestation: &str) -> PubkyMarketOrderReceipt {
        PubkyMarketOrderReceipt::new(
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
            attestation.to_string(),
        )
    }

    #[test]
    fn test_valid_attestation_full_recipe() {
        let key = attestor_key();
        let iss = attestor_pubky(&key);
        let claims = valid_claims(&iss);
        let jws = sign(&claims, &key);
        let receipt = matching_receipt(&jws);

        let attestation = PubkyMarketOrderReceiptAttestation::verify_for_order_receipt(&receipt)
            .expect("recipe verifies");
        assert_eq!(attestation.v1_claims(), Some(&claims));
        // The record's own field validation also accepts the JWS charset.
        assert!(receipt.validate(Some(RECEIPT_ID)).is_ok());
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
            PubkyMarketOrderReceiptAttestation::parse(&jws).expect("structurally valid");
        assert!(attestation.verify_signature().is_err());
    }

    #[test]
    fn test_tampered_payload_fails_signature() {
        let key = attestor_key();
        let claims = valid_claims(&attestor_pubky(&key));
        let jws = sign(&claims, &key);

        // Re-sign nothing: swap in a payload with a different total while
        // keeping the original signature bytes.
        let mut tampered_claims = claims.clone();
        tampered_claims.total_minor = 1;
        let parts: Vec<&str> = jws.split('.').collect();
        let tampered_payload =
            base64url_encode(serde_json::to_vec(&tampered_claims).unwrap().as_slice());
        let tampered = format!("{}.{}.{}", parts[0], tampered_payload, parts[2]);

        let attestation =
            PubkyMarketOrderReceiptAttestation::parse(&tampered).expect("structurally valid");
        assert!(attestation.verify_signature().is_err());
    }

    /// Signs the mutated claims and asserts they no longer bind to the
    /// otherwise-matching record, even though the signature stays honest.
    fn assert_binding_fails(claims: &PubkyMarketOrderReceiptAttestationClaims, key: &SigningKey) {
        let jws = sign(claims, key);
        let receipt = matching_receipt(&jws);
        let attestation =
            PubkyMarketOrderReceiptAttestation::parse(&jws).expect("structurally valid");
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
        claims.order = "1e9c2c4a-91d6-4a4e-8db3-2f14c1e8b7aa".to_string();
        assert_binding_fails(&claims, &key);

        let mut claims = valid_claims(&iss);
        claims.receipt = "b7fc7d5d-0b2a-4083-b278-47193f8fe536".to_string();
        assert_binding_fails(&claims, &key);

        let mut claims = valid_claims(&iss);
        claims.total_minor = 12_001;
        assert_binding_fails(&claims, &key);

        let mut claims = valid_claims(&iss);
        claims.currency = "EUR".to_string();
        assert_binding_fails(&claims, &key);

        let mut claims = valid_claims(&iss);
        claims.exponent = 3;
        assert_binding_fails(&claims, &key);

        let mut claims = valid_claims(&iss);
        claims.paid_at = "2026-01-02T03:04:06Z".to_string();
        claims.iat = PAID_AT_EPOCH_SECONDS + 1;
        assert_binding_fails(&claims, &key);
    }

    #[test]
    fn test_unknown_version_rejected() {
        let key = attestor_key();
        let mut claims = valid_claims(&attestor_pubky(&key));
        claims.v = 2;
        let jws = sign(&claims, &key);
        assert!(PubkyMarketOrderReceiptAttestation::parse(&jws).is_err());
    }

    #[test]
    fn test_unknown_claim_rejected() {
        let key = attestor_key();
        let claims = valid_claims(&attestor_pubky(&key));
        let mut payload = serde_json::to_value(&claims).unwrap();
        payload["surprise"] = serde_json::json!(true);
        let header = serde_json::json!({ "alg": "EdDSA", "typ": ORDER_RECEIPT_ATTESTATION_TYP });
        let header_b64 = base64url_encode(serde_json::to_vec(&header).unwrap().as_slice());
        let payload_b64 = base64url_encode(serde_json::to_vec(&payload).unwrap().as_slice());
        let signing_input = format!("{header_b64}.{payload_b64}");
        let signature = key.sign(signing_input.as_bytes());
        let jws = format!(
            "{signing_input}.{}",
            base64url_encode(&signature.to_bytes())
        );
        assert!(PubkyMarketOrderReceiptAttestation::parse(&jws).is_err());
    }

    #[test]
    fn test_wrong_header_rejected() {
        let key = attestor_key();
        let claims = valid_claims(&attestor_pubky(&key));
        for header in [
            serde_json::json!({ "alg": "ES256", "typ": ORDER_RECEIPT_ATTESTATION_TYP }),
            serde_json::json!({ "alg": "EdDSA", "typ": "jwt" }),
            // The public purchase attestation's typ is not this typ.
            serde_json::json!({ "alg": "EdDSA", "typ": "pubky-purchase-attestation+v1" }),
            serde_json::json!({ "alg": "EdDSA", "typ": ORDER_RECEIPT_ATTESTATION_TYP, "kid": "1" }),
        ] {
            let header_b64 = base64url_encode(serde_json::to_vec(&header).unwrap().as_slice());
            let payload_b64 = base64url_encode(serde_json::to_vec(&claims).unwrap().as_slice());
            let signing_input = format!("{header_b64}.{payload_b64}");
            let signature = key.sign(signing_input.as_bytes());
            let jws = format!(
                "{signing_input}.{}",
                base64url_encode(&signature.to_bytes())
            );
            assert!(PubkyMarketOrderReceiptAttestation::parse(&jws).is_err());
        }
    }

    #[test]
    fn test_claim_format_violations_rejected() {
        let key = attestor_key();
        let iss = attestor_pubky(&key);

        let mut bad_iss = valid_claims(&iss);
        bad_iss.iss = "not-a-pubky".to_string();
        assert!(validate_claims(&bad_iss).is_err());

        let mut bad_order = valid_claims(&iss);
        bad_order.order = "A7FC7D5D-0B2A-4083-B278-47193F8FE536".to_string();
        assert!(validate_claims(&bad_order).is_err());

        let mut bad_receipt = valid_claims(&iss);
        bad_receipt.receipt = "a7fc7d5d0b2a4083b27847193f8fe536".to_string();
        assert!(validate_claims(&bad_receipt).is_err());

        let mut zero_total = valid_claims(&iss);
        zero_total.total_minor = 0;
        assert!(validate_claims(&zero_total).is_err());

        let mut bad_currency = valid_claims(&iss);
        bad_currency.currency = "usd".to_string();
        assert!(validate_claims(&bad_currency).is_err());

        let mut bad_exponent = valid_claims(&iss);
        bad_exponent.exponent = 19;
        assert!(validate_claims(&bad_exponent).is_err());

        // Offset form is a valid instant but not the canonical UTC
        // serialization — determinism requires exactly one form.
        let mut offset_paid_at = valid_claims(&iss);
        offset_paid_at.paid_at = "2026-01-02T04:04:05+01:00".to_string();
        assert!(validate_claims(&offset_paid_at).is_err());

        let mut not_a_date = valid_claims(&iss);
        not_a_date.paid_at = "not a date".to_string();
        assert!(validate_claims(&not_a_date).is_err());

        // iat must be derived from paid_at, never the signing wall clock.
        let mut drifted_iat = valid_claims(&iss);
        drifted_iat.iat = PAID_AT_EPOCH_SECONDS + 1;
        assert!(validate_claims(&drifted_iat).is_err());

        assert!(validate_claims(&valid_claims(&iss)).is_ok());
    }

    #[test]
    fn test_malformed_compact_forms_rejected() {
        assert!(PubkyMarketOrderReceiptAttestation::parse("").is_err());
        assert!(PubkyMarketOrderReceiptAttestation::parse(&"a".repeat(64)).is_err());
        assert!(PubkyMarketOrderReceiptAttestation::parse(&format!(
            "{}.{}",
            "a".repeat(32),
            "b".repeat(32)
        ))
        .is_err());
        assert!(PubkyMarketOrderReceiptAttestation::parse(&format!(
            "{}.{}.{}.{}",
            "a".repeat(16),
            "b".repeat(16),
            "c".repeat(16),
            "d".repeat(16)
        ))
        .is_err());
        // Padding is not tolerated in unpadded base64url.
        assert!(PubkyMarketOrderReceiptAttestation::parse(&format!(
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
    #[test]
    fn test_captured_v1_and_v2_samples_verify_signatures() {
        // Captured from marketplace-service fcd38b2 contracts/samples.
        for fixture in [
            include_str!("../test_fixtures/order-receipt-attestation-v1.json"),
            include_str!("../test_fixtures/order-receipt-attestation-v2-bitcoin.json"),
            include_str!("../test_fixtures/order-receipt-attestation-v2-same-currency.json"),
        ] {
            let sample: serde_json::Value = serde_json::from_str(fixture).unwrap();
            let jws = sample["receipt_attestation"]["jws"].as_str().unwrap();
            let attestation = PubkyMarketOrderReceiptAttestation::parse(jws).unwrap();
            attestation.verify_signature().unwrap();
        }
    }

    #[test]
    fn test_v2_record_money_conversions_fail_closed() {
        let sample: serde_json::Value = serde_json::from_str(include_str!(
            "../test_fixtures/order-receipt-attestation-v2-same-currency.json"
        ))
        .unwrap();
        let jws = sample["receipt_attestation"]["jws"].as_str().unwrap();
        let attestation = PubkyMarketOrderReceiptAttestation::parse(jws).unwrap();
        let claims = attestation.v2_claims().unwrap();

        for total in [
            PubkyMarketMoney {
                amount_minor: -1,
                currency: claims.merchandise_total.currency.clone(),
                exponent: i64::from(claims.merchandise_total.exponent),
            },
            PubkyMarketMoney {
                amount_minor: claims.merchandise_total.amount_minor as i64,
                currency: claims.merchandise_total.currency.clone(),
                exponent: 256,
            },
        ] {
            let record = PubkyMarketOrderReceipt::new(
                claims.buyer.clone(),
                1,
                claims.paid_at.clone(),
                claims.paid_at.clone(),
                PubkyMarketOrderReceiptRole::Buyer,
                claims.receipt.clone(),
                claims.order.clone(),
                claims.buyer.clone(),
                claims.seller.clone(),
                total,
                claims.paid_at.clone(),
                jws.to_string(),
            );
            assert!(attestation.verify_binding(&record).is_err());
        }
    }

    #[test]
    fn test_v2_money_bounds_and_typ_mismatch_rejected() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../test_fixtures/order-receipt-attestation-v2-same-currency.json"
        ))
        .unwrap();
        let parts: Vec<&str> = fixture["receipt_attestation"]["jws"]
            .as_str()
            .unwrap()
            .split('.')
            .collect();
        let mut claims: serde_json::Value =
            serde_json::from_slice(&base64url_decode(parts[1]).unwrap()).unwrap();
        claims["settlement_total"]["exponent"] = serde_json::json!(19);
        let payload = base64url_encode(serde_json::to_vec(&claims).unwrap().as_slice());
        assert!(PubkyMarketOrderReceiptAttestation::parse(&format!(
            "{}.{}.{}",
            parts[0], payload, parts[2]
        ))
        .is_err());
        for amount in [0_i64, 9_007_199_254_740_992_i64] {
            claims["settlement_total"]["amount_minor"] = serde_json::json!(amount);
            let payload = base64url_encode(serde_json::to_vec(&claims).unwrap().as_slice());
            assert!(PubkyMarketOrderReceiptAttestation::parse(&format!(
                "{}.{}.{}",
                parts[0], payload, parts[2]
            ))
            .is_err());
        }
        claims["settlement_total"]["currency"] = serde_json::json!("usd");
        let payload = base64url_encode(serde_json::to_vec(&claims).unwrap().as_slice());
        assert!(PubkyMarketOrderReceiptAttestation::parse(&format!(
            "{}.{}.{}",
            parts[0], payload, parts[2]
        ))
        .is_err());
        let header = base64url_encode(br#"{"alg":"EdDSA","typ":"pubky-order-receipt+v1"}"#);
        assert!(PubkyMarketOrderReceiptAttestation::parse(&format!(
            "{}.{}.{}",
            header, parts[1], parts[2]
        ))
        .is_err());
    }
}
