//! Records an attestor publishes on its own homeserver, owned by the
//! attestor pubky: the attestor profile (who operates the key, which keys
//! it has used, how it issues purchase attestations) and one annotation per
//! attested order (refund and return outcomes).
//!
//! Each record carries a `proof`: a compact JWS (RFC 7515, `alg: EdDSA` per
//! RFC 8037) with a detached payload (RFC 7515 Appendix F), so cached copies
//! verify offline:
//!
//! - header: exactly `{"alg":"EdDSA","typ":<typ>}` with `typ`
//!   [`ATTESTOR_PROFILE_TYP`] or [`ATTESTOR_ANNOTATION_TYP`];
//! - payload: the canonical body, i.e. the record's JSON object without its
//!   `proof` member, serialized per RFC 8785 (JCS). The records hold only
//!   strings, integers, booleans, null, arrays and objects, so JCS reduces
//!   to: members sorted by key (UTF-16 code units), no whitespace, strings
//!   escaped as JSON requires, integers in plain decimal;
//! - serialization: `base64url(header) || ".." || base64url(signature)`,
//!   where the signature covers
//!   `base64url(header) || "." || base64url(canonical body)`.
//!
//! The verification key is the pubky that owns the record's URI, so a copy
//! moved to another homeserver fails. The `typ` tells the two records apart,
//! and an annotation's `orderRef` must equal its path ID, so a record moved
//! to another path fails too. Unknown members are tolerated and covered by
//! the proof.

use crate::{
    models::marketplace::{
        parse_rfc3339_millis, validate_hex_hash, validate_pubky, PubkyMarketMoney, MAX_SAFE_INTEGER,
    },
    models::marketplace_attestation::{
        base64url_decode, base64url_encode, is_civil_date, zbase32_decode_pubky,
    },
    traits::{HasIdPath, HasPath, Validatable},
    uri::{ParsedUri, Resource},
    APP_PATH, PUBLIC_PATH,
};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

/// The exact JOSE `typ` of an attestor profile proof.
pub const ATTESTOR_PROFILE_TYP: &str = "pubky-attestor-profile+v1";
/// The exact JOSE `typ` of an attestor annotation proof.
pub const ATTESTOR_ANNOTATION_TYP: &str = "pubky-attestor-annotation+v1";
/// `issuancePolicy.amountBand` of an attestor that adds the order total's
/// band only when both trade sides consent.
pub const AMOUNT_BAND_POLICY_BOTH_SIDES_CONSENT: &str = "both-sides consent";
/// `issuancePolicy.priceDecade` of an attestor that signs every per-item
/// attestation's `price_decade`.
pub const PRICE_DECADE_POLICY_ALWAYS: &str = "always";

const MAX_OPERATOR_NAME_LENGTH: usize = 100;
const MAX_URL_LENGTH: usize = 2_048;
const MAX_PROFILE_KEYS: usize = 16;
const MAX_POLICY_VALUE_LENGTH: usize = 64;
const MAX_EDIT_WINDOW_HOURS: i64 = 8_760;
const MAX_BLIND_WINDOW_DAYS: i64 = 365;
const MAX_ANNOTATION_OUTCOMES: usize = 16;
const MAX_PROOF_LENGTH: usize = 512;

/// Who operates the attestor key.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketAttestorOperator {
    /// Operator display name, e.g. `"Pubky Market"`.
    pub name: String,
    /// Operator website (https).
    pub url: String,
    /// The attestor's issuance policy page (https).
    pub policy_url: String,
}

/// One key the attestor has issued with. A verifier may only narrow its own
/// trust window from this entry (effective end = the earlier of its
/// configured `validUntil` and `compromisedFrom`); it never adds keys from a
/// profile, because whoever holds a stolen key can rewrite the profile.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketAttestorKey {
    /// z-base-32 pubky of the key.
    pub pubky: String,
    /// RFC 3339 datetime the key started issuing.
    pub valid_from: String,
    /// RFC 3339 datetime the key stopped issuing; absent while in use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_until: Option<String>,
    /// RFC 3339 datetime from which attestations by this key must not be
    /// trusted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compromised_from: Option<String>,
}

/// How the attestor issues purchase attestations.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketAttestorIssuancePolicy {
    /// Hours after the first review in which items may be added.
    pub edit_window_hours: i64,
    /// Days a double-blind review stays sealed.
    pub blind_window_days: i64,
    /// When `amount_band` is issued, e.g. [`AMOUNT_BAND_POLICY_BOTH_SIDES_CONSENT`].
    pub amount_band: String,
    /// When `price_decade` is issued, e.g. [`PRICE_DECADE_POLICY_ALWAYS`].
    pub price_decade: String,
    /// Smallest order total that earns an attestation; `null` for none.
    /// Always serialized, as `null` when unset.
    #[serde(default)]
    pub min_order_value: Option<PubkyMarketMoney>,
}

/// The attestor profile.
///
/// URI: `/pub/pubky.app/marketplace/v1/attestor/profile`, on the attestor's
/// own homeserver.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketAttestorProfile {
    pub operator: PubkyMarketAttestorOperator,
    pub keys: Vec<PubkyMarketAttestorKey>,
    /// z-base-32 pubky of the attestor that takes over from this one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub successor: Option<String>,
    pub issuance_policy: PubkyMarketAttestorIssuancePolicy,
    /// Detached-payload compact JWS over the canonical body.
    #[serde(default)]
    pub proof: String,
}

/// An outcome recorded against an attested order.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub enum PubkyMarketAttestorOutcomeKind {
    Refunded,
    Returned,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketAttestorOutcome {
    pub kind: PubkyMarketAttestorOutcomeKind,
    /// Day the outcome happened, `YYYY-MM-DD`.
    pub on: String,
}

/// Outcomes of one attested order, published only for an `order_ref` the
/// attestor issued an attestation for.
///
/// URI: `/pub/pubky.app/marketplace/v1/attestor/annotations/:order_ref`, on
/// the attestor's own homeserver. A newer `revision` replaces an older one.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketAttestorAnnotation {
    /// The attested order's `order_ref`; must equal the path ID.
    pub order_ref: String,
    pub outcomes: Vec<PubkyMarketAttestorOutcome>,
    /// Positive revision, raised on every change.
    pub revision: i64,
    /// Detached-payload compact JWS over the canonical body.
    #[serde(default)]
    pub proof: String,
}

impl HasPath for PubkyMarketAttestorProfile {
    const PATH_SEGMENT: &'static str = "marketplace/v1/attestor/profile";

    fn create_path() -> String {
        [PUBLIC_PATH, APP_PATH, Self::PATH_SEGMENT].concat()
    }
}

impl HasIdPath for PubkyMarketAttestorAnnotation {
    const PATH_SEGMENT: &'static str = "marketplace/v1/attestor/annotations/";

    fn create_path(id: &str) -> String {
        [PUBLIC_PATH, APP_PATH, Self::PATH_SEGMENT, id].concat()
    }
}

impl PubkyMarketAttestorProfile {
    /// Validates the record and sets `proof`, signed with the attestor's
    /// Ed25519 secret key.
    pub fn sign(mut self, secret_key: &[u8; 32]) -> Result<Self, String> {
        self.proof = String::new();
        self.validate_fields()?;
        self.proof = sign_proof(ATTESTOR_PROFILE_TYP, &to_object(&self)?, secret_key)?;
        Ok(self)
    }

    /// The canonical body the proof covers.
    pub fn canonical_body(&self) -> Result<String, String> {
        canonical_body(&to_object(self)?)
    }

    /// The whole record, `proof` included, in canonical form: the bytes a
    /// publisher writes.
    pub fn to_canonical_json(&self) -> Result<String, String> {
        canonical_json(&to_object(self)?)
    }

    /// Loads a profile fetched from `uri`: the URI must be an attestor
    /// profile path, the proof must verify against the URI's owner, and the
    /// record must validate.
    pub fn from_verified_json(uri: &str, blob: &[u8]) -> Result<Self, String> {
        let parsed = ParsedUri::try_from(uri)?;
        if parsed.resource != Resource::AttestorProfile {
            return Err("Proof Error: not an attestor profile URI".into());
        }
        let record: Self = load_verified(ATTESTOR_PROFILE_TYP, parsed.user_id.as_ref(), blob)?;
        record.validate(None)?;
        Ok(record)
    }

    fn validate_fields(&self) -> Result<(), String> {
        let name_length = self.operator.name.chars().count();
        if !(1..=MAX_OPERATOR_NAME_LENGTH).contains(&name_length)
            || self.operator.name.trim() != self.operator.name
        {
            return Err(format!(
                "Validation Error: operator.name must be 1-{MAX_OPERATOR_NAME_LENGTH} characters without surrounding whitespace"
            ));
        }
        validate_https_url(&self.operator.url, "operator.url")?;
        validate_https_url(&self.operator.policy_url, "operator.policyUrl")?;

        if !(1..=MAX_PROFILE_KEYS).contains(&self.keys.len()) {
            return Err(format!(
                "Validation Error: keys must hold 1-{MAX_PROFILE_KEYS} entries"
            ));
        }
        for (index, key) in self.keys.iter().enumerate() {
            validate_pubky(&key.pubky, "keys.pubky")?;
            if self.keys[..index]
                .iter()
                .any(|other| other.pubky == key.pubky)
            {
                return Err("Validation Error: keys must not repeat a pubky".into());
            }
            let from = parse_rfc3339_millis(&key.valid_from)
                .map_err(|e| format!("Validation Error: keys.validFrom {e}"))?;
            if let Some(until) = &key.valid_until {
                let until = parse_rfc3339_millis(until)
                    .map_err(|e| format!("Validation Error: keys.validUntil {e}"))?;
                if until <= from {
                    return Err("Validation Error: keys.validUntil must follow validFrom".into());
                }
            }
            if let Some(compromised) = &key.compromised_from {
                let compromised = parse_rfc3339_millis(compromised)
                    .map_err(|e| format!("Validation Error: keys.compromisedFrom {e}"))?;
                if compromised < from {
                    return Err(
                        "Validation Error: keys.compromisedFrom must not precede validFrom".into(),
                    );
                }
            }
        }
        if let Some(successor) = &self.successor {
            validate_pubky(successor, "successor")?;
        }

        let policy = &self.issuance_policy;
        if !(0..=MAX_EDIT_WINDOW_HOURS).contains(&policy.edit_window_hours) {
            return Err(format!(
                "Validation Error: issuancePolicy.editWindowHours must be 0-{MAX_EDIT_WINDOW_HOURS}"
            ));
        }
        if !(0..=MAX_BLIND_WINDOW_DAYS).contains(&policy.blind_window_days) {
            return Err(format!(
                "Validation Error: issuancePolicy.blindWindowDays must be 0-{MAX_BLIND_WINDOW_DAYS}"
            ));
        }
        validate_policy_value(&policy.amount_band, "issuancePolicy.amountBand")?;
        validate_policy_value(&policy.price_decade, "issuancePolicy.priceDecade")?;
        if let Some(min_order_value) = &policy.min_order_value {
            min_order_value.validate_positive("issuancePolicy.minOrderValue")?;
        }
        Ok(())
    }
}

impl Validatable for PubkyMarketAttestorProfile {
    fn validate(&self, _id: Option<&str>) -> Result<(), String> {
        self.validate_fields()?;
        validate_proof_shape(&self.proof)
    }
}

impl PubkyMarketAttestorAnnotation {
    /// Validates the record and sets `proof`, signed with the attestor's
    /// Ed25519 secret key.
    pub fn sign(mut self, secret_key: &[u8; 32]) -> Result<Self, String> {
        self.proof = String::new();
        self.validate_fields()?;
        self.proof = sign_proof(ATTESTOR_ANNOTATION_TYP, &to_object(&self)?, secret_key)?;
        Ok(self)
    }

    /// The canonical body the proof covers.
    pub fn canonical_body(&self) -> Result<String, String> {
        canonical_body(&to_object(self)?)
    }

    /// The whole record, `proof` included, in canonical form: the bytes a
    /// publisher writes.
    pub fn to_canonical_json(&self) -> Result<String, String> {
        canonical_json(&to_object(self)?)
    }

    /// Loads an annotation fetched from `uri`: the URI must be an attestor
    /// annotation path, the proof must verify against the URI's owner, and
    /// the record must validate with `orderRef` equal to the path ID.
    pub fn from_verified_json(uri: &str, blob: &[u8]) -> Result<Self, String> {
        let parsed = ParsedUri::try_from(uri)?;
        let Resource::AttestorAnnotation(order_ref) = &parsed.resource else {
            return Err("Proof Error: not an attestor annotation URI".into());
        };
        let record: Self = load_verified(ATTESTOR_ANNOTATION_TYP, parsed.user_id.as_ref(), blob)?;
        record.validate(Some(order_ref))?;
        Ok(record)
    }

    fn validate_fields(&self) -> Result<(), String> {
        validate_hex_hash(&self.order_ref, "orderRef")?;
        if self.outcomes.len() > MAX_ANNOTATION_OUTCOMES {
            return Err(format!(
                "Validation Error: outcomes must hold at most {MAX_ANNOTATION_OUTCOMES} entries"
            ));
        }
        if let Some(outcome) = self.outcomes.iter().find(|o| !is_civil_date(&o.on)) {
            return Err(format!(
                "Validation Error: outcomes.on must be a YYYY-MM-DD date, got {:?}",
                outcome.on
            ));
        }
        if !(1..=MAX_SAFE_INTEGER).contains(&self.revision) {
            return Err("Validation Error: revision must be a positive safe integer".into());
        }
        Ok(())
    }
}

impl Validatable for PubkyMarketAttestorAnnotation {
    fn validate(&self, id: Option<&str>) -> Result<(), String> {
        if let Some(id) = id {
            if self.order_ref != id {
                return Err(
                    "Validation Error: orderRef does not match the annotation path ID".into(),
                );
            }
        }
        self.validate_fields()?;
        validate_proof_shape(&self.proof)
    }
}

fn validate_https_url(value: &str, field: &str) -> Result<(), String> {
    let error = || {
        format!(
            "Validation Error: {field} must be an https URL of at most {MAX_URL_LENGTH} characters"
        )
    };
    if value.chars().count() > MAX_URL_LENGTH {
        return Err(error());
    }
    let parsed = url::Url::parse(value).map_err(|_| error())?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() {
        return Err(error());
    }
    Ok(())
}

fn validate_policy_value(value: &str, field: &str) -> Result<(), String> {
    let length = value.chars().count();
    if !(1..=MAX_POLICY_VALUE_LENGTH).contains(&length) || value.trim() != value {
        return Err(format!(
            "Validation Error: {field} must be 1-{MAX_POLICY_VALUE_LENGTH} characters without surrounding whitespace"
        ));
    }
    Ok(())
}

/// The protected header of an attestor record proof. Closed-world.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProofHeader {
    alg: String,
    typ: String,
}

/// Splits a detached-payload compact JWS into its header and signature
/// segments.
fn split_proof(proof: &str) -> Result<(&str, &str), String> {
    let error = || "Proof Error: proof must be a detached-payload compact JWS (header..signature)";
    if proof.is_empty() || proof.chars().count() > MAX_PROOF_LENGTH {
        return Err(format!(
            "Proof Error: proof must be 1-{MAX_PROOF_LENGTH} characters"
        ));
    }
    let mut parts = proof.split('.');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(header), Some(""), Some(signature), None)
            if !header.is_empty() && !signature.is_empty() =>
        {
            Ok((header, signature))
        }
        _ => Err(error().into()),
    }
}

fn validate_proof_shape(proof: &str) -> Result<(), String> {
    let (header, signature) = split_proof(proof)?;
    if base64url_decode(header).is_none() || base64url_decode(signature).is_none() {
        return Err("Proof Error: proof segments must be unpadded base64url".into());
    }
    Ok(())
}

fn to_object<T: Serialize>(record: &T) -> Result<Value, String> {
    serde_json::to_value(record).map_err(|e| format!("Proof Error: {e}"))
}

/// The record's JSON object without `proof`, in canonical form.
fn canonical_body(record: &Value) -> Result<String, String> {
    let Value::Object(members) = record else {
        return Err("Proof Error: record must be a JSON object".into());
    };
    let mut body = members.clone();
    body.remove("proof");
    canonical_json(&Value::Object(body))
}

/// RFC 8785 serialization for the JSON subset the records use (no
/// fractional or exponent numbers).
fn canonical_json(value: &Value) -> Result<String, String> {
    let mut out = String::new();
    write_canonical(value, &mut out)?;
    Ok(out)
}

fn write_canonical(value: &Value, out: &mut String) -> Result<(), String> {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
        Value::Number(number) => {
            if !(number.is_i64() || number.is_u64()) {
                return Err("Proof Error: record numbers must be integers".into());
            }
            out.push_str(&number.to_string());
        }
        Value::String(text) => out.push_str(&Value::String(text.clone()).to_string()),
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_canonical(item, out)?;
            }
            out.push(']');
        }
        Value::Object(members) => {
            let mut entries: Vec<(&String, &Value)> = members.iter().collect();
            entries.sort_by(|(a, _), (b, _)| a.encode_utf16().cmp(b.encode_utf16()));
            out.push('{');
            for (index, (key, member)) in entries.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(key.clone()).to_string());
                out.push(':');
                write_canonical(member, out)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

fn proof_header_segment(typ: &str) -> Result<String, String> {
    let header = serde_json::json!({ "alg": "EdDSA", "typ": typ });
    Ok(base64url_encode(canonical_json(&header)?.as_bytes()))
}

fn sign_proof(typ: &str, record: &Value, secret_key: &[u8; 32]) -> Result<String, String> {
    let header_b64 = proof_header_segment(typ)?;
    let payload_b64 = base64url_encode(canonical_body(record)?.as_bytes());
    let signature =
        SigningKey::from_bytes(secret_key).sign(format!("{header_b64}.{payload_b64}").as_bytes());
    Ok(format!(
        "{header_b64}..{}",
        base64url_encode(&signature.to_bytes())
    ))
}

/// Verifies `record.proof` as a `typ` proof by `signer` over the record's
/// canonical body.
fn verify_proof(typ: &str, record: &Value, signer: &str) -> Result<(), String> {
    let proof = record
        .get("proof")
        .and_then(Value::as_str)
        .ok_or("Proof Error: record has no proof")?;
    let (header_b64, signature_b64) = split_proof(proof)?;

    let header_bytes =
        base64url_decode(header_b64).ok_or("Proof Error: header is not valid base64url")?;
    let header: ProofHeader = serde_json::from_slice(&header_bytes)
        .map_err(|e| format!("Proof Error: invalid header: {e}"))?;
    if header.alg != "EdDSA" {
        return Err("Proof Error: alg must be EdDSA".into());
    }
    if header.typ != typ {
        return Err(format!("Proof Error: typ must be {typ}"));
    }

    let signature: [u8; 64] = base64url_decode(signature_b64)
        .ok_or("Proof Error: signature is not valid base64url")?
        .try_into()
        .map_err(|_| "Proof Error: signature must be 64 bytes".to_string())?;
    let key_bytes =
        zbase32_decode_pubky(signer).ok_or("Proof Error: record owner is not a pubky")?;
    let verifying_key = VerifyingKey::from_bytes(&key_bytes)
        .map_err(|_| "Proof Error: record owner is not a valid Ed25519 key".to_string())?;

    let payload_b64 = base64url_encode(canonical_body(record)?.as_bytes());
    verifying_key
        .verify_strict(
            format!("{header_b64}.{payload_b64}").as_bytes(),
            &Signature::from_bytes(&signature),
        )
        .map_err(|_| "Proof Error: proof does not verify for the record owner".to_string())
}

fn load_verified<T: DeserializeOwned>(typ: &str, signer: &str, blob: &[u8]) -> Result<T, String> {
    let record: Value =
        serde_json::from_slice(blob).map_err(|e| format!("Proof Error: invalid JSON: {e}"))?;
    verify_proof(typ, &record, signer)?;
    serde_json::from_value(record).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{attestor_annotation_uri_builder, attestor_profile_uri_builder, PubkyMarketObject};
    use base32::{encode as base32_encode, Alphabet};

    const SECRET_KEY: [u8; 32] = [7u8; 32];
    const OTHER_SECRET_KEY: [u8; 32] = [9u8; 32];
    const ORDER_REF: &str = "5f1a3a1c2d0e4b8f9a7c6e5d4b3a29181716151413121110f0e0d0c0b0a09080";

    fn pubky_of(secret_key: &[u8; 32]) -> String {
        let key = SigningKey::from_bytes(secret_key);
        base32_encode(Alphabet::Z, key.verifying_key().as_bytes())
    }

    fn profile() -> PubkyMarketAttestorProfile {
        PubkyMarketAttestorProfile {
            operator: PubkyMarketAttestorOperator {
                name: "Pubky Market".to_string(),
                url: "https://pubky-market.example".to_string(),
                policy_url: "https://pubky-market.example/attestor-policy".to_string(),
            },
            keys: vec![
                PubkyMarketAttestorKey {
                    pubky: pubky_of(&SECRET_KEY),
                    valid_from: "2026-08-01T00:00:00Z".to_string(),
                    valid_until: None,
                    compromised_from: None,
                },
                PubkyMarketAttestorKey {
                    pubky: pubky_of(&OTHER_SECRET_KEY),
                    valid_from: "2026-01-01T00:00:00Z".to_string(),
                    valid_until: Some("2026-08-01T00:00:00Z".to_string()),
                    compromised_from: Some("2026-07-15T00:00:00Z".to_string()),
                },
            ],
            successor: None,
            issuance_policy: PubkyMarketAttestorIssuancePolicy {
                edit_window_hours: 24,
                blind_window_days: 14,
                amount_band: AMOUNT_BAND_POLICY_BOTH_SIDES_CONSENT.to_string(),
                price_decade: PRICE_DECADE_POLICY_ALWAYS.to_string(),
                min_order_value: None,
            },
            proof: String::new(),
        }
    }

    fn annotation() -> PubkyMarketAttestorAnnotation {
        PubkyMarketAttestorAnnotation {
            order_ref: ORDER_REF.to_string(),
            outcomes: vec![
                PubkyMarketAttestorOutcome {
                    kind: PubkyMarketAttestorOutcomeKind::Returned,
                    on: "2026-10-12".to_string(),
                },
                PubkyMarketAttestorOutcome {
                    kind: PubkyMarketAttestorOutcomeKind::Refunded,
                    on: "2026-10-14".to_string(),
                },
            ],
            revision: 2,
            proof: String::new(),
        }
    }

    fn profile_uri(secret_key: &[u8; 32]) -> String {
        attestor_profile_uri_builder(pubky_of(secret_key))
    }

    fn annotation_uri(secret_key: &[u8; 32], order_ref: &str) -> String {
        attestor_annotation_uri_builder(pubky_of(secret_key), order_ref.to_string())
    }

    #[test]
    fn test_profile_proof_round_trip() {
        let signed = profile().sign(&SECRET_KEY).unwrap();
        let json = signed.to_canonical_json().unwrap();
        let loaded = PubkyMarketAttestorProfile::from_verified_json(
            &profile_uri(&SECRET_KEY),
            json.as_bytes(),
        )
        .unwrap();
        assert_eq!(loaded, signed);

        // Member order and whitespace in the fetched bytes do not matter.
        let pretty = serde_json::to_string_pretty(&signed).unwrap();
        assert!(PubkyMarketAttestorProfile::from_verified_json(
            &profile_uri(&SECRET_KEY),
            pretty.as_bytes()
        )
        .is_ok());
    }

    #[test]
    fn test_annotation_proof_round_trip() {
        let signed = annotation().sign(&SECRET_KEY).unwrap();
        let json = signed.to_canonical_json().unwrap();
        let loaded = PubkyMarketAttestorAnnotation::from_verified_json(
            &annotation_uri(&SECRET_KEY, ORDER_REF),
            json.as_bytes(),
        )
        .unwrap();
        assert_eq!(loaded, signed);
    }

    #[test]
    fn test_wrong_signer_fails() {
        // Signed by another key but served from this attestor's homeserver.
        let profile = profile().sign(&OTHER_SECRET_KEY).unwrap();
        let error = PubkyMarketAttestorProfile::from_verified_json(
            &profile_uri(&SECRET_KEY),
            profile.to_canonical_json().unwrap().as_bytes(),
        )
        .unwrap_err();
        assert!(error.contains("does not verify"), "{error}");

        // An honest annotation copied to another homeserver.
        let annotation = annotation().sign(&SECRET_KEY).unwrap();
        let error = PubkyMarketAttestorAnnotation::from_verified_json(
            &annotation_uri(&OTHER_SECRET_KEY, ORDER_REF),
            annotation.to_canonical_json().unwrap().as_bytes(),
        )
        .unwrap_err();
        assert!(error.contains("does not verify"), "{error}");
    }

    #[test]
    fn test_wrong_path_fails() {
        let annotation = annotation().sign(&SECRET_KEY).unwrap();
        let json = annotation.to_canonical_json().unwrap();

        // Moved to another order's path.
        let other_order = "a".repeat(64);
        let error = PubkyMarketAttestorAnnotation::from_verified_json(
            &annotation_uri(&SECRET_KEY, &other_order),
            json.as_bytes(),
        )
        .unwrap_err();
        assert!(error.contains("orderRef"), "{error}");

        // Served at the profile path, either way round.
        assert!(PubkyMarketAttestorAnnotation::from_verified_json(
            &profile_uri(&SECRET_KEY),
            json.as_bytes()
        )
        .is_err());
        assert!(PubkyMarketAttestorProfile::from_verified_json(
            &profile_uri(&SECRET_KEY),
            json.as_bytes()
        )
        .is_err());
        let profile = profile().sign(&SECRET_KEY).unwrap();
        assert!(PubkyMarketAttestorAnnotation::from_verified_json(
            &annotation_uri(&SECRET_KEY, ORDER_REF),
            profile.to_canonical_json().unwrap().as_bytes()
        )
        .is_err());
    }

    #[test]
    fn test_typ_separates_the_records() {
        // A profile-typed proof over an annotation body is refused even at
        // the right path and with the right key.
        let mut annotation = annotation();
        annotation.proof = sign_proof(
            ATTESTOR_PROFILE_TYP,
            &to_object(&annotation).unwrap(),
            &SECRET_KEY,
        )
        .unwrap();
        let error = PubkyMarketAttestorAnnotation::from_verified_json(
            &annotation_uri(&SECRET_KEY, ORDER_REF),
            annotation.to_canonical_json().unwrap().as_bytes(),
        )
        .unwrap_err();
        assert!(error.contains("typ"), "{error}");
    }

    #[test]
    fn test_tampered_body_fails() {
        let signed = annotation().sign(&SECRET_KEY).unwrap();
        let mut value = serde_json::to_value(&signed).unwrap();
        value["revision"] = serde_json::json!(3);
        let error = PubkyMarketAttestorAnnotation::from_verified_json(
            &annotation_uri(&SECRET_KEY, ORDER_REF),
            value.to_string().as_bytes(),
        )
        .unwrap_err();
        assert!(error.contains("does not verify"), "{error}");
    }

    #[test]
    fn test_unknown_members_are_covered_by_the_proof() {
        let mut value = serde_json::to_value(profile()).unwrap();
        value["surprise"] = serde_json::json!({ "b": 1, "a": [true, null] });
        let proof = sign_proof(ATTESTOR_PROFILE_TYP, &value, &SECRET_KEY).unwrap();
        value["proof"] = Value::String(proof);
        assert!(PubkyMarketAttestorProfile::from_verified_json(
            &profile_uri(&SECRET_KEY),
            value.to_string().as_bytes()
        )
        .is_ok());

        value["surprise"]["b"] = serde_json::json!(2);
        assert!(PubkyMarketAttestorProfile::from_verified_json(
            &profile_uri(&SECRET_KEY),
            value.to_string().as_bytes()
        )
        .is_err());
    }

    #[test]
    fn test_missing_or_malformed_proof_fails() {
        let mut value = serde_json::to_value(annotation()).unwrap();
        value.as_object_mut().unwrap().remove("proof");
        let uri = annotation_uri(&SECRET_KEY, ORDER_REF);
        assert!(PubkyMarketAttestorAnnotation::from_verified_json(
            &uri,
            value.to_string().as_bytes()
        )
        .is_err());

        let signed = annotation().sign(&SECRET_KEY).unwrap();
        let (header, signature) = split_proof(&signed.proof).unwrap();
        let body = base64url_encode(signed.canonical_body().unwrap().as_bytes());
        for proof in [
            String::new(),
            format!("{header}.{body}.{signature}"),
            format!("{header}.{signature}"),
            format!("{header}..{signature}.x"),
            format!("{header}..{}", &signature[1..]),
        ] {
            let mut record = signed.clone();
            record.proof = proof.clone();
            let json = serde_json::to_string(&record).unwrap();
            assert!(
                PubkyMarketAttestorAnnotation::from_verified_json(&uri, json.as_bytes()).is_err(),
                "{proof}"
            );
        }
    }

    #[test]
    fn test_canonical_body_sorts_members_and_drops_proof() {
        let value = serde_json::json!({
            "b": [1, {"z": "é\n", "a": null}],
            "a": true,
            "proof": "x",
            "\u{fb01}": 1,
            "\u{1d11e}": 2
        });
        assert_eq!(
            canonical_body(&value).unwrap(),
            "{\"a\":true,\"b\":[1,{\"a\":null,\"z\":\"é\\n\"}],\"\u{1d11e}\":2,\"\u{fb01}\":1}"
        );
        assert!(canonical_body(&serde_json::json!({ "x": 1.5 })).is_err());
        assert!(canonical_body(&serde_json::json!([1])).is_err());
    }

    #[test]
    fn test_profile_field_rules() {
        let cases: Vec<fn(&mut PubkyMarketAttestorProfile)> = vec![
            |p| p.operator.name = String::new(),
            |p| p.operator.name = " Pubky Market".to_string(),
            |p| p.operator.url = "http://pubky-market.example".to_string(),
            |p| p.operator.policy_url = "not a url".to_string(),
            |p| p.keys.clear(),
            |p| p.keys[1].pubky = p.keys[0].pubky.clone(),
            |p| p.keys[0].pubky = "not-a-pubky".to_string(),
            |p| p.keys[0].valid_from = "2026-08-01".to_string(),
            |p| p.keys[1].valid_until = Some("2025-12-31T00:00:00Z".to_string()),
            |p| p.keys[1].compromised_from = Some("2025-12-31T00:00:00Z".to_string()),
            |p| p.successor = Some("not-a-pubky".to_string()),
            |p| p.issuance_policy.edit_window_hours = -1,
            |p| p.issuance_policy.blind_window_days = 366,
            |p| p.issuance_policy.amount_band = String::new(),
            |p| p.issuance_policy.price_decade = "x".repeat(65),
            |p| {
                p.issuance_policy.min_order_value = Some(PubkyMarketMoney {
                    amount_minor: 0,
                    currency: "USD".to_string(),
                    exponent: 2,
                })
            },
        ];
        for (index, case) in cases.into_iter().enumerate() {
            let mut record = profile();
            case(&mut record);
            assert!(record.sign(&SECRET_KEY).is_err(), "case {index}");
        }

        let mut record = profile();
        record.successor = Some(pubky_of(&OTHER_SECRET_KEY));
        record.issuance_policy.min_order_value = Some(PubkyMarketMoney {
            amount_minor: 500,
            currency: "USD".to_string(),
            exponent: 2,
        });
        assert!(record.sign(&SECRET_KEY).is_ok());
    }

    #[test]
    fn test_annotation_field_rules() {
        let cases: Vec<fn(&mut PubkyMarketAttestorAnnotation)> = vec![
            |a| a.order_ref = "ABC".to_string(),
            |a| a.outcomes[0].on = "2026-02-30".to_string(),
            |a| a.outcomes[0].on = "2026-10-12T00:00:00Z".to_string(),
            |a| a.revision = 0,
            |a| {
                a.outcomes = (0..=MAX_ANNOTATION_OUTCOMES)
                    .map(|_| a.outcomes[0].clone())
                    .collect()
            },
        ];
        for (index, case) in cases.into_iter().enumerate() {
            let mut record = annotation();
            case(&mut record);
            assert!(record.sign(&SECRET_KEY).is_err(), "case {index}");
        }

        let unknown_kind = serde_json::json!({
            "orderRef": ORDER_REF,
            "outcomes": [{ "kind": "refund_reversal_cancelled", "on": "2026-10-12" }],
            "revision": 1,
            "proof": ""
        });
        assert!(serde_json::from_value::<PubkyMarketAttestorAnnotation>(unknown_kind).is_err());

        let mut empty = annotation();
        empty.outcomes.clear();
        assert!(empty.sign(&SECRET_KEY).is_ok());
    }

    #[test]
    fn test_min_order_value_serializes_as_null() {
        let value = serde_json::to_value(profile()).unwrap();
        assert_eq!(value["issuancePolicy"]["minOrderValue"], Value::Null);
        assert!(value.get("successor").is_none());
    }

    #[test]
    fn test_uris_parse() {
        let attestor = pubky_of(&SECRET_KEY);

        let uri = attestor_profile_uri_builder(attestor.clone());
        assert_eq!(
            uri,
            format!("pubky://{attestor}/pub/pubky.app/marketplace/v1/attestor/profile")
        );
        let parsed = ParsedUri::try_from(uri.as_str()).unwrap();
        assert_eq!(parsed.resource, Resource::AttestorProfile);
        assert_eq!(parsed.resource.id(), None);
        assert_eq!(
            parsed.resource.to_string(),
            "marketplace/v1/attestor/profile"
        );
        assert_eq!(parsed.try_to_uri_str().unwrap(), uri);

        let uri = attestor_annotation_uri_builder(attestor.clone(), ORDER_REF.to_string());
        assert_eq!(
            uri,
            format!(
                "pubky://{attestor}/pub/pubky.app/marketplace/v1/attestor/annotations/{ORDER_REF}"
            )
        );
        let parsed = ParsedUri::try_from(uri.as_str()).unwrap();
        assert_eq!(
            parsed.resource,
            Resource::AttestorAnnotation(ORDER_REF.to_string())
        );
        assert_eq!(parsed.resource.id(), Some(ORDER_REF.to_string()));
        assert_eq!(
            parsed.resource.to_string(),
            "marketplace/v1/attestor/annotations"
        );
        assert_eq!(parsed.try_to_uri_str().unwrap(), uri);

        for path in [
            "marketplace/v1/attestor/annotations/",
            "marketplace/v1/attestor/profile/extra",
            "marketplace/v1/attestor/annotations/abc/extra",
            "marketplace/v1/attestor/other",
        ] {
            let uri = format!("pubky://{attestor}/pub/pubky.app/{path}");
            let parsed = ParsedUri::try_from(uri.as_str()).unwrap();
            assert_eq!(parsed.resource, Resource::Unknown, "{path}");
        }
    }

    #[test]
    fn test_pubky_app_object_verifies_attestor_records() {
        let profile = profile().sign(&SECRET_KEY).unwrap();
        let json = profile.to_canonical_json().unwrap();
        match PubkyMarketObject::from_uri(profile_uri(&SECRET_KEY), json.as_bytes()).unwrap() {
            PubkyMarketObject::AttestorProfile(loaded) => assert_eq!(loaded, profile),
            other => panic!("expected an attestor profile, got {other:?}"),
        }
        assert!(
            PubkyMarketObject::from_uri(profile_uri(&OTHER_SECRET_KEY), json.as_bytes()).is_err()
        );
        assert!(
            PubkyMarketObject::from_resource(&Resource::AttestorProfile, json.as_bytes()).is_err()
        );

        let annotation = annotation().sign(&SECRET_KEY).unwrap();
        let json = annotation.to_canonical_json().unwrap();
        match PubkyMarketObject::from_uri(annotation_uri(&SECRET_KEY, ORDER_REF), json.as_bytes())
            .unwrap()
        {
            PubkyMarketObject::AttestorAnnotation(loaded) => assert_eq!(loaded, annotation),
            other => panic!("expected an attestor annotation, got {other:?}"),
        }
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct VectorRecord {
        uri: String,
        canonical_body: String,
        proof: String,
        record: Value,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Vectors {
        #[allow(dead_code)]
        description: String,
        attestor_secret_key_hex: String,
        attestor_pubky: String,
        profile: VectorRecord,
        annotation: VectorRecord,
    }

    #[test]
    fn test_canonical_vectors() {
        let vectors: Vectors =
            serde_json::from_str(include_str!("../test_fixtures/attestor-records-v1.json"))
                .expect("vector fixture parses");
        let secret_key_hex: String = SECRET_KEY.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(vectors.attestor_secret_key_hex, secret_key_hex);
        assert_eq!(vectors.attestor_pubky, pubky_of(&SECRET_KEY));

        let profile = profile().sign(&SECRET_KEY).unwrap();
        assert_eq!(vectors.profile.uri, profile_uri(&SECRET_KEY));
        assert_eq!(
            vectors.profile.canonical_body,
            profile.canonical_body().unwrap()
        );
        assert_eq!(vectors.profile.proof, profile.proof);
        assert_eq!(
            vectors.profile.record,
            serde_json::to_value(&profile).unwrap()
        );
        assert_eq!(
            PubkyMarketAttestorProfile::from_verified_json(
                &vectors.profile.uri,
                vectors.profile.record.to_string().as_bytes()
            )
            .unwrap(),
            profile
        );

        let annotation = annotation().sign(&SECRET_KEY).unwrap();
        assert_eq!(
            vectors.annotation.uri,
            annotation_uri(&SECRET_KEY, ORDER_REF)
        );
        assert_eq!(
            vectors.annotation.canonical_body,
            annotation.canonical_body().unwrap()
        );
        assert_eq!(vectors.annotation.proof, annotation.proof);
        assert_eq!(
            vectors.annotation.record,
            serde_json::to_value(&annotation).unwrap()
        );
        assert_eq!(
            PubkyMarketAttestorAnnotation::from_verified_json(
                &vectors.annotation.uri,
                vectors.annotation.record.to_string().as_bytes()
            )
            .unwrap(),
            annotation
        );
    }
}
