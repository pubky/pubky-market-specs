use crate::{
    constants::PROTOCOL,
    traits::{HasIdPath, HasPath},
    PubkyMarketAttestorAnnotation, PubkyMarketAttestorProfile, PubkyMarketDrop, PubkyMarketListing,
    PubkyMarketOrderReceipt, PubkyMarketReview, PubkyMarketReviewResponse, PubkyMarketShop,
    PubkyMarketWatchlist,
};

/// Builds a Shop URI of the form "pubky://<owner_id>/pub/pubky.app/marketplace/v1/shop.json"
pub fn shop_uri_builder(owner_id: String) -> String {
    let shop_path = PubkyMarketShop::create_path();
    [PROTOCOL, &owner_id, &shop_path].concat()
}

/// Builds a Listing URI of the form "pubky://<seller_id>/pub/pubky.app/marketplace/v1/listings/<listing_id>"
pub fn listing_uri_builder(seller_id: String, listing_id: String) -> String {
    let listing_path = PubkyMarketListing::create_path(&listing_id);
    [PROTOCOL, &seller_id, &listing_path].concat()
}

/// Builds a Drop URI of the form "pubky://<owner_id>/pub/pubky.app/marketplace/v1/drops/<drop_id>"
pub fn drop_uri_builder(owner_id: String, drop_id: String) -> String {
    let drop_path = PubkyMarketDrop::create_path(&drop_id);
    [PROTOCOL, &owner_id, &drop_path].concat()
}

/// Builds a review URI of the form "pubky://<author_id>/pub/pubky.app/marketplace/v1/reviews/<review_id>"
pub fn marketplace_review_uri_builder(author_id: String, review_id: String) -> String {
    let review_path = PubkyMarketReview::create_path(&review_id);
    [PROTOCOL, &author_id, &review_path].concat()
}

/// Builds a private Watchlist URI of the form "pubky://<owner_id>/priv/pubky.app/marketplace/v1/watchlist.json".
///
/// Note the `/priv/` prefix: only the owner's own authenticated sessions can
/// read or write this URI; it is never watcher-indexed.
pub fn watchlist_uri_builder(owner_id: String) -> String {
    let watchlist_path = PubkyMarketWatchlist::create_path();
    [PROTOCOL, &owner_id, &watchlist_path].concat()
}

/// Builds a private OrderReceipt URI of the form "pubky://<owner_id>/priv/pubky.app/marketplace/v1/receipts/<receipt_id>".
///
/// Note the `/priv/` prefix: only the owner's own authenticated sessions can
/// read or write this URI; it is never watcher-indexed.
pub fn order_receipt_uri_builder(owner_id: String, receipt_id: String) -> String {
    let receipt_path = PubkyMarketOrderReceipt::create_path(&receipt_id);
    [PROTOCOL, &owner_id, &receipt_path].concat()
}

/// Builds a ReviewResponse URI of the form "pubky://<responder_id>/pub/pubky.app/marketplace/v1/review_responses/<review_id>"
pub fn review_response_uri_builder(responder_id: String, review_id: String) -> String {
    let response_path = PubkyMarketReviewResponse::create_path(&review_id);
    [PROTOCOL, &responder_id, &response_path].concat()
}

/// Builds an attestor profile URI of the form
/// "pubky://<attestor_id>/pub/pubky.app/marketplace/v1/attestor/profile"
pub fn attestor_profile_uri_builder(attestor_id: String) -> String {
    let path = PubkyMarketAttestorProfile::create_path();
    [PROTOCOL, &attestor_id, &path].concat()
}

/// Builds an attestor annotation URI of the form
/// "pubky://<attestor_id>/pub/pubky.app/marketplace/v1/attestor/annotations/<order_ref>"
pub fn attestor_annotation_uri_builder(attestor_id: String, order_ref: String) -> String {
    let path = PubkyMarketAttestorAnnotation::create_path(&order_ref);
    [PROTOCOL, &attestor_id, &path].concat()
}
