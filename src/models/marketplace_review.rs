use crate::{
    models::marketplace::{
        validate_base_record, validate_entity_id, validate_hex_hash, validate_pubky,
    },
    models::marketplace_attestation::PubkyMarketPurchaseAttestation,
    traits::{HasIdPath, HashId, Validatable},
    APP_PATH, PUBLIC_PATH,
};
use serde::{Deserialize, Serialize};
use std::fmt;

#[cfg(target_arch = "wasm32")]
use crate::traits::Json;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

// Validation (mirroring the pubky-app commerce review record schema)
const REVIEW_RECORD_TYPE: &str = "review";
const MAX_REVIEW_TEXT_LENGTH: usize = 5_000;
const MIN_RATING: i64 = 1;
const MAX_RATING: i64 = 5;
const MIN_ATTESTATION_LENGTH: usize = 32;
const MAX_ATTESTATION_LENGTH: usize = 4_096;

/// Which side of the trade the review covers.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub enum PubkyMarketReviewRole {
    BuyerReviewingSeller,
    SellerReviewingBuyer,
}

impl PubkyMarketReviewRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            PubkyMarketReviewRole::BuyerReviewingSeller => "buyer_reviewing_seller",
            PubkyMarketReviewRole::SellerReviewingBuyer => "seller_reviewing_buyer",
        }
    }
}

/// Star ratings on a 1-5 integer scale. Only `overall` is required.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketReviewRatings {
    pub overall: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_accuracy: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub communication: Option<i64>,
}

impl PubkyMarketReviewRatings {
    fn validate(&self) -> Result<(), String> {
        let all_ratings = [
            Some(self.overall),
            self.item_accuracy,
            self.shipping,
            self.communication,
        ];
        for rating in all_ratings.into_iter().flatten() {
            if !(MIN_RATING..=MAX_RATING).contains(&rating) {
                return Err(format!(
                    "Validation Error: ratings must be integers between {MIN_RATING} and {MAX_RATING}"
                ));
            }
        }
        Ok(())
    }
}

/// Counting key of one item review; see
/// [`PubkyMarketReview::item_count_key`]. Its `Display` form joins the
/// parts with `:` (no part can contain one).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketItemCountKey {
    pub iss: String,
    pub order_ref: String,
    pub role: String,
    pub listing_owner: String,
    pub listing_id: String,
}

impl PubkyMarketItemCountKey {
    /// The seller rating key of the order this item belongs to.
    pub fn seller_rating_key(&self) -> PubkyMarketSellerRatingKey {
        PubkyMarketSellerRatingKey {
            iss: self.iss.clone(),
            order_ref: self.order_ref.clone(),
            role: self.role.clone(),
        }
    }
}

impl fmt::Display for PubkyMarketItemCountKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}:{}:{}",
            self.iss, self.order_ref, self.role, self.listing_owner, self.listing_id
        )
    }
}

/// Seller rating key of one order and role; see
/// [`PubkyMarketReview::seller_rating_key`]. Its `Display` form joins
/// the parts with `:`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketSellerRatingKey {
    pub iss: String,
    pub order_ref: String,
    pub role: String,
}

impl fmt::Display for PubkyMarketSellerRatingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.iss, self.order_ref, self.role)
    }
}

/// Represents a marketplace review of a trade counterparty.
///
/// URI: /pub/pubky.app/marketplace/v1/reviews/:review_id
///
/// Example URI:
///
/// `/pub/pubky.app/marketplace/v1/reviews/FPB0AM9S93Q3M1GFY1KV09GMQM`
///
/// Where review_id is
/// Crockford-base32(Blake3("{reviewed_listing_uri}:{subject_pubky}:{role}:{order_ref}")[:half])
/// and must match the record's `reviewId` field. A record without `orderRef`
/// (written before per-order reviews) keeps the three-part
/// `"{reviewed_listing_uri}:{subject_pubky}:{role}"` input.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketReview {
    /// Marketplace contract version, always `1`.
    pub schema_version: i64,
    /// Record discriminator, always `"review"`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub record_type: String,
    /// z-base-32 pubky of the reviewer.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub owner_pubky: String,
    /// Monotonically increasing record revision, starting at 1.
    pub revision: i64,
    /// ISO-8601 creation datetime with offset.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub created_at: String,
    /// ISO-8601 last-update datetime with offset. Must not precede `created_at`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub updated_at: String,
    /// Must match the hash ID in the record's path.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub review_id: String,
    /// z-base-32 pubky of the user being reviewed.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub subject_pubky: String,
    /// z-base-32 pubky of the seller who owns the reviewed listing.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub listing_owner_pubky: String,
    /// Identifier of the reviewed listing.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub listing_id: String,
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub role: PubkyMarketReviewRole,
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub ratings: PubkyMarketReviewRatings,
    /// Optional review text, 0-5000 characters after trimming. A missing
    /// `text` member reads as empty.
    #[serde(default)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub text: String,
    /// Opaque proof that the reviewer is eligible to review this trade.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub eligibility_attestation: String,
    /// The attested order's `order_ref` (the embedded attestation's claim).
    /// Part of the hash ID, so each attested order gets its own review.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub order_ref: Option<String>,
}

impl PubkyMarketReview {
    /// True when the record carries `orderRef`, so its ID is per attested order.
    /// When a scoped record and a legacy one (no `orderRef`) carry attestations
    /// with the same `(iss, order_ref)` and the same role, the scoped record
    /// counts and the legacy one is left out of counts, averages and lists.
    pub fn is_order_scoped(&self) -> bool {
        self.order_ref.is_some()
    }

    /// The item counting key `(iss, order_ref, role, listing_owner, listing_id)`:
    /// one counted review per attested order, role and reviewed listing, so
    /// each item of a multi-item order counts on its own. `iss` and
    /// `order_ref` come from the embedded attestation, which must bind this
    /// record; a record without `orderRef` keys by the attestation's
    /// `order_ref`, so it shares its key with a scoped twin of the same item.
    /// Signature verification and trust in `iss` stay with the caller.
    pub fn item_count_key(
        &self,
        attestation: &PubkyMarketPurchaseAttestation,
    ) -> Result<PubkyMarketItemCountKey, String> {
        attestation.verify_binding(self)?;
        Ok(PubkyMarketItemCountKey {
            iss: attestation.claims.iss.clone(),
            order_ref: attestation.claims.order_ref.clone(),
            role: self.role.as_str().to_string(),
            listing_owner: self.listing_owner_pubky.clone(),
            listing_id: self.listing_id.clone(),
        })
    }

    /// The seller rating key `(iss, order_ref, role)`: `shipping` and
    /// `communication` are written identically on every item review of an
    /// order and count once per key. When the copies differ, the record with
    /// the latest `updatedAt` wins; ties go to the lowest `reviewId`.
    pub fn seller_rating_key(
        &self,
        attestation: &PubkyMarketPurchaseAttestation,
    ) -> Result<PubkyMarketSellerRatingKey, String> {
        self.item_count_key(attestation)
            .map(|key| key.seller_rating_key())
    }

    /// Creates a new `PubkyMarketReview` instance and sanitizes it.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        owner_pubky: String,
        revision: i64,
        created_at: String,
        updated_at: String,
        review_id: String,
        subject_pubky: String,
        listing_owner_pubky: String,
        listing_id: String,
        role: PubkyMarketReviewRole,
        ratings: PubkyMarketReviewRatings,
        text: String,
        eligibility_attestation: String,
    ) -> Self {
        Self {
            schema_version: crate::models::marketplace::MARKETPLACE_SCHEMA_VERSION,
            record_type: REVIEW_RECORD_TYPE.to_string(),
            owner_pubky,
            revision,
            created_at,
            updated_at,
            review_id,
            subject_pubky,
            listing_owner_pubky,
            listing_id,
            role,
            ratings,
            text,
            eligibility_attestation,
            order_ref: None,
        }
        .sanitize()
    }
}

#[cfg(target_arch = "wasm32")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl PubkyMarketReview {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = fromJson))]
    pub fn from_json(js_value: &JsValue) -> Result<Self, String> {
        Self::import_json(js_value)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = toJson))]
    pub fn to_json(&self) -> Result<JsValue, String> {
        self.export_json()
    }

    /// Getter for `text`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter))]
    pub fn text(&self) -> String {
        self.text.clone()
    }

    /// Getter for `listing_id`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter))]
    pub fn listing_id(&self) -> String {
        self.listing_id.clone()
    }

    /// Getter for `subject_pubky`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter))]
    pub fn subject_pubky(&self) -> String {
        self.subject_pubky.clone()
    }
}

#[cfg(target_arch = "wasm32")]
impl Json for PubkyMarketReview {}

impl HasIdPath for PubkyMarketReview {
    const PATH_SEGMENT: &'static str = "marketplace/v1/reviews/";

    fn create_path(id: &str) -> String {
        [PUBLIC_PATH, APP_PATH, Self::PATH_SEGMENT, id].concat()
    }
}

impl HashId for PubkyMarketReview {
    /// Review ID is created based on the hash of the reviewed listing URI,
    /// the reviewed subject, the review role and, when present, the attested
    /// order's `order_ref`.
    fn get_id_data(&self) -> String {
        let listing_uri =
            crate::listing_uri_builder(self.listing_owner_pubky.clone(), self.listing_id.clone());
        let base = format!(
            "{}:{}:{}",
            listing_uri,
            self.subject_pubky,
            self.role.as_str()
        );
        match &self.order_ref {
            Some(order_ref) => format!("{base}:{order_ref}"),
            None => base,
        }
    }
}

impl Validatable for PubkyMarketReview {
    fn sanitize(self) -> Self {
        PubkyMarketReview {
            text: self.text.trim().to_string(),
            ..self
        }
    }

    fn validate(&self, id: Option<&str>) -> Result<(), String> {
        // Validate the review ID (hash regeneration) and its match with the record
        if let Some(id) = id {
            self.validate_id(id)?;
            if self.review_id != id {
                return Err("Validation Error: reviewId does not match the review path ID".into());
            }
        }

        validate_base_record(
            self.schema_version,
            &self.record_type,
            REVIEW_RECORD_TYPE,
            &self.owner_pubky,
            self.revision,
            &self.created_at,
            &self.updated_at,
        )?;
        validate_entity_id(&self.review_id, "reviewId")?;
        validate_pubky(&self.subject_pubky, "subjectPubky")?;
        validate_pubky(&self.listing_owner_pubky, "listingOwnerPubky")?;
        validate_entity_id(&self.listing_id, "listingId")?;
        if let Some(order_ref) = &self.order_ref {
            validate_hex_hash(order_ref, "orderRef")?;
        }

        self.ratings.validate()?;

        if self.text.chars().count() > MAX_REVIEW_TEXT_LENGTH {
            return Err(format!(
                "Validation Error: review text must be 0-{MAX_REVIEW_TEXT_LENGTH} characters"
            ));
        }

        let attestation_length = self.eligibility_attestation.chars().count();
        if !(MIN_ATTESTATION_LENGTH..=MAX_ATTESTATION_LENGTH).contains(&attestation_length) {
            return Err(format!(
                "Validation Error: eligibilityAttestation must be {MIN_ATTESTATION_LENGTH}-{MAX_ATTESTATION_LENGTH} characters"
            ));
        }
        if !self
            .eligibility_attestation
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._~-".contains(c))
        {
            return Err(
                "Validation Error: eligibilityAttestation must only contain characters [A-Za-z0-9._~-]"
                    .into(),
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::Validatable;

    const REVIEWER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
    const SELLER: &str = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";

    fn valid_review() -> PubkyMarketReview {
        let mut review = PubkyMarketReview::new(
            REVIEWER.to_string(),
            1,
            "2025-01-01T00:00:00Z".to_string(),
            "2025-01-01T00:00:00Z".to_string(),
            String::new(), // assigned below from the generated hash ID
            SELLER.to_string(),
            SELLER.to_string(),
            "0032SSN7Q4EVG".to_string(),
            PubkyMarketReviewRole::BuyerReviewingSeller,
            PubkyMarketReviewRatings {
                overall: 5,
                item_accuracy: Some(5),
                shipping: Some(4),
                communication: None,
            },
            "Great seller, fast shipping.".to_string(),
            "a".repeat(64),
        );
        review.review_id = review.create_id();
        review
    }

    #[test]
    fn test_create_id_is_deterministic() {
        let review = valid_review();
        assert_eq!(review.create_id(), review.create_id());

        let mut other = valid_review();
        other.role = PubkyMarketReviewRole::SellerReviewingBuyer;
        assert_ne!(review.create_id(), other.create_id());

        let mut other_listing = valid_review();
        other_listing.listing_id = "0032SSN7Q4EVH".to_string();
        assert_ne!(review.create_id(), other_listing.create_id());
    }

    #[test]
    fn test_order_ref_gives_each_order_its_own_id() {
        let legacy = valid_review();
        let mut first = valid_review();
        first.order_ref = Some("1".repeat(64));
        let mut second = valid_review();
        second.order_ref = Some("2".repeat(64));

        assert_ne!(first.create_id(), legacy.create_id());
        assert_ne!(first.create_id(), second.create_id());

        first.review_id = first.create_id();
        let id = first.review_id.clone();
        assert!(first.validate(Some(&id)).is_ok());
        // The ID binds the order: a record moved to another order's path fails.
        assert!(first.validate(Some(&second.create_id())).is_err());
    }

    #[test]
    fn test_record_without_order_ref_keeps_its_legacy_id() {
        let review = valid_review();
        let json = serde_json::to_string(&review).unwrap();
        assert!(!json.contains("orderRef"));
        let id = review.review_id.clone();
        let parsed = <PubkyMarketReview as Validatable>::try_from(json.as_bytes(), &id).unwrap();
        assert_eq!(parsed.order_ref, None);
    }

    #[test]
    fn test_order_ref_must_be_a_hex_hash() {
        let mut review = valid_review();
        review.order_ref = Some("not-a-hash".to_string());
        review.review_id = review.create_id();
        let id = review.review_id.clone();
        assert!(review.validate(Some(&id)).is_err());
    }

    #[test]
    fn test_create_path() {
        let review = valid_review();
        let path = PubkyMarketReview::create_path(&review.review_id);
        assert_eq!(
            path,
            format!("/pub/pubky.app/marketplace/v1/reviews/{}", review.review_id)
        );
    }

    #[test]
    fn test_validate_valid() {
        let review = valid_review();
        let id = review.review_id.clone();
        assert!(review.validate(Some(&id)).is_ok());
    }

    #[test]
    fn test_try_from_valid_roundtrip() {
        let review = valid_review();
        let id = review.review_id.clone();
        let json = serde_json::to_string(&review).unwrap();
        let parsed = <PubkyMarketReview as Validatable>::try_from(json.as_bytes(), &id).unwrap();
        assert_eq!(parsed, review);
    }

    #[test]
    fn test_try_from_accepts_unknown_field() {
        let review = valid_review();
        let id = review.review_id.clone();
        let mut value = serde_json::to_value(&review).unwrap();
        value["surprise"] = serde_json::json!(true);
        let json = serde_json::to_string(&value).unwrap();
        assert!(<PubkyMarketReview as Validatable>::try_from(json.as_bytes(), &id).is_ok());
    }

    #[test]
    fn test_validate_wrong_id() {
        let review = valid_review();
        assert!(review.validate(Some("WRONGID123456789012345678")).is_err());
    }

    #[test]
    fn test_validate_review_id_mismatch() {
        let mut review = valid_review();
        let id = review.review_id.clone();
        review.review_id = "different_id".to_string();
        // The hash is derived from target fields, not review_id, so the path
        // ID still regenerates; the mismatch against the record must fail.
        assert!(review.validate(Some(&id)).is_err());
    }

    #[test]
    fn test_validate_rating_out_of_range() {
        let mut review = valid_review();
        review.ratings.overall = 0;
        assert!(review.validate(None).is_err());

        let mut review = valid_review();
        review.ratings.shipping = Some(6);
        assert!(review.validate(None).is_err());
    }

    #[test]
    fn test_validate_empty_text() {
        let mut review = valid_review();
        review.text = String::new();
        assert!(review.validate(None).is_ok());

        let mut review = valid_review();
        review.text = "   ".to_string();
        let review = review.sanitize();
        assert_eq!(review.text, "");
        assert!(review.validate(None).is_ok());
    }

    #[test]
    fn test_missing_text_reads_as_empty() {
        let review = valid_review();
        let id = review.review_id.clone();
        let mut value = serde_json::to_value(&review).unwrap();
        value.as_object_mut().unwrap().remove("text");
        let json = serde_json::to_string(&value).unwrap();
        let parsed = <PubkyMarketReview as Validatable>::try_from(json.as_bytes(), &id).unwrap();
        assert_eq!(parsed.text, "");
    }

    #[test]
    fn test_validate_text_length_limit() {
        let mut review = valid_review();
        review.text = "a".repeat(MAX_REVIEW_TEXT_LENGTH);
        assert!(review.validate(None).is_ok());

        review.text = "a".repeat(MAX_REVIEW_TEXT_LENGTH + 1);
        assert!(review.validate(None).is_err());

        // The limit counts characters, not bytes.
        review.text = "é".repeat(MAX_REVIEW_TEXT_LENGTH);
        assert!(review.validate(None).is_ok());
    }

    #[test]
    fn test_spec_14_record_still_validates() {
        // A record as `.14` wrote it: per-order ID, non-empty text.
        let json = format!(
            r#"{{
                "schemaVersion": 1,
                "recordType": "review",
                "ownerPubky": "{REVIEWER}",
                "revision": 2,
                "createdAt": "2026-10-01T09:00:00Z",
                "updatedAt": "2026-10-02T09:00:00Z",
                "reviewId": "",
                "subjectPubky": "{SELLER}",
                "listingOwnerPubky": "{SELLER}",
                "listingId": "0032SSN7Q4EVG",
                "role": "buyer_reviewing_seller",
                "ratings": {{ "overall": 4, "shipping": 5 }},
                "text": "Arrived well packed.",
                "eligibilityAttestation": "{}",
                "orderRef": "{}"
            }}"#,
            "a".repeat(64),
            "b".repeat(64)
        );
        let mut record: PubkyMarketReview = serde_json::from_str(&json).unwrap();
        record.review_id = record.create_id();
        let id = record.review_id.clone();
        let json = serde_json::to_string(&record).unwrap();
        let parsed = <PubkyMarketReview as Validatable>::try_from(json.as_bytes(), &id).unwrap();
        assert_eq!(parsed.text, "Arrived well packed.");
        assert_eq!(parsed.order_ref, Some("b".repeat(64)));
    }

    #[test]
    fn test_validate_attestation_rules() {
        let mut review = valid_review();
        review.eligibility_attestation = "too-short".to_string();
        assert!(review.validate(None).is_err());

        let mut review = valid_review();
        review.eligibility_attestation = format!("{}!", "a".repeat(40));
        assert!(review.validate(None).is_err());
    }

    #[test]
    fn test_validate_invalid_subject() {
        let mut review = valid_review();
        review.subject_pubky = "not-a-pubky".to_string();
        assert!(review.validate(None).is_err());
    }

    #[test]
    fn test_sanitize_trims_text() {
        let mut review = valid_review();
        review.text = "  Great seller.  ".to_string();
        let review = review.sanitize();
        assert_eq!(review.text, "Great seller.");
    }
}
