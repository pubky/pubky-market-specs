use crate::{
    models::marketplace::{
        parse_rfc3339_millis, validate_base_record, validate_entity_id, validate_pubky,
        validate_uuid, PubkyMarketMoney, MAX_SAFE_INTEGER,
    },
    traits::{HasIdPath, Validatable},
    APP_PATH, MARKETPLACE_PATH, PRIVATE_PATH,
};
use serde::{Deserialize, Serialize};

#[cfg(target_arch = "wasm32")]
use crate::traits::Json;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

const ORDER_RECEIPT_RECORD_TYPE: &str = "order_receipt";

// The receipt-attestation field's own bounds and charset are identical to
// the review record's `eligibilityAttestation` field.
const MIN_ATTESTATION_LENGTH: usize = 32;
const MAX_ATTESTATION_LENGTH: usize = 4_096;

/// The record owner's side of the order.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub enum PubkyMarketOrderReceiptRole {
    Buyer,
    Seller,
}

impl PubkyMarketOrderReceiptRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            PubkyMarketOrderReceiptRole::Buyer => "buyer",
            PubkyMarketOrderReceiptRole::Seller => "seller",
        }
    }
}

/// The optional drop display object on an order receipt: which drop the
/// order was part of and which numbered edition (out of the drop's total at
/// issuance) the buyer received. Present exactly when `editionAttestation`
/// is present — the object is the human-readable projection of the same
/// facts the embedded edition-attestation JWS signs (see
/// `drop_edition_attestation.rs`).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketOrderReceiptDrop {
    /// The drop's entity id (see `drop.rs`).
    pub drop_id: String,
    /// This order's edition number, 1-based.
    pub edition: i64,
    /// The drop's `totalQuantity` at issuance; never below `edition`.
    pub of: i64,
}

impl PubkyMarketOrderReceiptDrop {
    fn validate(&self) -> Result<(), String> {
        validate_entity_id(&self.drop_id, "drop dropId")?;
        if !(1..=MAX_SAFE_INTEGER).contains(&self.edition) {
            return Err("Validation Error: drop edition must be a positive safe integer".into());
        }
        if !(1..=MAX_SAFE_INTEGER).contains(&self.of) {
            return Err("Validation Error: drop of must be a positive safe integer".into());
        }
        if self.of < self.edition {
            return Err("Validation Error: drop of must not be below the edition number".into());
        }
        Ok(())
    }
}

/// A PRIVATE portable order receipt — the buyer's or seller's own durable
/// copy of a completed order, written to their OWN homeserver.
///
/// URI: /priv/pubky.app/marketplace/v1/receipts/:receipt_id
///
/// The marketplace transaction service holds the canonical order state, but
/// a service is an operator that can disappear. This record is the credible
/// exit for orders: each party keeps a signed, self-contained receipt (the
/// embedded `receiptAttestation` JWS is offline-verifiable — see
/// `order_receipt_attestation.rs`) on storage they control, so a purchase
/// history survives the operator.
///
/// Like the watchlist, this is a `/priv/` record: an order history reveals
/// counterparties, amounts, and purchase timing, so it must never be
/// world-readable, directory-listable, or Nexus-indexable. It is therefore
/// deliberately NOT wired into `PubkyMarketObject` / the URI parser used by
/// watchers and indexers: nothing under `/priv/` is index-visible, and this
/// record only ever travels between the owner's own sessions.
///
/// Unlike the watchlist singleton, receipts are one record per order under
/// `receipts/:receipt_id` (the service's receipt UUID): receipts are
/// immutable facts, not merge targets, and per-id paths let a client sync
/// incrementally instead of rewriting one growing document.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketOrderReceipt {
    /// Marketplace contract version, always `1`.
    pub schema_version: i64,
    /// Record discriminator, always `"order_receipt"`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub record_type: String,
    /// z-base-32 pubky of the record owner (a trade party; see `role`).
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
    /// The record owner's role in the order: `"buyer"` or `"seller"`.
    /// `ownerPubky` must equal the matching party pubky.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub role: PubkyMarketOrderReceiptRole,
    /// The transaction service's receipt UUID (lowercase hyphenated); must
    /// equal the id in the record's path.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub receipt_id: String,
    /// The order UUID (lowercase hyphenated) the receipt settles.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub order_id: String,
    /// z-base-32 pubky of the buyer.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub buyer_pubky: String,
    /// z-base-32 pubky of the seller.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub seller_pubky: String,
    /// Order total in integer minor units.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub total: PubkyMarketMoney,
    /// ISO-8601 datetime of payment confirmation / receipt creation.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub paid_at: String,
    /// Compact JWS attesting the receipt (see `order_receipt_attestation.rs`).
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub receipt_attestation: String,
    /// Optional compact JWS attesting the drop edition (see
    /// `drop_edition_attestation.rs`). Present exactly when `drop` is
    /// present; absent fields are not serialized, so pre-drop receipts
    /// round-trip unchanged.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edition_attestation: Option<String>,
    /// Optional drop display object. Present exactly when
    /// `editionAttestation` is present.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drop: Option<PubkyMarketOrderReceiptDrop>,
}

impl PubkyMarketOrderReceipt {
    /// Creates a new `PubkyMarketOrderReceipt` instance. The
    /// optional drop-edition pair (`edition_attestation` + `drop`) starts
    /// absent; a drop-order receipt sets both fields together on the
    /// constructed record.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        owner_pubky: String,
        revision: i64,
        created_at: String,
        updated_at: String,
        role: PubkyMarketOrderReceiptRole,
        receipt_id: String,
        order_id: String,
        buyer_pubky: String,
        seller_pubky: String,
        total: PubkyMarketMoney,
        paid_at: String,
        receipt_attestation: String,
    ) -> Self {
        Self {
            schema_version: crate::models::marketplace::MARKETPLACE_SCHEMA_VERSION,
            record_type: ORDER_RECEIPT_RECORD_TYPE.to_string(),
            owner_pubky,
            revision,
            created_at,
            updated_at,
            role,
            receipt_id,
            order_id,
            buyer_pubky,
            seller_pubky,
            total,
            paid_at,
            receipt_attestation,
            edition_attestation: None,
            drop: None,
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl PubkyMarketOrderReceipt {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = fromJson))]
    pub fn from_json(js_value: &JsValue) -> Result<Self, String> {
        Self::import_json(js_value)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = toJson))]
    pub fn to_json(&self) -> Result<JsValue, String> {
        self.export_json()
    }

    /// Getter for `receipt_id`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter))]
    pub fn receipt_id(&self) -> String {
        self.receipt_id.clone()
    }

    /// Getter for `order_id`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter))]
    pub fn order_id(&self) -> String {
        self.order_id.clone()
    }

    /// Getter for `owner_pubky`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter))]
    pub fn owner_pubky(&self) -> String {
        self.owner_pubky.clone()
    }
}

#[cfg(target_arch = "wasm32")]
impl Json for PubkyMarketOrderReceipt {}

impl HasIdPath for PubkyMarketOrderReceipt {
    const PATH_SEGMENT: &'static str = "receipts/";

    fn create_path(id: &str) -> String {
        [
            PRIVATE_PATH,
            APP_PATH,
            MARKETPLACE_PATH,
            Self::PATH_SEGMENT,
            id,
        ]
        .concat()
    }
}

impl Validatable for PubkyMarketOrderReceipt {
    fn validate(&self, id: Option<&str>) -> Result<(), String> {
        if let Some(id) = id {
            if self.receipt_id != id {
                return Err(
                    "Validation Error: receiptId does not match the receipt path ID".into(),
                );
            }
        }

        validate_base_record(
            self.schema_version,
            &self.record_type,
            ORDER_RECEIPT_RECORD_TYPE,
            &self.owner_pubky,
            self.revision,
            &self.created_at,
            &self.updated_at,
        )?;
        validate_uuid(&self.receipt_id, "receiptId")?;
        validate_uuid(&self.order_id, "orderId")?;
        validate_pubky(&self.buyer_pubky, "buyerPubky")?;
        validate_pubky(&self.seller_pubky, "sellerPubky")?;

        if self.buyer_pubky == self.seller_pubky {
            return Err("Validation Error: buyerPubky and sellerPubky must differ".into());
        }
        // The record lives on the owner's homeserver as their own copy of
        // the trade: the owner must be the party their role names.
        let expected_owner = match self.role {
            PubkyMarketOrderReceiptRole::Buyer => &self.buyer_pubky,
            PubkyMarketOrderReceiptRole::Seller => &self.seller_pubky,
        };
        if &self.owner_pubky != expected_owner {
            return Err(format!(
                "Validation Error: ownerPubky must equal {}Pubky when role is '{}'",
                self.role.as_str(),
                self.role.as_str()
            ));
        }

        self.total.validate_positive("total")?;

        parse_rfc3339_millis(&self.paid_at).map_err(|e| format!("Validation Error: paidAt {e}"))?;

        validate_attestation_field(&self.receipt_attestation, "receiptAttestation")?;

        // The drop-edition pair travels together: the JWS is the proof and
        // the object is its human-readable projection — one without the
        // other is an inconsistent record.
        match (&self.edition_attestation, &self.drop) {
            (Some(edition_attestation), Some(drop)) => {
                validate_attestation_field(edition_attestation, "editionAttestation")?;
                drop.validate()?;
            }
            (None, None) => (),
            _ => {
                return Err(
                    "Validation Error: editionAttestation and drop must be present together".into(),
                )
            }
        }

        Ok(())
    }
}

/// Validates a compact-JWS field's bounds and charset (shared by
/// `receiptAttestation` and `editionAttestation`).
fn validate_attestation_field(value: &str, field: &str) -> Result<(), String> {
    let length = value.chars().count();
    if !(MIN_ATTESTATION_LENGTH..=MAX_ATTESTATION_LENGTH).contains(&length) {
        return Err(format!(
            "Validation Error: {field} must be {MIN_ATTESTATION_LENGTH}-{MAX_ATTESTATION_LENGTH} characters"
        ));
    }
    if !value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "._~-".contains(c))
    {
        return Err(format!(
            "Validation Error: {field} must only contain characters [A-Za-z0-9._~-]"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::Validatable;

    const BUYER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
    const SELLER: &str = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";
    const RECEIPT_ID: &str = "a7fc7d5d-0b2a-4083-b278-47193f8fe536";
    const ORDER_ID: &str = "0e9c2c4a-91d6-4a4e-8db3-2f14c1e8b7aa";

    fn valid_receipt() -> PubkyMarketOrderReceipt {
        PubkyMarketOrderReceipt::new(
            BUYER.to_string(),
            1,
            "2026-01-02T03:04:05Z".to_string(),
            "2026-01-02T03:04:05Z".to_string(),
            PubkyMarketOrderReceiptRole::Buyer,
            RECEIPT_ID.to_string(),
            ORDER_ID.to_string(),
            BUYER.to_string(),
            SELLER.to_string(),
            PubkyMarketMoney {
                amount_minor: 12_000,
                currency: "USD".to_string(),
                exponent: 2,
            },
            "2026-01-02T03:04:05Z".to_string(),
            "a".repeat(64),
        )
    }

    #[test]
    fn test_create_path_is_private() {
        assert_eq!(
            PubkyMarketOrderReceipt::create_path(RECEIPT_ID),
            format!("/priv/pubky.app/marketplace/v1/receipts/{RECEIPT_ID}")
        );
    }

    #[test]
    fn test_validate_valid() {
        assert!(valid_receipt().validate(Some(RECEIPT_ID)).is_ok());
        assert!(valid_receipt().validate(None).is_ok());
    }

    #[test]
    fn test_validate_valid_seller_copy() {
        let mut receipt = valid_receipt();
        receipt.role = PubkyMarketOrderReceiptRole::Seller;
        receipt.owner_pubky = SELLER.to_string();
        assert!(receipt.validate(Some(RECEIPT_ID)).is_ok());
    }

    #[test]
    fn test_try_from_valid_roundtrip() {
        let receipt = valid_receipt();
        let json = serde_json::to_string(&receipt).unwrap();
        let parsed =
            <PubkyMarketOrderReceipt as Validatable>::try_from(json.as_bytes(), RECEIPT_ID)
                .unwrap();
        assert_eq!(parsed, receipt);
    }

    #[test]
    fn test_serializes_camel_case_fields() {
        let receipt = valid_receipt();
        let value = serde_json::to_value(&receipt).unwrap();
        assert_eq!(value["recordType"], "order_receipt");
        assert_eq!(value["role"], "buyer");
        assert_eq!(value["receiptId"], RECEIPT_ID);
        assert_eq!(value["orderId"], ORDER_ID);
        assert_eq!(value["buyerPubky"], BUYER);
        assert_eq!(value["sellerPubky"], SELLER);
        assert!(value["total"]["amountMinor"].is_i64());
        assert!(value["paidAt"].is_string());
        assert!(value["receiptAttestation"].is_string());
    }

    #[test]
    fn test_try_from_accepts_unknown_field() {
        let receipt = valid_receipt();
        let mut value = serde_json::to_value(&receipt).unwrap();
        value["surprise"] = serde_json::json!(true);
        let json = serde_json::to_string(&value).unwrap();
        assert!(
            <PubkyMarketOrderReceipt as Validatable>::try_from(json.as_bytes(), RECEIPT_ID).is_ok()
        );
    }

    #[test]
    fn test_validate_wrong_record_type() {
        let mut receipt = valid_receipt();
        receipt.record_type = "shop".to_string();
        assert!(receipt.validate(None).is_err());
    }

    #[test]
    fn test_validate_path_id_mismatch() {
        let receipt = valid_receipt();
        assert!(receipt
            .validate(Some("0e9c2c4a-91d6-4a4e-8db3-2f14c1e8b7aa"))
            .is_err());
    }

    #[test]
    fn test_validate_owner_role_mismatch() {
        // role=buyer but the owner is the seller.
        let mut receipt = valid_receipt();
        receipt.owner_pubky = SELLER.to_string();
        assert!(receipt.validate(None).is_err());

        // role=seller but the owner is the buyer.
        let mut receipt = valid_receipt();
        receipt.role = PubkyMarketOrderReceiptRole::Seller;
        assert!(receipt.validate(None).is_err());
    }

    #[test]
    fn test_validate_rejects_same_buyer_and_seller() {
        let mut receipt = valid_receipt();
        receipt.seller_pubky = BUYER.to_string();
        assert!(receipt.validate(None).is_err());
    }

    #[test]
    fn test_validate_rejects_bad_uuids() {
        let mut receipt = valid_receipt();
        receipt.receipt_id = "not-a-uuid".to_string();
        assert!(receipt.validate(None).is_err());

        let mut receipt = valid_receipt();
        receipt.order_id = "A7FC7D5D-0B2A-4083-B278-47193F8FE536".to_string();
        assert!(receipt.validate(None).is_err());

        let mut receipt = valid_receipt();
        receipt.order_id = "a7fc7d5d0b2a4083b27847193f8fe536".to_string();
        assert!(receipt.validate(None).is_err());
    }

    #[test]
    fn test_validate_rejects_bad_pubkys() {
        let mut receipt = valid_receipt();
        receipt.buyer_pubky = "not-a-pubky".to_string();
        receipt.owner_pubky = "not-a-pubky".to_string();
        assert!(receipt.validate(None).is_err());

        let mut receipt = valid_receipt();
        receipt.seller_pubky = "not-a-pubky".to_string();
        assert!(receipt.validate(None).is_err());
    }

    #[test]
    fn test_validate_rejects_bad_money() {
        let mut receipt = valid_receipt();
        receipt.total.amount_minor = 0;
        assert!(receipt.validate(None).is_err());

        let mut receipt = valid_receipt();
        receipt.total.currency = "usd".to_string();
        assert!(receipt.validate(None).is_err());

        let mut receipt = valid_receipt();
        receipt.total.exponent = 19;
        assert!(receipt.validate(None).is_err());
    }

    #[test]
    fn test_validate_rejects_bad_timestamps() {
        // updatedAt precedes createdAt.
        let mut receipt = valid_receipt();
        receipt.created_at = "2026-01-03T00:00:00Z".to_string();
        assert!(receipt.validate(None).is_err());

        let mut receipt = valid_receipt();
        receipt.paid_at = "2026-01-02 03:04:05".to_string();
        assert!(receipt.validate(None).is_err());

        let mut receipt = valid_receipt();
        receipt.paid_at = "not a date".to_string();
        assert!(receipt.validate(None).is_err());
    }

    #[test]
    fn test_validate_attestation_bounds_and_charset() {
        let mut receipt = valid_receipt();
        receipt.receipt_attestation = "too-short".to_string();
        assert!(receipt.validate(None).is_err());

        let mut receipt = valid_receipt();
        receipt.receipt_attestation = "a".repeat(4_097);
        assert!(receipt.validate(None).is_err());

        let mut receipt = valid_receipt();
        receipt.receipt_attestation = format!("{}!", "a".repeat(40));
        assert!(receipt.validate(None).is_err());
    }

    fn valid_drop_object() -> PubkyMarketOrderReceiptDrop {
        PubkyMarketOrderReceiptDrop {
            drop_id: "spring-drop-01".to_string(),
            edition: 7,
            of: 500,
        }
    }

    #[test]
    fn test_validate_valid_with_drop_edition_pair() {
        let mut receipt = valid_receipt();
        receipt.edition_attestation = Some("b".repeat(64));
        receipt.drop = Some(valid_drop_object());
        assert!(receipt.validate(Some(RECEIPT_ID)).is_ok());
    }

    #[test]
    fn test_validate_rejects_lone_edition_attestation_or_drop() {
        let mut receipt = valid_receipt();
        receipt.edition_attestation = Some("b".repeat(64));
        assert!(receipt.validate(None).is_err());

        let mut receipt = valid_receipt();
        receipt.drop = Some(valid_drop_object());
        assert!(receipt.validate(None).is_err());
    }

    #[test]
    fn test_validate_edition_attestation_bounds_and_charset() {
        let mut receipt = valid_receipt();
        receipt.drop = Some(valid_drop_object());

        receipt.edition_attestation = Some("too-short".to_string());
        assert!(receipt.validate(None).is_err());

        receipt.edition_attestation = Some("b".repeat(4_097));
        assert!(receipt.validate(None).is_err());

        receipt.edition_attestation = Some(format!("{}!", "b".repeat(40)));
        assert!(receipt.validate(None).is_err());
    }

    #[test]
    fn test_validate_drop_object_rules() {
        let base = || {
            let mut receipt = valid_receipt();
            receipt.edition_attestation = Some("b".repeat(64));
            receipt
        };

        let mut receipt = base();
        receipt.drop = Some(PubkyMarketOrderReceiptDrop {
            drop_id: "has space".to_string(),
            ..valid_drop_object()
        });
        assert!(receipt.validate(None).is_err());

        let mut receipt = base();
        receipt.drop = Some(PubkyMarketOrderReceiptDrop {
            edition: 0,
            ..valid_drop_object()
        });
        assert!(receipt.validate(None).is_err());

        let mut receipt = base();
        receipt.drop = Some(PubkyMarketOrderReceiptDrop {
            edition: 8,
            of: 7,
            ..valid_drop_object()
        });
        assert!(receipt.validate(None).is_err());

        // edition == of is the last unit of the drop, valid.
        let mut receipt = base();
        receipt.drop = Some(PubkyMarketOrderReceiptDrop {
            edition: 500,
            of: 500,
            ..valid_drop_object()
        });
        assert!(receipt.validate(None).is_ok());
    }

    #[test]
    fn test_drop_object_accepts_unknown_field() {
        let mut receipt = valid_receipt();
        receipt.edition_attestation = Some("b".repeat(64));
        receipt.drop = Some(valid_drop_object());
        let mut value = serde_json::to_value(&receipt).unwrap();
        value["drop"]["surprise"] = serde_json::json!(true);
        let json = serde_json::to_string(&value).unwrap();
        assert!(
            <PubkyMarketOrderReceipt as Validatable>::try_from(json.as_bytes(), RECEIPT_ID).is_ok()
        );
    }

    #[test]
    fn test_drop_edition_fields_serialize_camel_case() {
        let mut receipt = valid_receipt();
        receipt.edition_attestation = Some("b".repeat(64));
        receipt.drop = Some(valid_drop_object());
        let value = serde_json::to_value(&receipt).unwrap();
        assert!(value["editionAttestation"].is_string());
        assert_eq!(value["drop"]["dropId"], "spring-drop-01");
        assert_eq!(value["drop"]["edition"], 7);
        assert_eq!(value["drop"]["of"], 500);
    }

    #[test]
    fn test_pre_drop_receipt_json_round_trips_unchanged() {
        // A `.7`-shaped receipt (no editionAttestation, no drop) must parse
        // and re-serialize without gaining the new fields.
        let legacy_json = format!(
            r#"{{
                "schemaVersion": 1,
                "recordType": "order_receipt",
                "ownerPubky": "{BUYER}",
                "revision": 1,
                "createdAt": "2026-01-02T03:04:05Z",
                "updatedAt": "2026-01-02T03:04:05Z",
                "role": "buyer",
                "receiptId": "{RECEIPT_ID}",
                "orderId": "{ORDER_ID}",
                "buyerPubky": "{BUYER}",
                "sellerPubky": "{SELLER}",
                "total": {{ "amountMinor": 12000, "currency": "USD", "exponent": 2 }},
                "paidAt": "2026-01-02T03:04:05Z",
                "receiptAttestation": "{attestation}"
            }}"#,
            attestation = "a".repeat(64)
        );
        let parsed =
            <PubkyMarketOrderReceipt as Validatable>::try_from(legacy_json.as_bytes(), RECEIPT_ID)
                .unwrap();
        assert_eq!(parsed.edition_attestation, None);
        assert_eq!(parsed.drop, None);

        let reserialized = serde_json::to_value(&parsed).unwrap();
        assert!(reserialized.get("editionAttestation").is_none());
        assert!(reserialized.get("drop").is_none());
        assert_eq!(parsed, valid_receipt());
    }

    #[test]
    fn test_validate_wrong_role_string_rejected_by_serde() {
        let receipt = valid_receipt();
        let mut value = serde_json::to_value(&receipt).unwrap();
        value["role"] = serde_json::json!("courier");
        let json = serde_json::to_string(&value).unwrap();
        assert!(
            <PubkyMarketOrderReceipt as Validatable>::try_from(json.as_bytes(), RECEIPT_ID)
                .is_err()
        );
    }
}
