use crate::{
    models::marketplace::{
        marketplace_media_prefix, parse_rfc3339_millis, validate_base_record, validate_entity_id,
        validate_marketplace_uri, MARKETPLACE_SCHEMA_VERSION,
    },
    traits::{HasIdPath, Validatable},
    APP_PATH, PUBLIC_PATH,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[cfg(target_arch = "wasm32")]
use crate::traits::Json;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

// Validation (mirroring the pubky-app commerce drop record schema)
const DROP_RECORD_TYPE: &str = "drop";
const MIN_TITLE_LENGTH: usize = 1;
const MAX_TITLE_LENGTH: usize = 120;
const MAX_DESCRIPTION_LENGTH: usize = 2_000;
const MAX_MEDIA: usize = 10;
const MIN_LISTINGS: usize = 1;
const MAX_LISTINGS: usize = 20;
const MIN_TOTAL_QUANTITY: i64 = 1;
const MAX_TOTAL_QUANTITY: i64 = 1_000_000;
const MIN_PER_BUYER_LIMIT: i64 = 1;
const MAX_PER_BUYER_LIMIT: i64 = 100;

/// How a drop sells. Closed-world: this version defines only `fcfs`
/// (first-come, first-served); any future format is a schema version bump,
/// so records carrying an unknown format are rejected rather than
/// misinterpreted.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub enum PubkyMarketDropFormat {
    Fcfs,
}

/// How much remaining-stock detail the seller wants the public projection to
/// reveal while the drop runs. This is the seller's DECLARED policy —
/// ENFORCEMENT is server-side: the transaction service (which holds the real
/// counters) decides what each stock query answers, and the public record
/// merely tells clients which presentation the seller chose.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub enum PubkyMarketDropStockDisplay {
    Exact,
    Bands,
    Hidden,
}

/// Represents a marketplace drop: a seller's scheduled, limited-quantity
/// release bundling one or more of their listings.
///
/// URI: /pub/pubky.app/marketplace/v1/drops/:drop_id
///
/// Where drop_id follows the marketplace entity-id convention and must match
/// the record's `dropId` field. Unlike the private order receipt, this is a
/// PUBLIC record wired into `PubkyMarketObject` and the URI parser so Nexus can
/// index it.
///
/// `startsAt`/`endsAt` are the seller's declared schedule intent; the
/// marketplace transaction service enforces the real sale window and the
/// real stock counters. `listingIds` are the seller's OWN listings: the drop
/// record owner is the listing owner by definition (both live under the same
/// pubky's `/pub/pubky.app/marketplace/v1/` tree), so no cross-owner
/// reference form exists here.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketDrop {
    /// Marketplace contract version, always `1`.
    pub schema_version: i64,
    /// Record discriminator, always `"drop"`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub record_type: String,
    /// z-base-32 pubky of the seller.
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
    /// Must match the entity id in the record's path.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub drop_id: String,
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub title: String,
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub description: String,
    /// Promotional media URIs, owned by the drop's seller (same rule as
    /// listing media URLs).
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub media: Vec<String>,
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub format: PubkyMarketDropFormat,
    /// ISO-8601 datetime with offset — the seller's declared start.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub starts_at: String,
    /// Optional ISO-8601 datetime with offset; when present it must be
    /// strictly after `starts_at`. Absent means the drop ends only by
    /// sell-out or seller cancellation.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ends_at: Option<String>,
    /// The seller's OWN listings bundled into the drop (the record owner is
    /// the listing owner by definition). 1-20 unique entity ids.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub listing_ids: Vec<String>,
    /// Total units across the drop, 1-1000000.
    pub total_quantity: i64,
    /// Per-buyer purchase cap, 1-100; never above `total_quantity`.
    pub per_buyer_limit: i64,
    /// The seller's declared stock-visibility policy (enforced server-side).
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub stock_display: PubkyMarketDropStockDisplay,
}

impl PubkyMarketDrop {
    /// Creates a new `PubkyMarketDrop` instance and sanitizes it.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        owner_pubky: String,
        revision: i64,
        created_at: String,
        updated_at: String,
        drop_id: String,
        title: String,
        description: String,
        media: Vec<String>,
        format: PubkyMarketDropFormat,
        starts_at: String,
        ends_at: Option<String>,
        listing_ids: Vec<String>,
        total_quantity: i64,
        per_buyer_limit: i64,
        stock_display: PubkyMarketDropStockDisplay,
    ) -> Self {
        Self {
            schema_version: MARKETPLACE_SCHEMA_VERSION,
            record_type: DROP_RECORD_TYPE.to_string(),
            owner_pubky,
            revision,
            created_at,
            updated_at,
            drop_id,
            title,
            description,
            media,
            format,
            starts_at,
            ends_at,
            listing_ids,
            total_quantity,
            per_buyer_limit,
            stock_display,
        }
        .sanitize()
    }
}

#[cfg(target_arch = "wasm32")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl PubkyMarketDrop {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = fromJson))]
    pub fn from_json(js_value: &JsValue) -> Result<Self, String> {
        Self::import_json(js_value)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = toJson))]
    pub fn to_json(&self) -> Result<JsValue, String> {
        self.export_json()
    }

    /// Getter for `drop_id`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter))]
    pub fn drop_id(&self) -> String {
        self.drop_id.clone()
    }

    /// Getter for `title`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter))]
    pub fn title(&self) -> String {
        self.title.clone()
    }

    /// Getter for `owner_pubky`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter))]
    pub fn owner_pubky(&self) -> String {
        self.owner_pubky.clone()
    }
}

#[cfg(target_arch = "wasm32")]
impl Json for PubkyMarketDrop {}

impl HasIdPath for PubkyMarketDrop {
    const PATH_SEGMENT: &'static str = "marketplace/v1/drops/";

    fn create_path(id: &str) -> String {
        [PUBLIC_PATH, APP_PATH, Self::PATH_SEGMENT, id].concat()
    }
}

impl Validatable for PubkyMarketDrop {
    fn sanitize(self) -> Self {
        PubkyMarketDrop {
            title: self.title.trim().to_string(),
            description: self.description.trim().to_string(),
            ..self
        }
    }

    fn validate(&self, id: Option<&str>) -> Result<(), String> {
        if let Some(id) = id {
            validate_entity_id(id, "drop path id")?;
            if self.drop_id != id {
                return Err("Validation Error: dropId does not match the drop path ID".into());
            }
        }

        validate_base_record(
            self.schema_version,
            &self.record_type,
            DROP_RECORD_TYPE,
            &self.owner_pubky,
            self.revision,
            &self.created_at,
            &self.updated_at,
        )?;
        validate_entity_id(&self.drop_id, "dropId")?;

        let title_length = self.title.chars().count();
        if !(MIN_TITLE_LENGTH..=MAX_TITLE_LENGTH).contains(&title_length) {
            return Err(format!(
                "Validation Error: drop title must be {MIN_TITLE_LENGTH}-{MAX_TITLE_LENGTH} characters"
            ));
        }
        if self.description.chars().count() > MAX_DESCRIPTION_LENGTH {
            return Err(format!(
                "Validation Error: drop description exceeds maximum length of {MAX_DESCRIPTION_LENGTH}"
            ));
        }

        // Media: same URI rule as listing media — a marketplace v1 URI owned
        // by the seller publishing the record.
        if self.media.len() > MAX_MEDIA {
            return Err(format!(
                "Validation Error: drop supports at most {MAX_MEDIA} media entries"
            ));
        }
        let media_prefix = marketplace_media_prefix(&self.owner_pubky);
        for url in &self.media {
            validate_marketplace_uri(url, "drop media entry")?;
            if !url.starts_with(&media_prefix) {
                return Err("Validation Error: drop media must be owned by the drop seller".into());
            }
        }
        ensure_unique(self.media.iter().map(String::as_str), "drop media entries")?;

        let starts = parse_rfc3339_millis(&self.starts_at)
            .map_err(|e| format!("Validation Error: startsAt {e}"))?;
        if let Some(ends_at) = &self.ends_at {
            let ends = parse_rfc3339_millis(ends_at)
                .map_err(|e| format!("Validation Error: endsAt {e}"))?;
            if ends <= starts {
                return Err("Validation Error: endsAt must be strictly after startsAt".into());
            }
        }

        if !(MIN_LISTINGS..=MAX_LISTINGS).contains(&self.listing_ids.len()) {
            return Err(format!(
                "Validation Error: drop requires {MIN_LISTINGS}-{MAX_LISTINGS} listingIds"
            ));
        }
        for listing_id in &self.listing_ids {
            validate_entity_id(listing_id, "listingIds entry")?;
        }
        ensure_unique(self.listing_ids.iter().map(String::as_str), "listingIds")?;

        if !(MIN_TOTAL_QUANTITY..=MAX_TOTAL_QUANTITY).contains(&self.total_quantity) {
            return Err(format!(
                "Validation Error: totalQuantity must be between {MIN_TOTAL_QUANTITY} and {MAX_TOTAL_QUANTITY}"
            ));
        }
        if !(MIN_PER_BUYER_LIMIT..=MAX_PER_BUYER_LIMIT).contains(&self.per_buyer_limit) {
            return Err(format!(
                "Validation Error: perBuyerLimit must be between {MIN_PER_BUYER_LIMIT} and {MAX_PER_BUYER_LIMIT}"
            ));
        }
        if self.per_buyer_limit > self.total_quantity {
            return Err("Validation Error: perBuyerLimit must not exceed totalQuantity".into());
        }

        Ok(())
    }
}

fn ensure_unique<'a, I: IntoIterator<Item = &'a str>>(values: I, what: &str) -> Result<(), String> {
    let mut seen = HashSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(format!("Validation Error: {what} must be unique"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::Validatable;

    const OWNER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
    const DROP_ID: &str = "spring-drop-01";

    fn media_uri(id: &str) -> String {
        format!("pubky://{OWNER}/pub/pubky.app/marketplace/v1/media/{id}")
    }

    fn valid_drop() -> PubkyMarketDrop {
        PubkyMarketDrop::new(
            OWNER.to_string(),
            1,
            "2026-01-01T00:00:00Z".to_string(),
            "2026-01-02T00:00:00Z".to_string(),
            DROP_ID.to_string(),
            "Spring boot drop".to_string(),
            "Limited spring release.".to_string(),
            vec![media_uri("drop_banner")],
            PubkyMarketDropFormat::Fcfs,
            "2026-02-01T00:00:00Z".to_string(),
            Some("2026-02-02T00:00:00Z".to_string()),
            vec!["listing_01".to_string(), "listing_02".to_string()],
            500,
            2,
            PubkyMarketDropStockDisplay::Bands,
        )
    }

    #[test]
    fn test_create_path() {
        assert_eq!(
            PubkyMarketDrop::create_path(DROP_ID),
            format!("/pub/pubky.app/marketplace/v1/drops/{DROP_ID}")
        );
    }

    #[test]
    fn test_validate_valid() {
        assert!(valid_drop().validate(Some(DROP_ID)).is_ok());
        assert!(valid_drop().validate(None).is_ok());
    }

    #[test]
    fn test_validate_open_ended_drop() {
        // Absent endsAt = the drop ends only by sell-out or cancellation.
        let mut drop = valid_drop();
        drop.ends_at = None;
        assert!(drop.validate(Some(DROP_ID)).is_ok());
    }

    #[test]
    fn test_try_from_valid_roundtrip() {
        let drop = valid_drop();
        let json = serde_json::to_string(&drop).unwrap();
        let parsed = <PubkyMarketDrop as Validatable>::try_from(json.as_bytes(), DROP_ID).unwrap();
        assert_eq!(parsed, drop);
    }

    #[test]
    fn test_serializes_camel_case_fields() {
        let drop = valid_drop();
        let value = serde_json::to_value(&drop).unwrap();
        assert_eq!(value["recordType"], "drop");
        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["dropId"], DROP_ID);
        assert_eq!(value["format"], "fcfs");
        assert_eq!(value["stockDisplay"], "bands");
        assert!(value["startsAt"].is_string());
        assert!(value["endsAt"].is_string());
        assert!(value["listingIds"].is_array());
        assert_eq!(value["totalQuantity"], 500);
        assert_eq!(value["perBuyerLimit"], 2);
    }

    #[test]
    fn test_absent_ends_at_is_omitted() {
        let mut drop = valid_drop();
        drop.ends_at = None;
        let value = serde_json::to_value(&drop).unwrap();
        assert!(value.get("endsAt").is_none());
    }

    #[test]
    fn test_try_from_accepts_unknown_field() {
        let drop = valid_drop();
        let mut value = serde_json::to_value(&drop).unwrap();
        value["surprise"] = serde_json::json!(true);
        let json = serde_json::to_string(&value).unwrap();
        assert!(<PubkyMarketDrop as Validatable>::try_from(json.as_bytes(), DROP_ID).is_ok());
    }

    #[test]
    fn test_validate_wrong_record_type() {
        let mut drop = valid_drop();
        drop.record_type = "listing".to_string();
        assert!(drop.validate(None).is_err());
    }

    #[test]
    fn test_validate_path_id_mismatch() {
        let drop = valid_drop();
        assert!(drop.validate(Some("another-drop")).is_err());
    }

    #[test]
    fn test_validate_bad_path_id() {
        let mut drop = valid_drop();
        drop.drop_id = "has space".to_string();
        assert!(drop.validate(Some("has space")).is_err());
        assert!(drop.validate(None).is_err());
    }

    #[test]
    fn test_validate_title_bounds() {
        let mut drop = valid_drop();
        drop.title = String::new();
        assert!(drop.validate(None).is_err());

        let mut drop = valid_drop();
        drop.title = "x".repeat(MAX_TITLE_LENGTH + 1);
        assert!(drop.validate(None).is_err());

        let mut drop = valid_drop();
        drop.title = "x".repeat(MAX_TITLE_LENGTH);
        assert!(drop.validate(None).is_ok());
    }

    #[test]
    fn test_validate_description_bounds() {
        // Empty descriptions are valid.
        let mut drop = valid_drop();
        drop.description = String::new();
        assert!(drop.validate(None).is_ok());

        let mut drop = valid_drop();
        drop.description = "x".repeat(MAX_DESCRIPTION_LENGTH + 1);
        assert!(drop.validate(None).is_err());
    }

    #[test]
    fn test_validate_media_bounds() {
        // No media is valid.
        let mut drop = valid_drop();
        drop.media = vec![];
        assert!(drop.validate(None).is_ok());

        // Too many entries.
        let mut drop = valid_drop();
        drop.media = (0..=MAX_MEDIA)
            .map(|i| media_uri(&format!("m{i}")))
            .collect();
        assert!(drop.validate(None).is_err());

        // Duplicate entries.
        let mut drop = valid_drop();
        drop.media = vec![media_uri("m1"), media_uri("m1")];
        assert!(drop.validate(None).is_err());

        // Not a marketplace URI.
        let mut drop = valid_drop();
        drop.media = vec![format!(
            "pubky://{OWNER}/pub/other.app/marketplace/v1/media/m1"
        )];
        assert!(drop.validate(None).is_err());

        // Marketplace URI owned by someone else.
        let mut drop = valid_drop();
        drop.media = vec![
            "pubky://pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pub/pubky.app/marketplace/v1/media/m1"
                .to_string(),
        ];
        assert!(drop.validate(None).is_err());
    }

    #[test]
    fn test_validate_datetime_rules() {
        let mut drop = valid_drop();
        drop.starts_at = "not a date".to_string();
        assert!(drop.validate(None).is_err());

        let mut drop = valid_drop();
        drop.ends_at = Some("not a date".to_string());
        assert!(drop.validate(None).is_err());

        // endsAt equal to startsAt is rejected — strictly after.
        let mut drop = valid_drop();
        drop.ends_at = Some(drop.starts_at.clone());
        assert!(drop.validate(None).is_err());

        // endsAt before startsAt is rejected.
        let mut drop = valid_drop();
        drop.ends_at = Some("2026-01-31T23:59:59Z".to_string());
        assert!(drop.validate(None).is_err());
    }

    #[test]
    fn test_validate_listing_ids_bounds() {
        let mut drop = valid_drop();
        drop.listing_ids = vec![];
        assert!(drop.validate(None).is_err());

        let mut drop = valid_drop();
        drop.listing_ids = (0..=MAX_LISTINGS).map(|i| format!("listing_{i}")).collect();
        assert!(drop.validate(None).is_err());

        let mut drop = valid_drop();
        drop.listing_ids = vec!["listing_01".to_string(), "listing_01".to_string()];
        assert!(drop.validate(None).is_err());

        let mut drop = valid_drop();
        drop.listing_ids = vec!["has space".to_string()];
        assert!(drop.validate(None).is_err());
    }

    #[test]
    fn test_validate_quantity_bounds() {
        let mut drop = valid_drop();
        drop.total_quantity = 0;
        assert!(drop.validate(None).is_err());

        let mut drop = valid_drop();
        drop.total_quantity = MAX_TOTAL_QUANTITY + 1;
        assert!(drop.validate(None).is_err());

        let mut drop = valid_drop();
        drop.total_quantity = MAX_TOTAL_QUANTITY;
        assert!(drop.validate(None).is_ok());
    }

    #[test]
    fn test_validate_per_buyer_limit_bounds() {
        let mut drop = valid_drop();
        drop.per_buyer_limit = 0;
        assert!(drop.validate(None).is_err());

        let mut drop = valid_drop();
        drop.per_buyer_limit = MAX_PER_BUYER_LIMIT + 1;
        assert!(drop.validate(None).is_err());

        // The cap can never exceed the total.
        let mut drop = valid_drop();
        drop.total_quantity = 3;
        drop.per_buyer_limit = 4;
        assert!(drop.validate(None).is_err());

        // Equal is allowed.
        let mut drop = valid_drop();
        drop.total_quantity = 4;
        drop.per_buyer_limit = 4;
        assert!(drop.validate(None).is_ok());
    }

    #[test]
    fn test_bad_enum_values_rejected_by_serde() {
        let drop = valid_drop();

        let mut value = serde_json::to_value(&drop).unwrap();
        value["format"] = serde_json::json!("auction");
        let json = serde_json::to_string(&value).unwrap();
        assert!(<PubkyMarketDrop as Validatable>::try_from(json.as_bytes(), DROP_ID).is_err());

        let mut value = serde_json::to_value(&drop).unwrap();
        value["stockDisplay"] = serde_json::json!("precise");
        let json = serde_json::to_string(&value).unwrap();
        assert!(<PubkyMarketDrop as Validatable>::try_from(json.as_bytes(), DROP_ID).is_err());
    }

    #[test]
    fn test_sanitize_trims_strings() {
        let mut drop = valid_drop();
        drop.title = "  Spring boot drop  ".to_string();
        drop.description = "  Limited spring release.  ".to_string();
        let sanitized = drop.sanitize();
        assert_eq!(sanitized.title, "Spring boot drop");
        assert_eq!(sanitized.description, "Limited spring release.");
    }
}
