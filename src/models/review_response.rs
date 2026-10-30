use crate::{
    models::marketplace::{validate_base_record, validate_entity_id},
    traits::{HasIdPath, Validatable},
    uri::{ParsedUri, Resource},
    APP_PATH, PUBLIC_PATH,
};
use serde::{Deserialize, Serialize};

#[cfg(target_arch = "wasm32")]
use crate::traits::Json;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

const REVIEW_RESPONSE_RECORD_TYPE: &str = "review_response";
const MAX_RESPONSE_TEXT_LENGTH: usize = 5_000;

/// Represents a response to a marketplace review, published by the review's
/// subject on their own homeserver.
///
/// URI: /pub/pubky.app/marketplace/v1/review_responses/:review_id
///
/// The path ID **equals the subject review's ID**, which gives O(1) lookup
/// in both directions and structurally caps responses at one per review
/// (revisable via `revision`).
///
/// Authorization is structural, not cryptographic: an indexer accepts a
/// response only when the response record's `owner_pubky` equals the
/// subject review's `subjectPubky` (see [`Self::is_authorized_response_to`]).
/// An impostor's response fails that check without any signature machinery.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketReviewResponse {
    /// Marketplace contract version, always `1`.
    pub schema_version: i64,
    /// Record discriminator, always `"review_response"`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub record_type: String,
    /// z-base-32 pubky of the responder (the review's subject).
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
    /// The subject review's ID; must equal the ID in the record's path.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub review_id: String,
    /// Full canonical URI of the subject review on the reviewer's homeserver.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub review_uri: String,
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub text: String,
}

impl PubkyMarketReviewResponse {
    /// Creates a new `PubkyMarketReviewResponse` instance and sanitizes it.
    pub fn new(
        owner_pubky: String,
        revision: i64,
        created_at: String,
        updated_at: String,
        review_id: String,
        review_uri: String,
        text: String,
    ) -> Self {
        Self {
            schema_version: crate::models::marketplace::MARKETPLACE_SCHEMA_VERSION,
            record_type: REVIEW_RESPONSE_RECORD_TYPE.to_string(),
            owner_pubky,
            revision,
            created_at,
            updated_at,
            review_id,
            review_uri,
            text,
        }
        .sanitize()
    }

    /// The structural authorization rule indexers apply: a response is
    /// accepted only when its owner is the subject of the review it
    /// responds to, its ID equals the review's ID, and its `reviewUri`
    /// names that exact review on the reviewer's homeserver.
    pub fn is_authorized_response_to(&self, review: &crate::PubkyMarketReview) -> bool {
        self.owner_pubky == review.subject_pubky
            && self.review_id == review.review_id
            && self.review_uri
                == crate::marketplace_review_uri_builder(
                    review.owner_pubky.clone(),
                    review.review_id.clone(),
                )
    }
}

#[cfg(target_arch = "wasm32")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl PubkyMarketReviewResponse {
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

    /// Getter for `review_id`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter))]
    pub fn review_id(&self) -> String {
        self.review_id.clone()
    }

    /// Getter for `review_uri`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter))]
    pub fn review_uri(&self) -> String {
        self.review_uri.clone()
    }
}

#[cfg(target_arch = "wasm32")]
impl Json for PubkyMarketReviewResponse {}

impl HasIdPath for PubkyMarketReviewResponse {
    const PATH_SEGMENT: &'static str = "marketplace/v1/review_responses/";

    fn create_path(id: &str) -> String {
        [PUBLIC_PATH, APP_PATH, Self::PATH_SEGMENT, id].concat()
    }
}

impl Validatable for PubkyMarketReviewResponse {
    fn sanitize(self) -> Self {
        PubkyMarketReviewResponse {
            text: self.text.trim().to_string(),
            ..self
        }
    }

    fn validate(&self, id: Option<&str>) -> Result<(), String> {
        // The path ID equals the subject review's ID by construction.
        if let Some(id) = id {
            if self.review_id != id {
                return Err(
                    "Validation Error: reviewId does not match the response path ID".into(),
                );
            }
        }

        validate_base_record(
            self.schema_version,
            &self.record_type,
            REVIEW_RESPONSE_RECORD_TYPE,
            &self.owner_pubky,
            self.revision,
            &self.created_at,
            &self.updated_at,
        )?;
        validate_entity_id(&self.review_id, "reviewId")?;

        // reviewUri must be a canonical marketplace review URI whose ID
        // matches this record's reviewId.
        let parsed = ParsedUri::try_from(self.review_uri.as_str()).map_err(|_| {
            "Validation Error: reviewUri must be a canonical marketplace review URI".to_string()
        })?;
        match &parsed.resource {
            Resource::MarketplaceReview(review_id) if *review_id == self.review_id => {}
            Resource::MarketplaceReview(_) => {
                return Err(
                    "Validation Error: reviewUri must reference the same reviewId as the record"
                        .into(),
                );
            }
            _ => {
                return Err(
                    "Validation Error: reviewUri must be a canonical marketplace review URI".into(),
                );
            }
        }
        // A subject never responds to their own record: the review lives on
        // the reviewer's homeserver, the response on the subject's.
        if parsed.user_id.as_ref() == self.owner_pubky {
            return Err(
                "Validation Error: reviewUri must not point at the responder's own homeserver"
                    .into(),
            );
        }

        let text_length = self.text.chars().count();
        if !(1..=MAX_RESPONSE_TEXT_LENGTH).contains(&text_length) {
            return Err(format!(
                "Validation Error: response text must be 1-{MAX_RESPONSE_TEXT_LENGTH} characters"
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::{HashId, Validatable};
    use crate::{PubkyMarketReview, PubkyMarketReviewRatings, PubkyMarketReviewRole};

    const REVIEWER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
    const SELLER: &str = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";

    fn subject_review() -> PubkyMarketReview {
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
                overall: 2,
                item_accuracy: None,
                shipping: None,
                communication: None,
            },
            "Item arrived late.".to_string(),
            "a".repeat(64),
        );
        review.review_id = review.create_id();
        review
    }

    fn valid_response(review: &PubkyMarketReview) -> PubkyMarketReviewResponse {
        PubkyMarketReviewResponse::new(
            review.subject_pubky.clone(),
            1,
            "2026-08-22T00:00:00Z".to_string(),
            "2026-08-22T00:00:00Z".to_string(),
            review.review_id.clone(),
            crate::marketplace_review_uri_builder(
                review.owner_pubky.clone(),
                review.review_id.clone(),
            ),
            "Sorry about the delay — carrier lost the parcel; refunded shipping.".to_string(),
        )
    }

    #[test]
    fn test_create_path_uses_review_id() {
        let review = subject_review();
        let response = valid_response(&review);
        assert_eq!(
            PubkyMarketReviewResponse::create_path(&response.review_id),
            format!(
                "/pub/pubky.app/marketplace/v1/review_responses/{}",
                review.review_id
            )
        );
    }

    #[test]
    fn test_validate_valid() {
        let review = subject_review();
        let response = valid_response(&review);
        assert!(response.validate(Some(&review.review_id)).is_ok());
        assert!(response.is_authorized_response_to(&review));
    }

    #[test]
    fn test_try_from_valid_roundtrip() {
        let review = subject_review();
        let response = valid_response(&review);
        let json = serde_json::to_string(&response).unwrap();
        let parsed = <PubkyMarketReviewResponse as Validatable>::try_from(
            json.as_bytes(),
            &review.review_id,
        )
        .unwrap();
        assert_eq!(parsed, response);
    }

    #[test]
    fn test_try_from_accepts_unknown_field() {
        let review = subject_review();
        let response = valid_response(&review);
        let mut value = serde_json::to_value(&response).unwrap();
        value["surprise"] = serde_json::json!(true);
        let json = serde_json::to_string(&value).unwrap();
        assert!(<PubkyMarketReviewResponse as Validatable>::try_from(
            json.as_bytes(),
            &review.review_id
        )
        .is_ok());
    }

    #[test]
    fn test_validate_path_id_mismatch() {
        let review = subject_review();
        let response = valid_response(&review);
        assert!(response
            .validate(Some("DIFFERENTID12345678901234"))
            .is_err());
    }

    #[test]
    fn test_validate_review_uri_id_mismatch() {
        let review = subject_review();
        let mut response = valid_response(&review);
        response.review_uri = crate::marketplace_review_uri_builder(
            review.owner_pubky.clone(),
            "8Z8CWH8NVYQY39ZEBFGKQWWEKG".to_string(),
        );
        assert!(response.validate(Some(&review.review_id)).is_err());
    }

    #[test]
    fn test_validate_review_uri_not_a_review() {
        let review = subject_review();
        let mut response = valid_response(&review);
        response.review_uri = format!(
            "pubky://{REVIEWER}/pub/pubky.app/posts/{}",
            review.review_id
        );
        assert!(response.validate(Some(&review.review_id)).is_err());
    }

    #[test]
    fn test_validate_rejects_self_hosted_review_uri() {
        let review = subject_review();
        let mut response = valid_response(&review);
        // A record claiming to respond to a review on the responder's own
        // homeserver is structurally impossible in the topology.
        response.review_uri = crate::marketplace_review_uri_builder(
            response.owner_pubky.clone(),
            review.review_id.clone(),
        );
        assert!(response.validate(Some(&review.review_id)).is_err());
    }

    #[test]
    fn test_structural_authorization_rejects_impostor() {
        let review = subject_review();
        let mut response = valid_response(&review);
        // An impostor (the reviewer themselves, or any third party) publishes
        // a response record under their own pubky: shape-valid, but the
        // structural check fails because owner != subjectPubky.
        response.owner_pubky = REVIEWER.to_string();
        assert!(!response.is_authorized_response_to(&review));

        // Wrong review id also fails authorization.
        let mut response = valid_response(&review);
        response.review_id = "8Z8CWH8NVYQY39ZEBFGKQWWEKG".to_string();
        assert!(!response.is_authorized_response_to(&review));
    }

    #[test]
    fn test_validate_text_bounds() {
        let review = subject_review();

        let mut response = valid_response(&review);
        response.text = "   ".to_string();
        let response = response.sanitize();
        assert!(response.validate(Some(&review.review_id)).is_err());

        let mut response = valid_response(&review);
        response.text = "a".repeat(MAX_RESPONSE_TEXT_LENGTH + 1);
        assert!(response.validate(Some(&review.review_id)).is_err());
    }

    #[test]
    fn test_sanitize_trims_text() {
        let review = subject_review();
        let mut response = valid_response(&review);
        response.text = "  Thanks for the patience.  ".to_string();
        let response = response.sanitize();
        assert_eq!(response.text, "Thanks for the patience.");
    }

    #[test]
    fn test_seller_reviewing_buyer_direction() {
        // Response to a seller_reviewing_buyer review: the buyer is the
        // subject and responds from their homeserver, symmetrically.
        let mut review = subject_review();
        review.role = PubkyMarketReviewRole::SellerReviewingBuyer;
        review.owner_pubky = SELLER.to_string();
        review.subject_pubky = REVIEWER.to_string();
        review.review_id = review.create_id();

        let response = valid_response(&review);
        assert_eq!(response.owner_pubky, REVIEWER);
        assert!(response.validate(Some(&review.review_id)).is_ok());
        assert!(response.is_authorized_response_to(&review));
    }
}
