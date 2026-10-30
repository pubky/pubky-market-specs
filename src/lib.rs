//! Pubky Market commerce data formats.
//!
//! Shops, listings, drops, reviews, review responses, private watchlists and
//! order receipts, the purchase, order-receipt and drop-edition attestations,
//! and the attestor records, with validation, sanitization, ID derivation and
//! URI helpers.
//!
//! Every record is stored under the `pubky.app` app directory
//! (`/pub/pubky.app/marketplace/v1/` and `/priv/pubky.app/marketplace/v1/`)
//! and serializes to the same JSON as the marketplace build of
//! `pubky-app-specs` (`0.6.2-marketplace.16`), which this crate replaces for
//! commerce data. Social records (profiles, posts, tags, follows, files) are
//! not part of this crate.

mod common;
mod constants;
mod models;
pub mod traits;
mod types;
mod uri;

pub use common::validate_crockford_id;
pub use constants::{APP_PATH, MARKETPLACE_PATH, PRIVATE_PATH, PROTOCOL, PUBLIC_PATH, VERSION};
pub use models::drop::{PubkyMarketDrop, PubkyMarketDropFormat, PubkyMarketDropStockDisplay};
pub use models::drop_edition_attestation::{
    PubkyMarketDropEditionAttestation, PubkyMarketDropEditionAttestationClaims,
    DROP_EDITION_ATTESTATION_TYP, DROP_EDITION_ATTESTATION_VERSION,
};
pub use models::listing::{
    PubkyMarketDigitalLock, PubkyMarketFulfillmentMethod, PubkyMarketListing,
    PubkyMarketListingAttributeValue, PubkyMarketListingCondition, PubkyMarketListingMedia,
    PubkyMarketListingMediaKind, PubkyMarketListingPackage, PubkyMarketListingSale,
    PubkyMarketListingState, PubkyMarketListingVariant, PubkyMarketReturnPolicy,
    PubkyMarketShippingOption,
};
pub use models::marketplace::{PubkyMarketLocation, PubkyMarketMoney, MARKETPLACE_SCHEMA_VERSION};
pub use models::marketplace_attestation::{
    base64url_encode, PubkyMarketPurchaseAttestation, PubkyMarketPurchaseAttestationClaims,
    PURCHASE_ATTESTATION_TYP, PURCHASE_ATTESTATION_VERSION,
};
pub use models::marketplace_attestor::{
    PubkyMarketAttestorAnnotation, PubkyMarketAttestorIssuancePolicy, PubkyMarketAttestorKey,
    PubkyMarketAttestorOperator, PubkyMarketAttestorOutcome, PubkyMarketAttestorOutcomeKind,
    PubkyMarketAttestorProfile, AMOUNT_BAND_POLICY_BOTH_SIDES_CONSENT, ATTESTOR_ANNOTATION_TYP,
    ATTESTOR_PROFILE_TYP, PRICE_DECADE_POLICY_ALWAYS,
};
pub use models::marketplace_review::{
    PubkyMarketItemCountKey, PubkyMarketReview, PubkyMarketReviewRatings, PubkyMarketReviewRole,
    PubkyMarketSellerRatingKey,
};
pub use models::marketplace_weight::{
    price_decade, price_weight, price_weight_currency, price_weight_decade_offset,
    PriceWeightCurrency, MAX_PRICE_WEIGHT, MIN_PRICE_WEIGHT, PRICE_WEIGHT_CURRENCIES,
};
pub use models::order_receipt::{
    PubkyMarketOrderReceipt, PubkyMarketOrderReceiptDrop, PubkyMarketOrderReceiptRole,
};
pub use models::order_receipt_attestation::{
    PubkyMarketMoneyObject, PubkyMarketOrderReceiptAttestation,
    PubkyMarketOrderReceiptAttestationClaims, PubkyMarketOrderReceiptAttestationV2Claims,
    ORDER_RECEIPT_ATTESTATION_TYP, ORDER_RECEIPT_ATTESTATION_V2_TYP,
    ORDER_RECEIPT_ATTESTATION_V2_VERSION, ORDER_RECEIPT_ATTESTATION_VERSION,
};
pub use models::review_response::PubkyMarketReviewResponse;
pub use models::shop::PubkyMarketShop;
pub use models::watchlist::{
    PubkyMarketWatchlist, PubkyMarketWatchlistItem, PubkyMarketWatchlistTombstone,
    MAX_WATCHLIST_ITEMS, MAX_WATCHLIST_TOMBSTONES,
};
pub use models::PubkyMarketObject;
pub use types::PubkyId;
#[doc(inline)]
pub use uri::{
    attestor_annotation_uri_builder, attestor_profile_uri_builder, drop_uri_builder,
    is_pubky_scheme, listing_uri_builder, marketplace_review_uri_builder,
    order_receipt_uri_builder, review_response_uri_builder, shop_uri_builder, try_parse_pubky_path,
    watchlist_uri_builder, ParsedUri, PubkyPath, Resource,
};

#[cfg(target_arch = "wasm32")]
mod wasm;
#[cfg(target_arch = "wasm32")]
pub use wasm::*;
