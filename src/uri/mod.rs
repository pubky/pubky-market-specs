//! URI parsing and construction for Pubky Market `pubky://` paths.
//!
//! - **Build** — `*_uri_builder` functions
//! - **Parse** — [`ParsedUri`] for the public Market record paths
//! - **Parse (structure)** — [`try_parse_pubky_path`] / [`PubkyPath`]

mod builders;
mod parsed;
mod path;
mod resource;
mod scheme;

pub use builders::{
    attestor_annotation_uri_builder, attestor_profile_uri_builder, drop_uri_builder,
    listing_uri_builder, marketplace_review_uri_builder, order_receipt_uri_builder,
    review_response_uri_builder, shop_uri_builder, watchlist_uri_builder,
};
pub use parsed::ParsedUri;
pub use path::{try_parse_pubky_path, PubkyPath};
pub use resource::Resource;
pub use scheme::is_pubky_scheme;
