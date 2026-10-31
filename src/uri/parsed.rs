use crate::{
    traits::{HasIdPath, HasPath},
    PubkyId, PubkyMarketAttestorAnnotation, PubkyMarketAttestorProfile, PubkyMarketDrop,
    PubkyMarketListing, PubkyMarketReview, PubkyMarketReviewResponse, PubkyMarketShop, APP_PATH,
    MARKETPLACE_PATH, PROTOCOL,
};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

use super::Resource;

/// A parsed `pubky://<user_id>/pub/pubky.app/marketplace/v1/...` URI.
///
/// For every Market path the result equals what `pubky-app-specs` parses.
/// Non-Market paths under `pubky.app` parse to [`Resource::Unknown`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedUri {
    pub user_id: PubkyId,
    pub resource: Resource,
}

impl ParsedUri {
    /// Converts the [ParsedUri] back into its URI string representation.
    /// Returns an error if the resource is Unknown.
    pub fn try_to_uri_str(&self) -> Result<String, String> {
        let path = match &self.resource {
            Resource::Shop => PubkyMarketShop::create_path(),
            Resource::Listing(id) => PubkyMarketListing::create_path(id),
            Resource::Drop(id) => PubkyMarketDrop::create_path(id),
            Resource::MarketplaceReview(id) => PubkyMarketReview::create_path(id),
            Resource::ReviewResponse(id) => PubkyMarketReviewResponse::create_path(id),
            Resource::AttestorProfile => PubkyMarketAttestorProfile::create_path(),
            Resource::AttestorAnnotation(id) => PubkyMarketAttestorAnnotation::create_path(id),
            Resource::Unknown => return Err("Cannot convert Unknown resource to URI".to_string()),
        };

        Ok([PROTOCOL, self.user_id.as_ref(), &path].concat())
    }
}

impl TryFrom<&str> for ParsedUri {
    type Error = String;
    fn try_from(uri: &str) -> Result<Self, Self::Error> {
        let path = super::path::try_parse_pubky_path(uri)?;

        if path.app != APP_PATH.trim_matches('/') {
            return Err(format!(
                "Expected app path '{}' but got '{}' in URI: {}",
                APP_PATH.trim_matches('/'),
                path.app,
                uri
            ));
        }

        Ok(ParsedUri {
            user_id: path.user_id,
            resource: resource_from_segments(&path.segments),
        })
    }
}

impl TryFrom<String> for ParsedUri {
    type Error = String;

    fn try_from(uri: String) -> Result<Self, Self::Error> {
        ParsedUri::try_from(uri.as_str())
    }
}

fn resource_from_segments(segments: &[String]) -> Resource {
    match segments {
        [marketplace, version, segment]
            if format!("{marketplace}/{version}/{segment}") == PubkyMarketShop::PATH_SEGMENT =>
        {
            Resource::Shop
        }
        // Attestor records live one level deeper, under `marketplace/v1/attestor/`.
        [marketplace, version, attestor, segment]
            if format!("{marketplace}/{version}/{attestor}/{segment}")
                == PubkyMarketAttestorProfile::PATH_SEGMENT =>
        {
            Resource::AttestorProfile
        }
        [marketplace, version, attestor, collection, id]
            if format!("{marketplace}/{version}/{attestor}/{collection}/")
                == PubkyMarketAttestorAnnotation::PATH_SEGMENT
                && !id.is_empty() =>
        {
            Resource::AttestorAnnotation(id.clone())
        }
        [marketplace, version, res_type, id, ..]
            if format!("{marketplace}/{version}/") == MARKETPLACE_PATH && !id.is_empty() =>
        {
            let resource_type = format!("{MARKETPLACE_PATH}{res_type}/");
            match resource_type.as_str() {
                PubkyMarketListing::PATH_SEGMENT => Resource::Listing(id.clone()),
                PubkyMarketDrop::PATH_SEGMENT => Resource::Drop(id.clone()),
                PubkyMarketReview::PATH_SEGMENT => Resource::MarketplaceReview(id.clone()),
                PubkyMarketReviewResponse::PATH_SEGMENT => Resource::ReviewResponse(id.clone()),
                _ => Resource::Unknown,
            }
        }
        _ => Resource::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        attestor_annotation_uri_builder, attestor_profile_uri_builder, drop_uri_builder,
        listing_uri_builder, marketplace_review_uri_builder, review_response_uri_builder,
        shop_uri_builder,
    };

    use super::*;

    const USER_ID: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";

    fn parse(uri: String) -> ParsedUri {
        ParsedUri::try_from(uri).expect("uri should parse")
    }

    #[test]
    fn test_market_uris_parse_and_roundtrip() {
        let cases = [
            (shop_uri_builder(USER_ID.into()), Resource::Shop),
            (
                listing_uri_builder(USER_ID.into(), "0032SSN7Q4EVG".into()),
                Resource::Listing("0032SSN7Q4EVG".into()),
            ),
            (
                drop_uri_builder(USER_ID.into(), "spring-drop-01".into()),
                Resource::Drop("spring-drop-01".into()),
            ),
            (
                marketplace_review_uri_builder(USER_ID.into(), "8Z8CWH8NVYQY39ZEBFGKQWWEKG".into()),
                Resource::MarketplaceReview("8Z8CWH8NVYQY39ZEBFGKQWWEKG".into()),
            ),
            (
                review_response_uri_builder(USER_ID.into(), "8Z8CWH8NVYQY39ZEBFGKQWWEKG".into()),
                Resource::ReviewResponse("8Z8CWH8NVYQY39ZEBFGKQWWEKG".into()),
            ),
            (
                attestor_profile_uri_builder(USER_ID.into()),
                Resource::AttestorProfile,
            ),
            (
                attestor_annotation_uri_builder(USER_ID.into(), "ref-1".into()),
                Resource::AttestorAnnotation("ref-1".into()),
            ),
        ];
        for (uri, expected) in cases {
            let parsed = parse(uri.clone());
            assert_eq!(parsed.user_id, PubkyId::try_from(USER_ID).unwrap());
            assert_eq!(parsed.resource, expected, "{uri}");
            assert_eq!(parsed.try_to_uri_str().unwrap(), uri, "roundtrip {uri}");
        }
    }

    #[test]
    fn test_unknown_market_paths() {
        for path in [
            "marketplace/v1/drops/",
            "marketplace/v1/listings/",
            "marketplace/v1/orders/xyz",
            "marketplace/v1",
            "marketplace/v2/listings/abc",
            "",
            "unknown/xyz",
        ] {
            let parsed = parse(format!("pubky://{USER_ID}/pub/pubky.app/{path}"));
            assert_eq!(parsed.resource, Resource::Unknown, "{path}");
            assert!(parsed.try_to_uri_str().is_err());
        }
    }

    #[test]
    fn test_social_paths_are_not_market_resources() {
        for path in [
            "profile.json",
            "posts/0032SSN7Q4EVG",
            "tags/8Z8CWH8NVYQY39ZEBFGKQWWEKG",
        ] {
            let parsed = parse(format!("pubky://{USER_ID}/pub/pubky.app/{path}"));
            assert_eq!(parsed.resource, Resource::Unknown, "{path}");
        }
    }

    #[test]
    fn test_invalid_uris() {
        for uri in [
            format!("http://{USER_ID}/pub/pubky.app/marketplace/v1/shop.json"),
            format!("pubky://{USER_ID}/invalid/pubky.app/marketplace/v1/shop.json"),
            format!("pubky://{USER_ID}/pub/other.app/marketplace/v1/shop.json"),
            "pubky:///pub/pubky.app/marketplace/v1/shop.json".to_string(),
            "not a url".to_string(),
        ] {
            assert!(ParsedUri::try_from(uri.as_str()).is_err(), "{uri}");
        }
    }
}
