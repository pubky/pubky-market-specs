//! The marketplace purchase attestation: a durable, publicly verifiable
//! compact JWS (RFC 7515, `alg: EdDSA` per RFC 8037) embedded in a
//! marketplace review record's `eligibilityAttestation` field.
//!
//! This module is the normative reference for third-party verifiers
//! (ADR 0024 in the pubky-app marketplace branch). The verification recipe:
//!
//! 1. Parse the compact serialization (`header.payload.signature`, all
//!    base64url without padding) with [`PubkyMarketPurchaseAttestation::parse`].
//!    Parsing is closed-world: the header must be exactly
//!    `{"alg":"EdDSA","typ":"pubky-purchase-attestation+v1"}` and the claim
//!    set must be exactly the `v: 1` claims — unknown claims are rejected.
//! 2. Verify the Ed25519 signature with
//!    [`PubkyMarketPurchaseAttestation::verify_signature`]. The issuer key needs
//!    no discovery: the `iss` claim is a pubky, i.e. the z-base-32 encoding
//!    of the Ed25519 verification key itself.
//! 3. Check the claim bindings against the review record with
//!    [`PubkyMarketPurchaseAttestation::verify_binding`]: `sub` must equal the
//!    record's `ownerPubky`, `cpk` its `subjectPubky`, `listing` the
//!    canonical listing URI, and `role` the record's role.
//! 4. Accept as *verified* only when `iss` is on the verifier's own attestor
//!    trust list. The signature proves key possession, never legitimacy.
//!
//! The attestation attests the purchase, not the review text: revisions of
//! the review record leave it unchanged. It has no expiry; bad outcomes are
//! annotated by the attestor, not revoked.

use crate::{
    listing_uri_builder,
    models::marketplace::{validate_hex_hash, validate_pubky, MAX_SAFE_INTEGER},
    models::marketplace_review::PubkyMarketReview,
    models::marketplace_weight::{parse_log_decade, price_weight, MIN_PRICE_WEIGHT},
    traits::Validatable,
    uri::{ParsedUri, Resource},
};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

/// The exact JOSE `typ` value of a v1 purchase attestation.
pub const PURCHASE_ATTESTATION_TYP: &str = "pubky-purchase-attestation+v1";
/// The only claim-set version this module accepts.
pub const PURCHASE_ATTESTATION_VERSION: i64 = 1;

// The spec field's own bounds (mirrors marketplace_review.rs).
const MIN_ATTESTATION_LENGTH: usize = 32;
const MAX_ATTESTATION_LENGTH: usize = 4_096;

/// z-base-32 alphabet used by pubky identifiers (RFC-less, Phil Zimmermann's
/// human-oriented base-32; the pubky encoding of Ed25519 public keys).
const Z_BASE_32_ALPHABET: &[u8; 32] = b"ybndrfg8ejkmcpqxot1uwisza345h769";

/// The protected JOSE header of a purchase attestation. Closed-world: both
/// fields are mandatory and no other field is tolerated.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct AttestationHeader {
    alg: String,
    typ: String,
}

/// The `v: 1` claim set. Closed-world (`deny_unknown_fields`): verifiers
/// reject attestations carrying claims this version does not define.
///
/// Privacy constraints inherited from the marketplace redaction rules:
/// `completed_on` is day-granularity, `order_ref` is an attestor-salted hash
/// (nobody but the attestor can link it back to an order), and
/// `amount_band` is an optional log-decade band of the order total emitted
/// only under both-sides consent, and `price_decade` the log decade of the
/// line's unit price, which the public listing price already discloses —
/// never exact amounts, addresses, or payment IDs.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketPurchaseAttestationClaims {
    /// Claim-set version; always `1`.
    pub v: i64,
    /// Attestor pubky (z-base-32 of the Ed25519 verification key).
    pub iss: String,
    /// Reviewer pubky; must equal the review record's `ownerPubky`.
    pub sub: String,
    /// Counterparty pubky; must equal the review record's `subjectPubky`.
    pub cpk: String,
    /// Review role, `buyer_reviewing_seller` or `seller_reviewing_buyer`.
    pub role: String,
    /// Canonical listing URI
    /// (`pubky://<seller>/pub/pubky.app/marketplace/v1/listings/<id>`).
    pub listing: String,
    /// Attestor-salted Blake3 of the private order UUID, lowercase hex.
    pub order_ref: String,
    /// Order completion date, day granularity (`YYYY-MM-DD`).
    pub completed_on: String,
    /// Optional log-decade amount band, `{CURRENCY}:{floor(log10(minor))}`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_band: Option<String>,
    /// Log decade of the attested line's unit price paid,
    /// `{CURRENCY}:{floor(log10(price))}` with the listing's currency code,
    /// counted in the price weight table's minor unit (satoshis for `BTC`;
    /// see [`crate::price_decade`]); the review's price weight. Absent on
    /// attestations issued before it existed, which weigh 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_decade: Option<String>,
    /// Issuance time, seconds since the UNIX epoch.
    pub iat: i64,
}

/// A parsed (structurally valid) purchase attestation. Construction via
/// [`Self::parse`] guarantees the header and claims are well-formed; it does
/// NOT imply the signature was checked — call [`Self::verify_signature`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PubkyMarketPurchaseAttestation {
    pub claims: PubkyMarketPurchaseAttestationClaims,
    /// The exact bytes the signature covers: `header_b64 || '.' || payload_b64`.
    signing_input: String,
    signature: [u8; 64],
}

impl PubkyMarketPurchaseAttestation {
    /// Parses and structurally validates a compact JWS purchase attestation.
    /// Rejects unknown versions, unknown claims, unknown header fields, and
    /// every claim-format violation. Does not verify the signature.
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
        if header.typ != PURCHASE_ATTESTATION_TYP {
            return Err(format!(
                "Attestation Error: typ must be {PURCHASE_ATTESTATION_TYP}"
            ));
        }

        let payload_bytes = base64url_decode(payload_b64)
            .ok_or("Attestation Error: payload is not valid base64url")?;
        let claims: PubkyMarketPurchaseAttestationClaims =
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

    /// Checks that the claims cover exactly this review record: `sub` is the
    /// record owner, `cpk` the subject, `listing` the canonical listing URI,
    /// and `role` the record role. Any mismatch means the attestation does
    /// not cover the review.
    pub fn verify_binding(&self, review: &PubkyMarketReview) -> Result<(), String> {
        if self.claims.sub != review.owner_pubky {
            return Err("Attestation Error: sub does not match the review owner".into());
        }
        if self.claims.cpk != review.subject_pubky {
            return Err("Attestation Error: cpk does not match the review subject".into());
        }
        let listing_uri = listing_uri_builder(
            review.listing_owner_pubky.clone(),
            review.listing_id.clone(),
        );
        if self.claims.listing != listing_uri {
            return Err("Attestation Error: listing does not match the reviewed listing".into());
        }
        if self.claims.role != review.role.as_str() {
            return Err("Attestation Error: role does not match the review role".into());
        }
        if let Some(order_ref) = &review.order_ref {
            if &self.claims.order_ref != order_ref {
                return Err(
                    "Attestation Error: order_ref does not match the review orderRef".into(),
                );
            }
        }
        Ok(())
    }

    /// The attested order this attestation stands for: `(iss, order_ref)`.
    /// A verified review counts once per attested order and review role;
    /// see [`PubkyMarketReview::is_order_scoped`] for which record
    /// counts when two carry the same key.
    pub fn order_key(&self) -> (String, String) {
        (self.claims.iss.clone(), self.claims.order_ref.clone())
    }

    /// The review's 1-5 price weight from the `price_decade` claim; 1 when
    /// the claim is absent or names a currency without a decade offset.
    /// [`Self::parse`] has already rejected a malformed claim.
    pub fn price_weight(&self) -> u8 {
        price_weight(self.claims.price_decade.as_deref()).unwrap_or(MIN_PRICE_WEIGHT)
    }

    /// The full local verification recipe for one review record: parse the
    /// record's embedded attestation, verify the signature, and check the
    /// bindings. Trust in `iss` remains the caller's policy decision.
    pub fn verify_for_review(review: &PubkyMarketReview) -> Result<Self, String> {
        let attestation = Self::parse(&review.eligibility_attestation)?;
        attestation.verify_signature()?;
        attestation.verify_binding(review)?;
        Ok(attestation)
    }
}

fn validate_claims(claims: &PubkyMarketPurchaseAttestationClaims) -> Result<(), String> {
    if claims.v != PURCHASE_ATTESTATION_VERSION {
        return Err(format!(
            "Attestation Error: v must be {PURCHASE_ATTESTATION_VERSION}"
        ));
    }
    validate_pubky(&claims.iss, "iss").map_err(attestation_error)?;
    validate_pubky(&claims.sub, "sub").map_err(attestation_error)?;
    validate_pubky(&claims.cpk, "cpk").map_err(attestation_error)?;
    if claims.role != "buyer_reviewing_seller" && claims.role != "seller_reviewing_buyer" {
        return Err(
            "Attestation Error: role must be buyer_reviewing_seller or seller_reviewing_buyer"
                .into(),
        );
    }
    let parsed = ParsedUri::try_from(claims.listing.as_str())
        .map_err(|_| "Attestation Error: listing must be a canonical listing URI".to_string())?;
    if !matches!(parsed.resource, Resource::Listing(_)) {
        return Err("Attestation Error: listing must be a canonical listing URI".into());
    }
    validate_hex_hash(&claims.order_ref, "order_ref").map_err(attestation_error)?;
    validate_civil_date(&claims.completed_on)?;
    if let Some(band) = &claims.amount_band {
        parse_log_decade(band, "amount_band").map_err(attestation_error)?;
    }
    if let Some(decade) = &claims.price_decade {
        parse_log_decade(decade, "price_decade").map_err(attestation_error)?;
    }
    if !(1..=MAX_SAFE_INTEGER).contains(&claims.iat) {
        return Err("Attestation Error: iat must be a positive safe integer".into());
    }
    Ok(())
}

pub(crate) fn attestation_error(validation_error: String) -> String {
    validation_error.replace("Validation Error:", "Attestation Error:")
}

/// Validates a `YYYY-MM-DD` civil date (day granularity, deliberately no
/// time component — finer timestamps would strengthen payment correlation).
fn validate_civil_date(value: &str) -> Result<(), String> {
    if is_civil_date(value) {
        Ok(())
    } else {
        Err("Attestation Error: completed_on must be a YYYY-MM-DD date".to_string())
    }
}

/// True for a real `YYYY-MM-DD` calendar date.
pub(crate) fn is_civil_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let digits = |range: std::ops::Range<usize>| -> Option<i64> {
        let slice = &bytes[range];
        if !slice.iter().all(u8::is_ascii_digit) {
            return None;
        }
        Some(
            slice
                .iter()
                .fold(0i64, |acc, b| acc * 10 + i64::from(b - b'0')),
        )
    };
    let (Some(year), Some(month), Some(day)) = (digits(0..4), digits(5..7), digits(8..10)) else {
        return false;
    };
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days_in_month).contains(&day)
}

/// Decodes unpadded base64url (RFC 4648 §5). Padding characters and
/// non-alphabet characters are rejected.
pub(crate) fn base64url_decode(input: &str) -> Option<Vec<u8>> {
    fn value_of(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some(u32::from(c - b'A')),
            b'a'..=b'z' => Some(u32::from(c - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(c - b'0') + 52),
            b'-' => Some(62),
            b'_' => Some(63),
            _ => None,
        }
    }

    let bytes = input.as_bytes();
    if bytes.len() % 4 == 1 {
        return None;
    }
    let mut output = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut accumulator: u32 = 0;
        for &byte in chunk {
            accumulator = (accumulator << 6) | value_of(byte)?;
        }
        match chunk.len() {
            4 => output.extend_from_slice(&[
                (accumulator >> 16) as u8,
                (accumulator >> 8) as u8,
                accumulator as u8,
            ]),
            3 => {
                accumulator <<= 6;
                output.extend_from_slice(&[(accumulator >> 16) as u8, (accumulator >> 8) as u8]);
            }
            2 => {
                accumulator <<= 12;
                output.push((accumulator >> 16) as u8);
            }
            _ => return None,
        }
    }
    Some(output)
}

/// Encodes bytes as unpadded base64url (RFC 4648 §5). Exposed so issuers
/// and test harnesses can build compact JWS segments without a JOSE stack.
pub fn base64url_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut output = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let mut accumulator: u32 = 0;
        for (index, &byte) in chunk.iter().enumerate() {
            accumulator |= u32::from(byte) << (16 - 8 * index);
        }
        for position in 0..=chunk.len() {
            let shift = 18 - 6 * position;
            output.push(ALPHABET[((accumulator >> shift) & 0x3F) as usize] as char);
        }
    }
    output
}

/// Decodes a 52-character z-base-32 pubky into its 32-byte Ed25519 key.
pub(crate) fn zbase32_decode_pubky(pubky: &str) -> Option<[u8; 32]> {
    let bytes = pubky.as_bytes();
    if bytes.len() != 52 {
        return None;
    }
    let mut accumulator: u64 = 0;
    let mut bit_count: u32 = 0;
    let mut output = Vec::with_capacity(32);
    for &byte in bytes {
        let value = Z_BASE_32_ALPHABET.iter().position(|&c| c == byte)? as u64;
        accumulator = (accumulator << 5) | value;
        bit_count += 5;
        if bit_count >= 8 {
            bit_count -= 8;
            output.push((accumulator >> bit_count) as u8);
        }
    }
    // 52 chars * 5 bits = 260 bits: the trailing 4 bits are padding.
    output.truncate(32);
    output.try_into().ok()
}

impl Validatable for PubkyMarketPurchaseAttestationClaims {
    fn validate(&self, _id: Option<&str>) -> Result<(), String> {
        validate_claims(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::HashId;
    use crate::{PubkyMarketReviewRatings, PubkyMarketReviewRole};
    use base32::{encode as base32_encode, Alphabet};
    use ed25519_dalek::{Signer, SigningKey};

    const REVIEWER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
    const SELLER: &str = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";

    fn attestor_key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    fn attestor_pubky(key: &SigningKey) -> String {
        base32_encode(Alphabet::Z, key.verifying_key().as_bytes())
    }

    fn valid_claims(iss: &str) -> PubkyMarketPurchaseAttestationClaims {
        PubkyMarketPurchaseAttestationClaims {
            v: 1,
            iss: iss.to_string(),
            sub: REVIEWER.to_string(),
            cpk: SELLER.to_string(),
            role: "buyer_reviewing_seller".to_string(),
            listing: crate::listing_uri_builder(SELLER.to_string(), "0032SSN7Q4EVG".to_string()),
            order_ref: "a".repeat(64),
            completed_on: "2026-08-21".to_string(),
            amount_band: Some("SAT:5".to_string()),
            price_decade: None,
            iat: 1_787_654_321,
        }
    }

    fn sign(claims: &PubkyMarketPurchaseAttestationClaims, key: &SigningKey) -> String {
        let header = serde_json::json!({ "alg": "EdDSA", "typ": PURCHASE_ATTESTATION_TYP });
        let header_b64 = base64url_encode(serde_json::to_vec(&header).unwrap().as_slice());
        let payload_b64 = base64url_encode(serde_json::to_vec(claims).unwrap().as_slice());
        let signing_input = format!("{header_b64}.{payload_b64}");
        let signature = key.sign(signing_input.as_bytes());
        format!(
            "{signing_input}.{}",
            base64url_encode(&signature.to_bytes())
        )
    }

    fn matching_review(attestation: &str) -> PubkyMarketReview {
        let mut review = PubkyMarketReview::new(
            REVIEWER.to_string(),
            1,
            "2026-08-21T00:00:00Z".to_string(),
            "2026-08-21T00:00:00Z".to_string(),
            String::new(),
            SELLER.to_string(),
            SELLER.to_string(),
            "0032SSN7Q4EVG".to_string(),
            PubkyMarketReviewRole::BuyerReviewingSeller,
            PubkyMarketReviewRatings {
                overall: 5,
                item_accuracy: None,
                shipping: None,
                communication: None,
            },
            "Great seller, fast shipping.".to_string(),
            attestation.to_string(),
        );
        review.review_id = review.create_id();
        review
    }

    #[test]
    fn test_valid_attestation_full_recipe() {
        let key = attestor_key();
        let iss = attestor_pubky(&key);
        let claims = valid_claims(&iss);
        let jws = sign(&claims, &key);
        let review = matching_review(&jws);

        let attestation =
            PubkyMarketPurchaseAttestation::verify_for_review(&review).expect("recipe verifies");
        assert_eq!(attestation.claims, claims);
        // The record's own field validation also accepts the JWS charset.
        let id = review.review_id.clone();
        assert!(review.validate(Some(&id)).is_ok());
    }

    #[test]
    fn test_order_ref_binds_to_the_attested_order() {
        let key = attestor_key();
        let claims = valid_claims(&attestor_pubky(&key));
        let jws = sign(&claims, &key);

        let mut review = matching_review(&jws);
        review.order_ref = Some(claims.order_ref.clone());
        review.review_id = review.create_id();
        assert!(PubkyMarketPurchaseAttestation::verify_for_review(&review).is_ok());

        review.order_ref = Some("b".repeat(64));
        review.review_id = review.create_id();
        let error = PubkyMarketPurchaseAttestation::verify_for_review(&review).unwrap_err();
        assert!(error.contains("order_ref"), "{error}");
    }

    #[test]
    fn test_legacy_and_scoped_records_share_one_order_key() {
        let key = attestor_key();
        let claims = valid_claims(&attestor_pubky(&key));
        let jws = sign(&claims, &key);

        let legacy = matching_review(&jws);
        let mut scoped = matching_review(&jws);
        scoped.order_ref = Some(claims.order_ref.clone());
        scoped.review_id = scoped.create_id();

        // Both records verify at their own paths, so a consumer must dedupe them.
        let legacy_attestation =
            PubkyMarketPurchaseAttestation::verify_for_review(&legacy).unwrap();
        let scoped_attestation =
            PubkyMarketPurchaseAttestation::verify_for_review(&scoped).unwrap();
        assert!(legacy.validate(Some(&legacy.review_id.clone())).is_ok());
        assert!(scoped.validate(Some(&scoped.review_id.clone())).is_ok());
        assert_ne!(legacy.review_id, scoped.review_id);

        assert_eq!(
            legacy_attestation.order_key(),
            scoped_attestation.order_key()
        );
        assert_eq!(
            scoped_attestation.order_key(),
            (claims.iss.clone(), claims.order_ref.clone())
        );
        assert!(scoped.is_order_scoped());
        assert!(!legacy.is_order_scoped());
    }

    fn review_of_listing(listing_id: &str, attestation: &str) -> PubkyMarketReview {
        let mut review = matching_review(attestation);
        review.listing_id = listing_id.to_string();
        review.order_ref = Some("a".repeat(64));
        review.review_id = review.create_id();
        review
    }

    #[test]
    fn test_item_keys_differ_across_listings_of_one_order() {
        let key = attestor_key();
        let iss = attestor_pubky(&key);
        let item_a = "0032SSN7Q4EVG";
        let item_b = "0032SSN7Q4EVH";

        let mut claims_b = valid_claims(&iss);
        claims_b.listing = crate::listing_uri_builder(SELLER.to_string(), item_b.to_string());
        let review_a = review_of_listing(item_a, &sign(&valid_claims(&iss), &key));
        let review_b = review_of_listing(item_b, &sign(&claims_b, &key));

        let attestation_a = PubkyMarketPurchaseAttestation::verify_for_review(&review_a).unwrap();
        let attestation_b = PubkyMarketPurchaseAttestation::verify_for_review(&review_b).unwrap();

        let item_key_a = review_a.item_count_key(&attestation_a).unwrap();
        let item_key_b = review_b.item_count_key(&attestation_b).unwrap();
        assert_ne!(item_key_a, item_key_b);
        assert_ne!(review_a.review_id, review_b.review_id);

        let seller_key_a = review_a.seller_rating_key(&attestation_a).unwrap();
        let seller_key_b = review_b.seller_rating_key(&attestation_b).unwrap();
        assert_eq!(seller_key_a, seller_key_b);
        assert_eq!(seller_key_a, item_key_a.seller_rating_key());

        assert_eq!(
            item_key_a.to_string(),
            format!(
                "{iss}:{}:buyer_reviewing_seller:{SELLER}:{item_a}",
                "a".repeat(64)
            )
        );
        assert_eq!(
            seller_key_a.to_string(),
            format!("{iss}:{}:buyer_reviewing_seller", "a".repeat(64))
        );
    }

    #[test]
    fn test_legacy_record_shares_its_item_key_with_a_scoped_twin() {
        let key = attestor_key();
        let jws = sign(&valid_claims(&attestor_pubky(&key)), &key);
        let legacy = matching_review(&jws);
        let scoped = review_of_listing(&legacy.listing_id, &jws);

        let legacy_attestation =
            PubkyMarketPurchaseAttestation::verify_for_review(&legacy).unwrap();
        let scoped_attestation =
            PubkyMarketPurchaseAttestation::verify_for_review(&scoped).unwrap();
        assert_eq!(
            legacy.item_count_key(&legacy_attestation).unwrap(),
            scoped.item_count_key(&scoped_attestation).unwrap()
        );
    }

    #[test]
    fn test_count_keys_require_a_binding_attestation() {
        let key = attestor_key();
        let iss = attestor_pubky(&key);
        let mut other_listing = valid_claims(&iss);
        other_listing.listing =
            crate::listing_uri_builder(SELLER.to_string(), "0032SSN7Q4EVH".to_string());
        let attestation =
            PubkyMarketPurchaseAttestation::parse(&sign(&other_listing, &key)).unwrap();

        let review = review_of_listing("0032SSN7Q4EVG", &sign(&valid_claims(&iss), &key));
        assert!(review.item_count_key(&attestation).is_err());
        assert!(review.seller_rating_key(&attestation).is_err());
    }

    #[test]
    fn test_price_decade_claim_sets_the_weight() {
        let key = attestor_key();
        let mut claims = valid_claims(&attestor_pubky(&key));
        claims.price_decade = Some("USD:4".to_string());
        let jws = sign(&claims, &key);
        let review = matching_review(&jws);

        let attestation = PubkyMarketPurchaseAttestation::verify_for_review(&review).unwrap();
        assert_eq!(attestation.claims.price_decade.as_deref(), Some("USD:4"));
        assert_eq!(attestation.price_weight(), 3);

        claims.price_decade = Some("XYZ:9".to_string());
        let attestation = PubkyMarketPurchaseAttestation::parse(&sign(&claims, &key)).unwrap();
        assert_eq!(attestation.price_weight(), 1);
    }

    #[test]
    fn test_attestation_without_price_decade_still_verifies() {
        let key = attestor_key();
        let claims = valid_claims(&attestor_pubky(&key));
        let jws = sign(&claims, &key);
        let payload = serde_json::to_value(&claims).unwrap();
        assert!(payload.get("price_decade").is_none());

        let attestation =
            PubkyMarketPurchaseAttestation::verify_for_review(&matching_review(&jws)).unwrap();
        assert_eq!(attestation.claims.price_decade, None);
        assert_eq!(attestation.price_weight(), 1);
    }

    #[test]
    fn test_malformed_price_decade_rejected() {
        let key = attestor_key();
        let iss = attestor_pubky(&key);
        for value in ["usd:3", "USD", "USD:19", "US:3", "USD:3.0", ""] {
            let mut claims = valid_claims(&iss);
            claims.price_decade = Some(value.to_string());
            assert!(validate_claims(&claims).is_err(), "{value:?}");
            let error = PubkyMarketPurchaseAttestation::parse(&sign(&claims, &key)).unwrap_err();
            assert!(
                error.starts_with("Attestation Error: price_decade"),
                "{error}"
            );
        }
    }

    #[test]
    fn test_wrong_key_fails_signature() {
        let key = attestor_key();
        let other_key = SigningKey::from_bytes(&[9u8; 32]);
        // Claims name the honest attestor, but the signature is forged with
        // another key.
        let claims = valid_claims(&attestor_pubky(&key));
        let jws = sign(&claims, &other_key);
        let attestation = PubkyMarketPurchaseAttestation::parse(&jws).expect("structurally valid");
        assert!(attestation.verify_signature().is_err());
    }

    #[test]
    fn test_mismatched_binding_fails() {
        let key = attestor_key();
        let iss = attestor_pubky(&key);

        // sub names a different reviewer than the record owner.
        let mut claims = valid_claims(&iss);
        claims.sub = SELLER.to_string();
        claims.cpk = REVIEWER.to_string();
        let jws = sign(&claims, &key);
        let review = matching_review(&jws);
        let attestation = PubkyMarketPurchaseAttestation::parse(&jws).expect("structurally valid");
        attestation.verify_signature().expect("honest signature");
        assert!(attestation.verify_binding(&review).is_err());

        // listing names a different listing than the record.
        let mut claims = valid_claims(&iss);
        claims.listing = crate::listing_uri_builder(SELLER.to_string(), "0032SSN7Q4EVH".into());
        let jws = sign(&claims, &key);
        let review = matching_review(&jws);
        let attestation = PubkyMarketPurchaseAttestation::parse(&jws).expect("structurally valid");
        assert!(attestation.verify_binding(&review).is_err());

        // role differs from the record's role.
        let mut claims = valid_claims(&iss);
        claims.role = "seller_reviewing_buyer".to_string();
        let jws = sign(&claims, &key);
        let review = matching_review(&jws);
        let attestation = PubkyMarketPurchaseAttestation::parse(&jws).expect("structurally valid");
        assert!(attestation.verify_binding(&review).is_err());
    }

    #[test]
    fn test_unknown_version_rejected() {
        let key = attestor_key();
        let mut claims = valid_claims(&attestor_pubky(&key));
        claims.v = 2;
        let jws = sign(&claims, &key);
        assert!(PubkyMarketPurchaseAttestation::parse(&jws).is_err());
    }

    #[test]
    fn test_unknown_claim_rejected() {
        let key = attestor_key();
        let claims = valid_claims(&attestor_pubky(&key));
        let mut payload = serde_json::to_value(&claims).unwrap();
        payload["surprise"] = serde_json::json!(true);
        let header = serde_json::json!({ "alg": "EdDSA", "typ": PURCHASE_ATTESTATION_TYP });
        let header_b64 = base64url_encode(serde_json::to_vec(&header).unwrap().as_slice());
        let payload_b64 = base64url_encode(serde_json::to_vec(&payload).unwrap().as_slice());
        let signing_input = format!("{header_b64}.{payload_b64}");
        let signature = key.sign(signing_input.as_bytes());
        let jws = format!(
            "{signing_input}.{}",
            base64url_encode(&signature.to_bytes())
        );
        assert!(PubkyMarketPurchaseAttestation::parse(&jws).is_err());
    }

    #[test]
    fn test_wrong_header_rejected() {
        let key = attestor_key();
        let claims = valid_claims(&attestor_pubky(&key));
        for header in [
            serde_json::json!({ "alg": "ES256", "typ": PURCHASE_ATTESTATION_TYP }),
            serde_json::json!({ "alg": "EdDSA", "typ": "jwt" }),
            serde_json::json!({ "alg": "EdDSA", "typ": PURCHASE_ATTESTATION_TYP, "kid": "1" }),
        ] {
            let header_b64 = base64url_encode(serde_json::to_vec(&header).unwrap().as_slice());
            let payload_b64 = base64url_encode(serde_json::to_vec(&claims).unwrap().as_slice());
            let signing_input = format!("{header_b64}.{payload_b64}");
            let signature = key.sign(signing_input.as_bytes());
            let jws = format!(
                "{signing_input}.{}",
                base64url_encode(&signature.to_bytes())
            );
            assert!(PubkyMarketPurchaseAttestation::parse(&jws).is_err());
        }
    }

    #[test]
    fn test_claim_format_violations_rejected() {
        let key = attestor_key();
        let iss = attestor_pubky(&key);

        let mut bad_order_ref = valid_claims(&iss);
        bad_order_ref.order_ref = "UPPERCASE".repeat(8);
        assert!(validate_claims(&bad_order_ref).is_err());

        let mut bad_date = valid_claims(&iss);
        bad_date.completed_on = "2026-02-30".to_string();
        assert!(validate_claims(&bad_date).is_err());

        let mut timestamp_not_date = valid_claims(&iss);
        timestamp_not_date.completed_on = "2026-08-21T10:00:00Z".to_string();
        assert!(validate_claims(&timestamp_not_date).is_err());

        let mut bad_band = valid_claims(&iss);
        bad_band.amount_band = Some("sat:5".to_string());
        assert!(validate_claims(&bad_band).is_err());

        let mut bad_band_magnitude = valid_claims(&iss);
        bad_band_magnitude.amount_band = Some("SAT:19".to_string());
        assert!(validate_claims(&bad_band_magnitude).is_err());

        let mut band_without_magnitude = valid_claims(&iss);
        band_without_magnitude.amount_band = Some("SAT".to_string());
        assert!(validate_claims(&band_without_magnitude).is_err());

        let mut bad_listing = valid_claims(&iss);
        bad_listing.listing = format!("pubky://{SELLER}/pub/pubky.app/posts/0032SSN7Q4EVG");
        assert!(validate_claims(&bad_listing).is_err());

        let mut bad_iss = valid_claims(&iss);
        bad_iss.iss = "not-a-pubky".to_string();
        assert!(validate_claims(&bad_iss).is_err());

        let mut band_absent = valid_claims(&iss);
        band_absent.amount_band = None;
        assert!(validate_claims(&band_absent).is_ok());
    }

    #[test]
    fn test_malformed_compact_forms_rejected() {
        assert!(PubkyMarketPurchaseAttestation::parse("").is_err());
        assert!(PubkyMarketPurchaseAttestation::parse(&"a".repeat(64)).is_err());
        assert!(PubkyMarketPurchaseAttestation::parse(&format!(
            "{}.{}",
            "a".repeat(32),
            "b".repeat(32)
        ))
        .is_err());
        assert!(PubkyMarketPurchaseAttestation::parse(&format!(
            "{}.{}.{}.{}",
            "a".repeat(16),
            "b".repeat(16),
            "c".repeat(16),
            "d".repeat(16)
        ))
        .is_err());
        // Padding is not tolerated in unpadded base64url.
        assert!(PubkyMarketPurchaseAttestation::parse(&format!(
            "{}=.{}.{}",
            "a".repeat(15),
            "b".repeat(16),
            "c".repeat(86)
        ))
        .is_err());
    }

    #[test]
    fn test_base64url_roundtrip() {
        for input in [
            &b""[..],
            &b"f"[..],
            &b"fo"[..],
            &b"foo"[..],
            &b"foob"[..],
            &b"fooba"[..],
            &b"foobar"[..],
            &[0xFF, 0xFE, 0xFD][..],
        ] {
            let encoded = base64url_encode(input);
            assert!(!encoded.contains('='));
            assert_eq!(base64url_decode(&encoded).unwrap(), input);
        }
        assert!(base64url_decode("a").is_none());
        assert!(base64url_decode("ab=c").is_none());
        assert!(base64url_decode("a+bc").is_none());
    }

    #[test]
    fn test_zbase32_pubky_roundtrip() {
        let key = attestor_key();
        let pubky = attestor_pubky(&key);
        assert_eq!(pubky.chars().count(), 52);
        let decoded = zbase32_decode_pubky(&pubky).expect("decodes");
        assert_eq!(&decoded, key.verifying_key().as_bytes());
        assert!(zbase32_decode_pubky("short").is_none());
        assert!(zbase32_decode_pubky(&"l".repeat(52)).is_none());
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
