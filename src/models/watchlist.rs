use crate::{
    models::marketplace::{
        validate_base_record, validate_entity_id, validate_pubky, MAX_SAFE_INTEGER,
    },
    traits::{HasPath, Validatable},
    APP_PATH, MARKETPLACE_PATH, PRIVATE_PATH,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[cfg(target_arch = "wasm32")]
use crate::traits::Json;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

const WATCHLIST_RECORD_TYPE: &str = "watchlist";
/// Maximum number of active watch entries in one document.
pub const MAX_WATCHLIST_ITEMS: usize = 500;
/// Maximum number of retained tombstones in one document.
pub const MAX_WATCHLIST_TOMBSTONES: usize = 500;

/// One actively watched listing.
///
/// `watched_at_ms` is milliseconds since the UNIX epoch. Entry timestamps are
/// integers (not ISO-8601 strings like the document-level `createdAt` /
/// `updatedAt`) on purpose: they are last-write-wins merge keys that clients
/// compare numerically, and integer milliseconds cannot be skewed by
/// offset-formatting differences between writers.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketWatchlistItem {
    /// z-base-32 pubky of the seller who owns the watched listing.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub listing_owner_pubky: String,
    /// Identifier of the watched listing.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub listing_id: String,
    /// Milliseconds since the UNIX epoch when the watch was (re)asserted.
    pub watched_at_ms: i64,
}

/// A removed watch, retained so a delete wins over a stale re-add during merge.
///
/// `removed_at_ms` is milliseconds since the UNIX epoch (see
/// [`PubkyMarketWatchlistItem`] for why entry timestamps are integers).
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketWatchlistTombstone {
    /// z-base-32 pubky of the seller who owns the unwatched listing.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub listing_owner_pubky: String,
    /// Identifier of the unwatched listing.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub listing_id: String,
    /// Milliseconds since the UNIX epoch when the watch was removed.
    pub removed_at_ms: i64,
}

/// Represents a user's PRIVATE marketplace watchlist.
///
/// URI: /priv/pubky.app/marketplace/v1/watchlist.json
///
/// This is the first record under `/priv/` — the homeserver's authenticated
/// private storage (reads, listings, and writes by anyone but the owner's
/// session are refused). A watchlist reveals purchase intent, so unlike every
/// `/pub/pubky.app/` record it must not be world-readable, directory-listable,
/// or Nexus-indexable. It is therefore deliberately NOT wired into
/// `PubkyMarketObject` / the URI parser used by watchers and indexers: nothing
/// under `/priv/` is index-visible, and this record only ever travels between
/// the owner's own sessions.
///
/// It is a SINGLE revisioned document rather than one record per watched
/// listing because: (a) watch/unwatch toggles are high-churn and a single
/// document makes each sync one `PUT` instead of a create/delete stream;
/// (b) merge needs items and tombstones resolved together atomically — two
/// files could tear; and (c) private storage has no index to benefit from
/// per-item paths. Clients merge concurrent writers per entry
/// (last-write-wins on the entry's millisecond timestamp) and write back the
/// resolved document, in which each listing key appears in `items` or
/// `tombstones` but never both.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketWatchlist {
    /// Marketplace contract version, always `1`.
    pub schema_version: i64,
    /// Record discriminator, always `"watchlist"`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub record_type: String,
    /// z-base-32 pubky of the watchlist owner.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub owner_pubky: String,
    /// Monotonically increasing document revision, starting at 1.
    pub revision: i64,
    /// ISO-8601 creation datetime with offset.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub created_at: String,
    /// ISO-8601 last-update datetime with offset. Must not precede `created_at`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub updated_at: String,
    /// Actively watched listings, keyed by (listingOwnerPubky, listingId).
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub items: Vec<PubkyMarketWatchlistItem>,
    /// Removed watches retained for merge. Clients prune tombstones beyond
    /// the cap oldest-first.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub tombstones: Vec<PubkyMarketWatchlistTombstone>,
}

impl PubkyMarketWatchlist {
    /// Creates a new `PubkyMarketWatchlist` instance.
    pub fn new(
        owner_pubky: String,
        revision: i64,
        created_at: String,
        updated_at: String,
        items: Vec<PubkyMarketWatchlistItem>,
        tombstones: Vec<PubkyMarketWatchlistTombstone>,
    ) -> Self {
        Self {
            schema_version: crate::models::marketplace::MARKETPLACE_SCHEMA_VERSION,
            record_type: WATCHLIST_RECORD_TYPE.to_string(),
            owner_pubky,
            revision,
            created_at,
            updated_at,
            items,
            tombstones,
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl PubkyMarketWatchlist {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = fromJson))]
    pub fn from_json(js_value: &JsValue) -> Result<Self, String> {
        Self::import_json(js_value)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = toJson))]
    pub fn to_json(&self) -> Result<JsValue, String> {
        self.export_json()
    }
}

#[cfg(target_arch = "wasm32")]
impl Json for PubkyMarketWatchlist {}

impl HasPath for PubkyMarketWatchlist {
    const PATH_SEGMENT: &'static str = "watchlist.json";

    fn create_path() -> String {
        [PRIVATE_PATH, APP_PATH, MARKETPLACE_PATH, Self::PATH_SEGMENT].concat()
    }
}

fn validate_timestamp_ms(value: i64, field: &str) -> Result<(), String> {
    if !(1..=MAX_SAFE_INTEGER).contains(&value) {
        return Err(format!(
            "Validation Error: {field} must be a positive safe integer of epoch milliseconds"
        ));
    }
    Ok(())
}

impl Validatable for PubkyMarketWatchlist {
    fn validate(&self, _id: Option<&str>) -> Result<(), String> {
        validate_base_record(
            self.schema_version,
            &self.record_type,
            WATCHLIST_RECORD_TYPE,
            &self.owner_pubky,
            self.revision,
            &self.created_at,
            &self.updated_at,
        )?;

        if self.items.len() > MAX_WATCHLIST_ITEMS {
            return Err(format!(
                "Validation Error: watchlist must contain at most {MAX_WATCHLIST_ITEMS} items"
            ));
        }
        if self.tombstones.len() > MAX_WATCHLIST_TOMBSTONES {
            return Err(format!(
                "Validation Error: watchlist must contain at most {MAX_WATCHLIST_TOMBSTONES} tombstones"
            ));
        }

        // Every listing key must appear at most once across items AND
        // tombstones: the document is the post-merge resolved state, in which
        // a listing is either watched or removed, never both.
        let mut seen_keys: HashSet<(&str, &str)> = HashSet::new();

        for item in &self.items {
            validate_pubky(&item.listing_owner_pubky, "items listingOwnerPubky")?;
            validate_entity_id(&item.listing_id, "items listingId")?;
            validate_timestamp_ms(item.watched_at_ms, "items watchedAtMs")?;
            if !seen_keys.insert((item.listing_owner_pubky.as_str(), item.listing_id.as_str())) {
                return Err(
                    "Validation Error: watchlist listing keys must be unique across items and tombstones"
                        .into(),
                );
            }
        }

        for tombstone in &self.tombstones {
            validate_pubky(
                &tombstone.listing_owner_pubky,
                "tombstones listingOwnerPubky",
            )?;
            validate_entity_id(&tombstone.listing_id, "tombstones listingId")?;
            validate_timestamp_ms(tombstone.removed_at_ms, "tombstones removedAtMs")?;
            if !seen_keys.insert((
                tombstone.listing_owner_pubky.as_str(),
                tombstone.listing_id.as_str(),
            )) {
                return Err(
                    "Validation Error: watchlist listing keys must be unique across items and tombstones"
                        .into(),
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::Validatable;

    const OWNER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
    const SELLER: &str = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";

    fn item(listing_id: &str, watched_at_ms: i64) -> PubkyMarketWatchlistItem {
        PubkyMarketWatchlistItem {
            listing_owner_pubky: SELLER.to_string(),
            listing_id: listing_id.to_string(),
            watched_at_ms,
        }
    }

    fn tombstone(listing_id: &str, removed_at_ms: i64) -> PubkyMarketWatchlistTombstone {
        PubkyMarketWatchlistTombstone {
            listing_owner_pubky: SELLER.to_string(),
            listing_id: listing_id.to_string(),
            removed_at_ms,
        }
    }

    fn valid_watchlist() -> PubkyMarketWatchlist {
        PubkyMarketWatchlist::new(
            OWNER.to_string(),
            1,
            "2025-01-01T00:00:00Z".to_string(),
            "2025-01-02T00:00:00Z".to_string(),
            vec![item("0032SSN7Q4EVG", 1_735_689_600_000)],
            vec![tombstone("0032SSN7Q4EVH", 1_735_776_000_000)],
        )
    }

    #[test]
    fn test_create_path_is_private() {
        assert_eq!(
            PubkyMarketWatchlist::create_path(),
            "/priv/pubky.app/marketplace/v1/watchlist.json"
        );
    }

    #[test]
    fn test_validate_valid() {
        assert!(valid_watchlist().validate(None).is_ok());
    }

    #[test]
    fn test_validate_empty_lists_valid() {
        let mut watchlist = valid_watchlist();
        watchlist.items = vec![];
        watchlist.tombstones = vec![];
        assert!(watchlist.validate(None).is_ok());
    }

    #[test]
    fn test_try_from_valid_roundtrip() {
        let watchlist = valid_watchlist();
        let json = serde_json::to_string(&watchlist).unwrap();
        let parsed = <PubkyMarketWatchlist as Validatable>::try_from(json.as_bytes(), "").unwrap();
        assert_eq!(parsed, watchlist);
    }

    #[test]
    fn test_serializes_camel_case_entry_fields() {
        let watchlist = valid_watchlist();
        let value = serde_json::to_value(&watchlist).unwrap();
        assert!(value["items"][0]["watchedAtMs"].is_i64());
        assert!(value["tombstones"][0]["removedAtMs"].is_i64());
        assert!(value["items"][0]["listingOwnerPubky"].is_string());
    }

    #[test]
    fn test_try_from_accepts_unknown_field() {
        let watchlist = valid_watchlist();
        let mut value = serde_json::to_value(&watchlist).unwrap();
        value["surprise"] = serde_json::json!(true);
        let json = serde_json::to_string(&value).unwrap();
        assert!(<PubkyMarketWatchlist as Validatable>::try_from(json.as_bytes(), "").is_ok());
    }

    #[test]
    fn test_validate_wrong_record_type() {
        let mut watchlist = valid_watchlist();
        watchlist.record_type = "shop".to_string();
        assert!(watchlist.validate(None).is_err());
    }

    #[test]
    fn test_validate_rejects_duplicate_item_keys() {
        let mut watchlist = valid_watchlist();
        watchlist.items.push(item("0032SSN7Q4EVG", 1));
        assert!(watchlist.validate(None).is_err());
    }

    #[test]
    fn test_validate_rejects_key_in_items_and_tombstones() {
        let mut watchlist = valid_watchlist();
        watchlist.tombstones.push(tombstone("0032SSN7Q4EVG", 1));
        assert!(watchlist.validate(None).is_err());
    }

    #[test]
    fn test_validate_rejects_bad_timestamps() {
        let mut watchlist = valid_watchlist();
        watchlist.items[0].watched_at_ms = 0;
        assert!(watchlist.validate(None).is_err());

        let mut watchlist = valid_watchlist();
        watchlist.tombstones[0].removed_at_ms = -5;
        assert!(watchlist.validate(None).is_err());

        let mut watchlist = valid_watchlist();
        watchlist.items[0].watched_at_ms = MAX_SAFE_INTEGER + 1;
        assert!(watchlist.validate(None).is_err());
    }

    #[test]
    fn test_validate_rejects_bad_pubky_and_listing_id() {
        let mut watchlist = valid_watchlist();
        watchlist.items[0].listing_owner_pubky = "not-a-pubky".to_string();
        assert!(watchlist.validate(None).is_err());

        let mut watchlist = valid_watchlist();
        watchlist.tombstones[0].listing_id = "has space".to_string();
        assert!(watchlist.validate(None).is_err());
    }

    #[test]
    fn test_validate_rejects_over_caps() {
        let mut watchlist = valid_watchlist();
        watchlist.tombstones = vec![];
        watchlist.items = (0..=MAX_WATCHLIST_ITEMS)
            .map(|index| item(&format!("listing_{index}"), 1))
            .collect();
        assert!(watchlist.validate(None).is_err());

        let mut watchlist = valid_watchlist();
        watchlist.items = vec![];
        watchlist.tombstones = (0..=MAX_WATCHLIST_TOMBSTONES)
            .map(|index| tombstone(&format!("listing_{index}"), 1))
            .collect();
        assert!(watchlist.validate(None).is_err());
    }

    #[test]
    fn test_validate_updated_before_created_rejected() {
        let mut watchlist = valid_watchlist();
        watchlist.created_at = "2025-01-03T00:00:00Z".to_string();
        assert!(watchlist.validate(None).is_err());
    }
}
