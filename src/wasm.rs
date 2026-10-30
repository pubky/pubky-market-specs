use crate::traits::{HasIdPath, HasPath, HashId, TimestampId, Validatable};
use crate::*;
use serde_wasm_bindgen::{from_value, to_value};
use wasm_bindgen::prelude::*;

/// Each `build*` function:
/// - Accepts the record as a plain camelCase JSON object and the owner's pubky.
/// - Sanitizes and validates the record.
/// - Derives the ID where the record has one.
/// - Returns the record with its `meta` (`id`, `path`, `url`), or a descriptive error.
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct Meta {
    /// The unique ID for this object (empty if none)
    id: String,
    /// The final path
    path: String,
    /// The final url
    url: String,
}

#[wasm_bindgen]
impl Meta {
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn path(&self) -> String {
        self.path.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn url(&self) -> String {
        self.url.clone()
    }
}

impl Meta {
    fn from_object(object_id: Option<&str>, owner: &PubkyId, path: String) -> Self {
        Self {
            id: object_id.unwrap_or_default().to_string(),
            url: format!("{}{}{}", PROTOCOL, owner, path),
            path,
        }
    }
}

/// Generates a result struct holding one record and its `meta`, with getters.
/// A struct per `build*` function gives the generated TypeScript precise types.
macro_rules! result_struct {
    ($struct_name:ident, $field_name:ident, $field_type:ty) => {
        #[wasm_bindgen]
        pub struct $struct_name {
            $field_name: $field_type,
            meta: Meta,
        }

        #[wasm_bindgen]
        impl $struct_name {
            #[wasm_bindgen(getter)]
            pub fn $field_name(&self) -> $field_type {
                self.$field_name.clone()
            }

            #[wasm_bindgen(getter)]
            pub fn meta(&self) -> Meta {
                self.meta.clone()
            }
        }
    };
}

result_struct!(ShopResult, shop, PubkyMarketShop);
result_struct!(ListingResult, listing, PubkyMarketListing);
result_struct!(ReviewResult, review, PubkyMarketReview);
result_struct!(
    ReviewResponseResult,
    review_response,
    PubkyMarketReviewResponse
);
result_struct!(WatchlistResult, watchlist, PubkyMarketWatchlist);
result_struct!(OrderReceiptResult, order_receipt, PubkyMarketOrderReceipt);
result_struct!(DropResult, drop, PubkyMarketDrop);

fn owner(owner_pubky: &str) -> Result<PubkyId, String> {
    PubkyId::try_from(owner_pubky)
}

/// Builds a shop record (singleton at `/pub/pubky.app/marketplace/v1/shop.json`).
#[wasm_bindgen(js_name = buildShop)]
pub fn build_shop(owner_pubky: &str, shop: JsValue) -> Result<ShopResult, String> {
    let owner = owner(owner_pubky)?;
    let shop: PubkyMarketShop = from_value(shop).map_err(|e| e.to_string())?;
    let shop = shop.sanitize();
    shop.validate(None)?;

    let meta = Meta::from_object(None, &owner, PubkyMarketShop::create_path());
    Ok(ShopResult { shop, meta })
}

/// Builds a listing record. A fresh timestamp ID is generated and written
/// into the record's `listingId`.
#[wasm_bindgen(js_name = buildListing)]
pub fn build_listing(owner_pubky: &str, listing: JsValue) -> Result<ListingResult, String> {
    let owner = owner(owner_pubky)?;
    let listing: PubkyMarketListing = from_value(listing).map_err(|e| e.to_string())?;
    let mut listing = listing.sanitize();

    let listing_id = listing.create_id();
    listing.listing_id = listing_id.clone();
    listing.validate(Some(&listing_id))?;

    let path = PubkyMarketListing::create_path(&listing_id);
    let meta = Meta::from_object(Some(&listing_id), &owner, path);
    Ok(ListingResult { listing, meta })
}

/// Builds a review record. The hash ID is derived from the reviewed listing
/// URI, subject, role and (when present) `orderRef`, and written into the
/// record's `reviewId`.
#[wasm_bindgen(js_name = buildReview)]
pub fn build_review(owner_pubky: &str, review: JsValue) -> Result<ReviewResult, String> {
    let owner = owner(owner_pubky)?;
    let review: PubkyMarketReview = from_value(review).map_err(|e| e.to_string())?;
    let mut review = review.sanitize();

    let review_id = review.create_id();
    review.review_id = review_id.clone();
    review.validate(Some(&review_id))?;

    let path = PubkyMarketReview::create_path(&review_id);
    let meta = Meta::from_object(Some(&review_id), &owner, path);
    Ok(ReviewResult { review, meta })
}

/// Builds a review-response record. The path ID equals the subject review's
/// ID carried in the record's `reviewId`.
#[wasm_bindgen(js_name = buildReviewResponse)]
pub fn build_review_response(
    owner_pubky: &str,
    response: JsValue,
) -> Result<ReviewResponseResult, String> {
    let owner = owner(owner_pubky)?;
    let response: PubkyMarketReviewResponse = from_value(response).map_err(|e| e.to_string())?;
    let response = response.sanitize();

    let review_id = response.review_id.clone();
    response.validate(Some(&review_id))?;

    let path = PubkyMarketReviewResponse::create_path(&review_id);
    let meta = Meta::from_object(Some(&review_id), &owner, path);
    Ok(ReviewResponseResult {
        review_response: response,
        meta,
    })
}

/// Builds the private watchlist document. Its path lives under
/// `/priv/pubky.app/`: only the owner's authenticated sessions can read or
/// write it.
#[wasm_bindgen(js_name = buildWatchlist)]
pub fn build_watchlist(owner_pubky: &str, watchlist: JsValue) -> Result<WatchlistResult, String> {
    let owner = owner(owner_pubky)?;
    let watchlist: PubkyMarketWatchlist = from_value(watchlist).map_err(|e| e.to_string())?;
    let watchlist = watchlist.sanitize();
    watchlist.validate(None)?;

    let meta = Meta::from_object(None, &owner, PubkyMarketWatchlist::create_path());
    Ok(WatchlistResult { watchlist, meta })
}

/// Builds a private order receipt. The path ID equals the record's
/// `receiptId` (the transaction service's receipt UUID); the path lives under
/// `/priv/pubky.app/`.
#[wasm_bindgen(js_name = buildOrderReceipt)]
pub fn build_order_receipt(
    owner_pubky: &str,
    receipt: JsValue,
) -> Result<OrderReceiptResult, String> {
    let owner = owner(owner_pubky)?;
    let receipt: PubkyMarketOrderReceipt = from_value(receipt).map_err(|e| e.to_string())?;
    let receipt = receipt.sanitize();

    let receipt_id = receipt.receipt_id.clone();
    receipt.validate(Some(&receipt_id))?;

    let path = PubkyMarketOrderReceipt::create_path(&receipt_id);
    let meta = Meta::from_object(Some(&receipt_id), &owner, path);
    Ok(OrderReceiptResult {
        order_receipt: receipt,
        meta,
    })
}

/// Builds a drop record. The path ID equals the record's `dropId` (a
/// marketplace entity id chosen by the seller); the path is public so
/// indexers can read the drop.
#[wasm_bindgen(js_name = buildDrop)]
pub fn build_drop(owner_pubky: &str, drop: JsValue) -> Result<DropResult, String> {
    let owner = owner(owner_pubky)?;
    let drop: PubkyMarketDrop = from_value(drop).map_err(|e| e.to_string())?;
    let drop = drop.sanitize();

    let drop_id = drop.drop_id.clone();
    drop.validate(Some(&drop_id))?;

    let path = PubkyMarketDrop::create_path(&drop_id);
    let meta = Meta::from_object(Some(&drop_id), &owner, path);
    Ok(DropResult { drop, meta })
}

/// Builds the `pubky://` URI of a Market record.
///
/// `kind` is one of `shop`, `listing`, `drop`, `review`, `review_response`,
/// `watchlist`, `order_receipt`, `attestor_profile`, `attestor_annotation`.
/// Singletons (`shop`, `watchlist`, `attestor_profile`) take no `id`; every
/// other kind requires one (the listing, drop, review or receipt id, or the
/// annotation's order reference). The owner must be a valid pubky.
#[wasm_bindgen(js_name = buildMarketUri)]
pub fn build_market_uri(
    owner_pubky: &str,
    kind: &str,
    id: Option<String>,
) -> Result<String, String> {
    let owner = owner(owner_pubky)?.to_string();
    let singleton = |build: fn(String) -> String| match &id {
        None => Ok(build(owner.clone())),
        Some(_) => Err(format!("Validation Error: {kind} URIs take no id")),
    };
    let with_id = |build: fn(String, String) -> String| match &id {
        Some(id) if !id.is_empty() => Ok(build(owner.clone(), id.clone())),
        _ => Err(format!("Validation Error: {kind} URIs require an id")),
    };
    match kind {
        "shop" => singleton(shop_uri_builder),
        "watchlist" => singleton(watchlist_uri_builder),
        "attestor_profile" => singleton(attestor_profile_uri_builder),
        "listing" => with_id(listing_uri_builder),
        "drop" => with_id(drop_uri_builder),
        "review" => with_id(marketplace_review_uri_builder),
        "review_response" => with_id(review_response_uri_builder),
        "order_receipt" => with_id(order_receipt_uri_builder),
        "attestor_annotation" => with_id(attestor_annotation_uri_builder),
        other => Err(format!(
            "Validation Error: unknown Market URI kind '{other}'"
        )),
    }
}

/// Parses and structurally validates a compact JWS purchase attestation,
/// returning its claims as a plain JSON object. Does NOT verify the
/// signature — see `verifyPurchaseAttestation`.
#[wasm_bindgen(js_name = parsePurchaseAttestation)]
pub fn parse_purchase_attestation(jws: &str) -> Result<JsValue, String> {
    let attestation = PubkyMarketPurchaseAttestation::parse(jws)?;
    to_value(&attestation.claims).map_err(|e| e.to_string())
}

/// The full local verification recipe for one review record: parses the
/// record (camelCase JSON) against the spec, parses its embedded
/// attestation, verifies the Ed25519 signature against the `iss` pubky, and
/// checks the claim bindings. Returns the verified claims with wire-format
/// snake_case keys. Whether `iss` is a *trusted* attestor remains the caller's policy decision.
#[wasm_bindgen(js_name = verifyPurchaseAttestation)]
pub fn verify_purchase_attestation(review: JsValue) -> Result<JsValue, String> {
    let review: PubkyMarketReview = from_value(review).map_err(|e| e.to_string())?;
    let attestation = PubkyMarketPurchaseAttestation::verify_for_review(&review)?;
    to_value(&attestation.claims).map_err(|e| e.to_string())
}

/// The item counting key of one review record, `iss:order_ref:role:listing_owner:listing_id`:
/// runs the full verification recipe (`verifyPurchaseAttestation`) on the
/// record (camelCase JSON) and keys it by its attested order and listing.
/// Whether `iss` is a trusted attestor remains the caller's policy decision.
#[wasm_bindgen(js_name = itemCountKey)]
pub fn item_count_key(review: JsValue) -> Result<String, String> {
    let review: PubkyMarketReview = from_value(review).map_err(|e| e.to_string())?;
    let attestation = PubkyMarketPurchaseAttestation::verify_for_review(&review)?;
    Ok(review.item_count_key(&attestation)?.to_string())
}

/// The seller rating key of one review record, `iss:order_ref:role`, after
/// the same verification as `itemCountKey`. Every item review of one order
/// shares it, so `shipping` and `communication` count once per order.
#[wasm_bindgen(js_name = sellerRatingKey)]
pub fn seller_rating_key(review: JsValue) -> Result<String, String> {
    let review: PubkyMarketReview = from_value(review).map_err(|e| e.to_string())?;
    let attestation = PubkyMarketPurchaseAttestation::verify_for_review(&review)?;
    Ok(review.seller_rating_key(&attestation)?.to_string())
}

/// The 1-5 price weight of a review from its attestation's `price_decade`
/// claim (`"{CURRENCY}:{k}"`); a missing claim (`undefined` or `null`) or an
/// unknown currency weighs 1, and a malformed value throws.
#[wasm_bindgen(js_name = priceWeight)]
pub fn price_weight_js(price_decade: Option<String>) -> Result<u8, String> {
    price_weight(price_decade.as_deref())
}

/// The `price_decade` claim value for a listing unit price: `currency`,
/// `exponent` and `unitPriceMinor` (a safe-integer number or a BigInt) as
/// the listing writes them, e.g. `priceDecade("BTC", 8, 25000)` ->
/// `"BTC:4"`. Table currencies are decaded in the table's minor unit
/// (satoshis for BTC). `undefined` when there is no decade to sign.
#[wasm_bindgen(js_name = priceDecade)]
pub fn price_decade_js(
    currency: &str,
    exponent: u32,
    unit_price_minor: JsValue,
) -> Result<Option<String>, String> {
    let unit_price_minor: i64 = from_value(unit_price_minor)
        .map_err(|_| "Validation Error: unitPriceMinor must be an integer".to_string())?;
    Ok(price_decade(
        currency,
        i64::from(exponent),
        unit_price_minor,
    ))
}

/// Parses and structurally validates a compact JWS order receipt
/// attestation, returning its v1 or v2 claims as a plain JSON object with
/// the attestation's wire-format snake_case keys (`total_minor` for v1;
/// `settlement_total` and `merchandise_total` for v2). Does NOT verify the
/// signature — see `verifyOrderReceiptAttestation`.
#[wasm_bindgen(js_name = parseOrderReceiptAttestation)]
pub fn parse_order_receipt_attestation(jws: &str) -> Result<JsValue, String> {
    let attestation = PubkyMarketOrderReceiptAttestation::parse(jws)?;
    order_receipt_claims(&attestation)
}

/// The full local verification recipe for one order receipt record: parses
/// the record (camelCase JSON) against the spec, parses its embedded
/// attestation, verifies the Ed25519 signature against the `iss` pubky, and
/// checks the claim bindings. Returns the verified v1 or v2 claims with the
/// same snake_case keys. For v2, `settlement_total` is signed but is not bound
/// to a field in the receipt record. Whether `iss` is a trusted attestor
/// remains the caller's policy decision.
#[wasm_bindgen(js_name = verifyOrderReceiptAttestation)]
pub fn verify_order_receipt_attestation(receipt: JsValue) -> Result<JsValue, String> {
    let receipt: PubkyMarketOrderReceipt = from_value(receipt).map_err(|e| e.to_string())?;
    let attestation = PubkyMarketOrderReceiptAttestation::verify_for_order_receipt(&receipt)?;
    order_receipt_claims(&attestation)
}

fn order_receipt_claims(
    attestation: &PubkyMarketOrderReceiptAttestation,
) -> Result<JsValue, String> {
    match (attestation.v2_claims(), attestation.v1_claims()) {
        (Some(claims), _) => to_value(claims).map_err(|e| e.to_string()),
        (None, Some(claims)) => to_value(claims).map_err(|e| e.to_string()),
        (None, None) => Err("Validation Error: attestation carries no claims".to_string()),
    }
}

/// Parses and structurally validates a compact JWS drop edition
/// attestation, returning its claims as a plain JSON object. Does NOT
/// verify the signature — see `verifyDropEditionAttestation`.
#[wasm_bindgen(js_name = parseDropEditionAttestation)]
pub fn parse_drop_edition_attestation(jws: &str) -> Result<JsValue, String> {
    let attestation = PubkyMarketDropEditionAttestation::parse(jws)?;
    to_value(&attestation.claims).map_err(|e| e.to_string())
}

/// The full local verification recipe for one drop-order receipt record:
/// parses the record (camelCase JSON) against the spec, requires both the
/// `editionAttestation` JWS and the `drop` display object to be present,
/// verifies the Ed25519 signature against the `iss` pubky, and checks the
/// claim bindings. Returns the verified claims. Whether `iss` is a
/// *trusted* attestor remains the caller's policy decision.
#[wasm_bindgen(js_name = verifyDropEditionAttestation)]
pub fn verify_drop_edition_attestation(receipt: JsValue) -> Result<JsValue, String> {
    let receipt: PubkyMarketOrderReceipt = from_value(receipt).map_err(|e| e.to_string())?;
    let attestation =
        PubkyMarketDropEditionAttestation::verify_edition_for_order_receipt(&receipt)?;
    to_value(&attestation.claims).map_err(|e| e.to_string())
}

/// Verifies an attestor profile fetched from `uri` (its raw JSON text):
/// the URI must be an attestor profile path and the `proof` must verify
/// against the URI's owner over the canonical body. Returns the record as a
/// plain object. Whether the attestor is trusted remains the caller's
/// policy decision; a profile may only narrow that trust.
#[wasm_bindgen(js_name = verifyAttestorProfile)]
pub fn verify_attestor_profile(uri: &str, json: &str) -> Result<JsValue, String> {
    let profile = PubkyMarketAttestorProfile::from_verified_json(uri, json.as_bytes())?;
    to_value(&profile).map_err(|e| e.to_string())
}

/// Verifies an attestor annotation fetched from `uri` (its raw JSON text):
/// the URI must be an attestor annotation path, the `proof` must verify
/// against the URI's owner, and `orderRef` must equal the path ID. Returns
/// the record as a plain object.
#[wasm_bindgen(js_name = verifyAttestorAnnotation)]
pub fn verify_attestor_annotation(uri: &str, json: &str) -> Result<JsValue, String> {
    let annotation = PubkyMarketAttestorAnnotation::from_verified_json(uri, json.as_bytes())?;
    to_value(&annotation).map_err(|e| e.to_string())
}
