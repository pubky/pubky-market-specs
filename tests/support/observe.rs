// Shared by `tests/vectors.rs` (this crate only) and the parity tool in
// `parity/` (this crate and `pubky-app-specs` side by side), so both record
// exactly the same observations for a record.

/// The record kinds the vectors cover, named as in `buildMarketUri`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Shop,
    Listing,
    Drop,
    Review,
    ReviewResponse,
    Watchlist,
    OrderReceipt,
    AttestorProfile,
    AttestorAnnotation,
}

impl Kind {
    pub fn parse(kind: &str) -> Kind {
        match kind {
            "shop" => Kind::Shop,
            "listing" => Kind::Listing,
            "drop" => Kind::Drop,
            "review" => Kind::Review,
            "review_response" => Kind::ReviewResponse,
            "watchlist" => Kind::Watchlist,
            "order_receipt" => Kind::OrderReceipt,
            "attestor_profile" => Kind::AttestorProfile,
            "attestor_annotation" => Kind::AttestorAnnotation,
            other => panic!("unknown kind {other}"),
        }
    }
}

/// `ok:<serde_json output>` on success, `err` on failure. Error texts are
/// not part of the wire format and are not compared.
pub fn outcome<T: serde::Serialize, E>(result: Result<T, E>) -> String {
    match result {
        Ok(value) => format!(
            "ok:{}",
            serde_json::to_string(&value).expect("a parsed record serializes")
        ),
        Err(_) => "err".to_string(),
    }
}

/// Builds the record URI with the named crate's builders (the builder names
/// are the same in both crates).
macro_rules! record_uri {
    ($c:ident, $kind:expr, $owner:expr, $id:expr) => {{
        let owner: String = $owner.to_string();
        let id: String = $id.to_string();
        match $kind {
            Kind::Shop => $c::shop_uri_builder(owner),
            Kind::Listing => $c::listing_uri_builder(owner, id),
            Kind::Drop => $c::drop_uri_builder(owner, id),
            Kind::Review => $c::marketplace_review_uri_builder(owner, id),
            Kind::ReviewResponse => $c::review_response_uri_builder(owner, id),
            Kind::Watchlist => $c::watchlist_uri_builder(owner),
            Kind::OrderReceipt => $c::order_receipt_uri_builder(owner, id),
            Kind::AttestorProfile => $c::attestor_profile_uri_builder(owner),
            Kind::AttestorAnnotation => $c::attestor_annotation_uri_builder(owner, id),
        }
    }};
}

/// Defines `fn $name(kind, uri, id, blob) -> Vec<(&str, String)>`: every
/// observable outcome of reading `blob` as a `kind` record at `uri`, with the
/// named crate and its type names.
macro_rules! observer {
    (
        $name:ident, $c:ident,
        object: $Object:ident,
        shop: $Shop:ident,
        listing: $Listing:ident,
        drop: $Drop:ident,
        review: $Review:ident,
        response: $Response:ident,
        watchlist: $Watchlist:ident,
        receipt: $Receipt:ident,
        profile: $Profile:ident,
        annotation: $Annotation:ident,
        purchase: $Purchase:ident,
        receipt_attestation: $ReceiptAtt:ident,
        edition_attestation: $EditionAtt:ident
    ) => {
        #[allow(unreachable_patterns)]
        fn object_json(object: $c::$Object) -> String {
            let json = match object {
                $c::$Object::Shop(r) => serde_json::to_string(&r),
                $c::$Object::Listing(r) => serde_json::to_string(&r),
                $c::$Object::Drop(r) => serde_json::to_string(&r),
                $c::$Object::MarketplaceReview(r) => serde_json::to_string(&r),
                $c::$Object::ReviewResponse(r) => serde_json::to_string(&r),
                $c::$Object::AttestorProfile(r) => serde_json::to_string(&r),
                $c::$Object::AttestorAnnotation(r) => serde_json::to_string(&r),
                _ => return "not a market record".to_string(),
            };
            format!("ok:{}", json.expect("a parsed record serializes"))
        }

        pub fn $name(kind: Kind, uri: &str, id: &str, blob: &[u8]) -> Vec<(&'static str, String)> {
            use $c::traits::{HashId, Validatable};
            let mut out: Vec<(&'static str, String)> = Vec::new();
            let object = || match $c::$Object::from_uri(uri, blob) {
                Ok(object) => object_json(object),
                Err(_) => "err".to_string(),
            };
            match kind {
                Kind::Shop => {
                    out.push(("validated", outcome(<$c::$Shop as Validatable>::try_from(blob, id))));
                    out.push(("raw", outcome(serde_json::from_slice::<$c::$Shop>(blob))));
                    out.push(("object", object()));
                }
                Kind::Listing => {
                    out.push(("validated", outcome(<$c::$Listing as Validatable>::try_from(blob, id))));
                    out.push(("raw", outcome(serde_json::from_slice::<$c::$Listing>(blob))));
                    out.push(("object", object()));
                }
                Kind::Drop => {
                    out.push(("validated", outcome(<$c::$Drop as Validatable>::try_from(blob, id))));
                    out.push(("raw", outcome(serde_json::from_slice::<$c::$Drop>(blob))));
                    out.push(("object", object()));
                }
                Kind::Review => {
                    out.push(("validated", outcome(<$c::$Review as Validatable>::try_from(blob, id))));
                    let raw = serde_json::from_slice::<$c::$Review>(blob);
                    out.push(("raw", outcome(raw.as_ref())));
                    out.push(("object", object()));
                    if let Ok(review) = raw {
                        out.push(("hash id", review.create_id()));
                        out.push((
                            "attestation",
                            outcome($c::$Purchase::parse(&review.eligibility_attestation).map(|a| a.claims)),
                        ));
                        match $c::$Purchase::verify_for_review(&review) {
                            Ok(attestation) => {
                                out.push(("verified", outcome(Ok::<_, ()>(&attestation.claims))));
                                out.push((
                                    "item count key",
                                    outcome(review.item_count_key(&attestation).map(|k| k.to_string())),
                                ));
                                out.push((
                                    "seller rating key",
                                    outcome(review.seller_rating_key(&attestation).map(|k| k.to_string())),
                                ));
                                out.push(("price weight", attestation.price_weight().to_string()));
                            }
                            Err(_) => out.push(("verified", "err".to_string())),
                        }
                    }
                }
                Kind::ReviewResponse => {
                    out.push(("validated", outcome(<$c::$Response as Validatable>::try_from(blob, id))));
                    out.push(("raw", outcome(serde_json::from_slice::<$c::$Response>(blob))));
                    out.push(("object", object()));
                }
                Kind::Watchlist => {
                    out.push(("validated", outcome(<$c::$Watchlist as Validatable>::try_from(blob, id))));
                    out.push(("raw", outcome(serde_json::from_slice::<$c::$Watchlist>(blob))));
                }
                Kind::OrderReceipt => {
                    out.push(("validated", outcome(<$c::$Receipt as Validatable>::try_from(blob, id))));
                    let raw = serde_json::from_slice::<$c::$Receipt>(blob);
                    out.push(("raw", outcome(raw.as_ref())));
                    if let Ok(receipt) = raw {
                        let claims = |a: &$c::$ReceiptAtt| match (a.v1_claims(), a.v2_claims()) {
                            (Some(v1), _) => outcome(Ok::<_, ()>(v1)),
                            (_, Some(v2)) => outcome(Ok::<_, ()>(v2)),
                            _ => "no claims".to_string(),
                        };
                        out.push((
                            "attestation",
                            match $c::$ReceiptAtt::parse(&receipt.receipt_attestation) {
                                Ok(a) => claims(&a),
                                Err(_) => "err".to_string(),
                            },
                        ));
                        out.push((
                            "verified",
                            match $c::$ReceiptAtt::verify_for_order_receipt(&receipt) {
                                Ok(a) => claims(&a),
                                Err(_) => "err".to_string(),
                            },
                        ));
                        out.push((
                            "edition verified",
                            outcome($c::$EditionAtt::verify_edition_for_order_receipt(&receipt).map(|a| a.claims)),
                        ));
                    }
                }
                Kind::AttestorProfile => {
                    out.push(("validated", outcome($c::$Profile::from_verified_json(uri, blob))));
                    let raw = serde_json::from_slice::<$c::$Profile>(blob);
                    out.push(("raw", outcome(raw.as_ref())));
                    out.push(("object", object()));
                    if let Ok(record) = raw {
                        out.push(("canonical body", outcome(record.canonical_body())));
                        out.push(("canonical json", outcome(record.to_canonical_json())));
                    }
                }
                Kind::AttestorAnnotation => {
                    out.push(("validated", outcome($c::$Annotation::from_verified_json(uri, blob))));
                    let raw = serde_json::from_slice::<$c::$Annotation>(blob);
                    out.push(("raw", outcome(raw.as_ref())));
                    out.push(("object", object()));
                    if let Ok(record) = raw {
                        out.push(("canonical body", outcome(record.canonical_body())));
                        out.push(("canonical json", outcome(record.to_canonical_json())));
                    }
                }
            }
            out
        }
    };
}
