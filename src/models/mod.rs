use crate::{traits::Validatable, ParsedUri, Resource};

pub mod drop;
pub mod drop_edition_attestation;
pub mod listing;
pub mod marketplace;
pub mod marketplace_attestation;
pub mod marketplace_attestor;
pub mod marketplace_review;
pub mod marketplace_weight;
pub mod order_receipt;
pub mod order_receipt_attestation;
pub mod review_response;
pub mod shop;
pub mod watchlist;

use super::{
    PubkyMarketAttestorAnnotation, PubkyMarketAttestorProfile, PubkyMarketDrop, PubkyMarketListing,
    PubkyMarketReview, PubkyMarketReviewResponse, PubkyMarketShop,
};

/// A unified enum wrapping every public Pubky Market record an indexer reads.
///
/// The private records (watchlist, order receipts) are deliberately absent:
/// they are never indexed.
#[derive(Debug, Clone)]
pub enum PubkyMarketObject {
    Shop(shop::PubkyMarketShop),
    Listing(Box<listing::PubkyMarketListing>),
    Drop(drop::PubkyMarketDrop),
    MarketplaceReview(marketplace_review::PubkyMarketReview),
    ReviewResponse(review_response::PubkyMarketReviewResponse),
    AttestorProfile(marketplace_attestor::PubkyMarketAttestorProfile),
    AttestorAnnotation(marketplace_attestor::PubkyMarketAttestorAnnotation),
}

impl PubkyMarketObject {
    /// Given a URI and a blob (raw data from the homeserver),
    /// this function returns the fully formed PubkyMarketObject.
    /// Attestor records are verified against the URI's owner here.
    pub fn from_uri<S: AsRef<str>>(uri: S, blob: &[u8]) -> Result<Self, String> {
        let uri = uri.as_ref();
        let parsed_uri = ParsedUri::try_from(uri)?;
        match parsed_uri.resource {
            Resource::AttestorProfile => Ok(PubkyMarketObject::AttestorProfile(
                PubkyMarketAttestorProfile::from_verified_json(uri, blob)?,
            )),
            Resource::AttestorAnnotation(_) => Ok(PubkyMarketObject::AttestorAnnotation(
                PubkyMarketAttestorAnnotation::from_verified_json(uri, blob)?,
            )),
            resource => Self::from_resource(&resource, blob),
        }
    }

    /// Given a Resource and a blob (raw data from the homeserver),
    /// this function returns the fully formed PubkyMarketObject.
    pub fn from_resource(resource: &Resource, blob: &[u8]) -> Result<Self, String> {
        match resource {
            Resource::Shop => {
                let shop = <PubkyMarketShop as Validatable>::try_from(blob, "")?;
                Ok(PubkyMarketObject::Shop(shop))
            }
            Resource::Listing(listing_id) => {
                let listing = <PubkyMarketListing as Validatable>::try_from(blob, listing_id)?;
                Ok(PubkyMarketObject::Listing(Box::new(listing)))
            }
            Resource::Drop(drop_id) => {
                let drop = <PubkyMarketDrop as Validatable>::try_from(blob, drop_id)?;
                Ok(PubkyMarketObject::Drop(drop))
            }
            Resource::MarketplaceReview(review_id) => {
                let review = <PubkyMarketReview as Validatable>::try_from(blob, review_id)?;
                Ok(PubkyMarketObject::MarketplaceReview(review))
            }
            Resource::ReviewResponse(review_id) => {
                let response =
                    <PubkyMarketReviewResponse as Validatable>::try_from(blob, review_id)?;
                Ok(PubkyMarketObject::ReviewResponse(response))
            }
            Resource::AttestorProfile | Resource::AttestorAnnotation(_) => Err(
                "Attestor records verify against their owner: use PubkyMarketObject::from_uri"
                    .into(),
            ),
            Resource::Unknown => Err(format!("Unrecognized resource {:?}", resource)),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{listing_uri_builder, marketplace_review_uri_builder, shop_uri_builder};

    use super::*;

    #[test]
    fn test_import_shop() {
        let owner = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
        let uri = shop_uri_builder(owner.into());
        let shop_json = r#"{
            "schemaVersion": 1,
            "recordType": "shop",
            "ownerPubky": "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo",
            "revision": 1,
            "createdAt": "2025-01-01T00:00:00Z",
            "updatedAt": "2025-01-02T00:00:00Z",
            "name": "Boots & Co",
            "bio": "Quality hiking boots.",
            "location": { "countryCode": "US", "region": "Oregon" },
            "shippingPolicy": "Ships within 3 business days.",
            "returnPolicy": "Returns accepted within 30 days.",
            "vacationMode": false
        }"#;
        let result = PubkyMarketObject::from_uri(uri, shop_json.as_bytes());
        assert!(
            result.is_ok(),
            "Expected a successful import for shop, got error: {:?}",
            result.err()
        );
        match result.unwrap() {
            PubkyMarketObject::Shop(shop) => {
                assert_eq!(shop.name, "Boots & Co", "Shop name mismatch");
                assert_eq!(shop.owner_pubky, owner, "Shop owner mismatch");
            }
            other => panic!("Expected a Shop object, got {:?}", other),
        }
    }

    #[test]
    fn test_import_listing() {
        use crate::traits::TimestampId;

        let owner = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
        let listing = crate::PubkyMarketListing::new(
            owner.to_string(),
            1,
            "2025-01-01T00:00:00Z".to_string(),
            "2025-01-02T00:00:00Z".to_string(),
            String::new(),
            crate::PubkyMarketListingState::Active,
            "Hiking boots".to_string(),
            "Sturdy leather hiking boots.".to_string(),
            1,
            "fashion".to_string(),
            None,
            crate::PubkyMarketListingCondition::New,
            None,
            vec!["boots".to_string()],
            crate::PubkyMarketLocation {
                country_code: "US".to_string(),
                region: None,
            },
            vec![crate::PubkyMarketListingMedia {
                id: "image_01".to_string(),
                kind: crate::PubkyMarketListingMediaKind::Image,
                url: format!("pubky://{owner}/pub/pubky.app/marketplace/v1/media/image_01"),
                content_hash: "a".repeat(64),
                mime_type: "image/png".to_string(),
                byte_size: 1024,
                width: 800,
                height: 600,
                duration_ms: None,
                alt_text: "A pair of boots".to_string(),
            }],
            vec![crate::PubkyMarketListingVariant {
                id: "variant_01".to_string(),
                sku: None,
                options: Default::default(),
                price_override: None,
                quantity: 5,
                media_ids: vec![],
                enabled: true,
            }],
            crate::PubkyMarketListingSale::FixedPrice {
                unit_price: crate::PubkyMarketMoney {
                    amount_minor: 12_000,
                    currency: "USD".to_string(),
                    exponent: 2,
                },
                accepts_offers: false,
            },
            vec![crate::PubkyMarketFulfillmentMethod::Pickup],
            None,
            vec![],
            crate::PubkyMarketReturnPolicy {
                accepts_returns: false,
                return_window_days: None,
                buyer_pays_return_shipping: false,
                details: None,
            },
            None,
            false,
        );
        let mut listing = listing;
        listing.listing_id = listing.create_id();

        let uri = listing_uri_builder(owner.into(), listing.listing_id.clone());
        let listing_json = serde_json::to_string(&listing).unwrap();
        let result = PubkyMarketObject::from_uri(uri, listing_json.as_bytes());
        assert!(
            result.is_ok(),
            "Expected a successful import for listing, got error: {:?}",
            result.err()
        );
        match result.unwrap() {
            PubkyMarketObject::Listing(imported) => {
                assert_eq!(imported.title, "Hiking boots", "Listing title mismatch");
                assert_eq!(
                    imported.listing_id, listing.listing_id,
                    "Listing ID mismatch"
                );
            }
            other => panic!("Expected a Listing object, got {:?}", other),
        }
    }

    #[test]
    fn test_import_drop() {
        let owner = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
        let drop = crate::PubkyMarketDrop::new(
            owner.to_string(),
            1,
            "2026-01-01T00:00:00Z".to_string(),
            "2026-01-02T00:00:00Z".to_string(),
            "spring-drop-01".to_string(),
            "Spring boot drop".to_string(),
            "Limited spring release.".to_string(),
            vec![format!(
                "pubky://{owner}/pub/pubky.app/marketplace/v1/media/drop_banner"
            )],
            crate::PubkyMarketDropFormat::Fcfs,
            "2026-02-01T00:00:00Z".to_string(),
            Some("2026-02-02T00:00:00Z".to_string()),
            vec!["listing_01".to_string()],
            500,
            2,
            crate::PubkyMarketDropStockDisplay::Exact,
        );

        let uri = crate::drop_uri_builder(owner.into(), drop.drop_id.clone());
        let drop_json = serde_json::to_string(&drop).unwrap();
        let result = PubkyMarketObject::from_uri(uri, drop_json.as_bytes());
        assert!(
            result.is_ok(),
            "Expected a successful import for drop, got error: {:?}",
            result.err()
        );
        match result.unwrap() {
            PubkyMarketObject::Drop(imported) => {
                assert_eq!(imported.title, "Spring boot drop", "Drop title mismatch");
                assert_eq!(imported.drop_id, drop.drop_id, "Drop ID mismatch");
            }
            other => panic!("Expected a Drop object, got {:?}", other),
        }
    }

    #[test]
    fn test_import_marketplace_review() {
        use crate::traits::HashId;

        let reviewer = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
        let seller = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";
        let mut review = crate::PubkyMarketReview::new(
            reviewer.to_string(),
            1,
            "2025-01-01T00:00:00Z".to_string(),
            "2025-01-01T00:00:00Z".to_string(),
            String::new(),
            seller.to_string(),
            seller.to_string(),
            "0032SSN7Q4EVG".to_string(),
            crate::PubkyMarketReviewRole::BuyerReviewingSeller,
            crate::PubkyMarketReviewRatings {
                overall: 5,
                item_accuracy: None,
                shipping: None,
                communication: None,
            },
            "Great seller.".to_string(),
            "a".repeat(64),
        );
        review.review_id = review.create_id();

        let uri = marketplace_review_uri_builder(reviewer.into(), review.review_id.clone());
        let review_json = serde_json::to_string(&review).unwrap();
        let result = PubkyMarketObject::from_uri(uri, review_json.as_bytes());
        assert!(
            result.is_ok(),
            "Expected a successful import for marketplace review, got error: {:?}",
            result.err()
        );
        match result.unwrap() {
            PubkyMarketObject::MarketplaceReview(imported) => {
                assert_eq!(imported.text, "Great seller.", "Review text mismatch");
                assert_eq!(imported.review_id, review.review_id, "Review ID mismatch");
            }
            other => panic!("Expected a MarketplaceReview object, got {:?}", other),
        }
    }

    #[test]
    fn test_import_unknown_resource() {
        let uri =
            "pubky://operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo/pub/pubky.app/unknown/ID";
        let json = r#"{}"#;
        let result = PubkyMarketObject::from_uri(uri, json.as_bytes());
        assert!(
            result.is_err(),
            "Expected an error for unknown resource, but got: {:?}",
            result.ok()
        );
        let err = result.err().unwrap();
        assert!(
            err.contains("Unrecognized resource"),
            "Error message does not contain expected text: {}",
            err
        );
    }
}
