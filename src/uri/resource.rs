use crate::{
    traits::{HasIdPath, HasPath},
    PubkyMarketAttestorAnnotation, PubkyMarketAttestorProfile, PubkyMarketDrop, PubkyMarketListing,
    PubkyMarketReview, PubkyMarketReviewResponse, PubkyMarketShop,
};
use serde::{Deserialize, Serialize};
use std::fmt;

/// The public Pubky Market record a `pubky://` URI names.
///
/// Variant names and the `Display` strings are the ones `pubky-app-specs`
/// uses for the same paths, so serialized values and logged resource names
/// stay the same. Social paths are not Market resources and parse as
/// [`Resource::Unknown`].
#[derive(Debug, PartialEq, Default, Clone, Serialize, Deserialize)]
pub enum Resource {
    Shop,
    Listing(String),
    Drop(String),
    MarketplaceReview(String),
    ReviewResponse(String),
    AttestorProfile,
    AttestorAnnotation(String),
    #[default]
    Unknown,
}

impl fmt::Display for Resource {
    /// Returns the resource name without any identifier.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Resource::Shop => PubkyMarketShop::PATH_SEGMENT,
            Resource::Listing(_) => PubkyMarketListing::PATH_SEGMENT.trim_end_matches('/'),
            Resource::Drop(_) => PubkyMarketDrop::PATH_SEGMENT.trim_end_matches('/'),
            Resource::MarketplaceReview(_) => PubkyMarketReview::PATH_SEGMENT.trim_end_matches('/'),
            Resource::ReviewResponse(_) => {
                PubkyMarketReviewResponse::PATH_SEGMENT.trim_end_matches('/')
            }
            Resource::AttestorProfile => PubkyMarketAttestorProfile::PATH_SEGMENT,
            Resource::AttestorAnnotation(_) => {
                PubkyMarketAttestorAnnotation::PATH_SEGMENT.trim_end_matches('/')
            }
            Resource::Unknown => "unknown",
        };
        write!(f, "{}", name)
    }
}

impl Resource {
    /// Returns the identifier as a `Some(String)` if the resource variant holds one,
    /// or `None` if there is no identifier.
    pub fn id(&self) -> Option<String> {
        match self {
            Resource::Listing(id)
            | Resource::Drop(id)
            | Resource::MarketplaceReview(id)
            | Resource::ReviewResponse(id)
            | Resource::AttestorAnnotation(id) => Some(id.clone()),
            Resource::Shop | Resource::AttestorProfile | Resource::Unknown => None,
        }
    }
}
