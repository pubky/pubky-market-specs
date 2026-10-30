//! Replays `test-vectors/market-records-v1.json`: every expected value is
//! what `pubky-app-specs` (`v0.6.2-marketplace.16`) returned for the same
//! input, so a pass means this crate reads and writes those records
//! byte-for-byte the same. `parity/` regenerates the file and compares both
//! crates on the full mutation set.

include!("support/observe.rs");

mod ours_side {
    use super::*;
    observer!(
        observe, pubky_market_specs,
        object: PubkyMarketObject,
        shop: PubkyMarketShop,
        listing: PubkyMarketListing,
        drop: PubkyMarketDrop,
        review: PubkyMarketReview,
        response: PubkyMarketReviewResponse,
        watchlist: PubkyMarketWatchlist,
        receipt: PubkyMarketOrderReceipt,
        profile: PubkyMarketAttestorProfile,
        annotation: PubkyMarketAttestorAnnotation,
        purchase: PubkyMarketPurchaseAttestation,
        receipt_attestation: PubkyMarketOrderReceiptAttestation,
        edition_attestation: PubkyMarketDropEditionAttestation
    );
}

use pubky_market_specs as ours;
use serde_json::Value;

fn vectors() -> Value {
    serde_json::from_str(include_str!("../test-vectors/market-records-v1.json"))
        .expect("vectors file parses")
}

#[test]
fn record_vectors_match_pubky_app_specs() {
    let document = vectors();
    let records = document["records"].as_array().unwrap();
    assert!(records.len() > 500, "vectors file is complete");
    let mut seeds_accepted = 0;
    for vector in records {
        let kind = Kind::parse(vector["kind"].as_str().unwrap());
        let owner = vector["owner"].as_str().unwrap();
        let id = vector["id"].as_str().unwrap_or("");
        let uri = vector["uri"].as_str().unwrap();
        assert_eq!(record_uri!(pubky_market_specs, kind, owner, id), uri);

        let input = vector["input"].as_str().unwrap();
        let observed = ours_side::observe(kind, uri, id, input.as_bytes());
        let expected = vector["expected"].as_object().unwrap();
        let label = format!("{} / {}", vector["seed"], vector["case"]);
        assert_eq!(observed.len(), expected.len(), "{label}: observation set");
        for (key, value) in observed {
            assert_eq!(
                Some(value.as_str()),
                expected[key].as_str(),
                "{label}: {key}"
            );
        }
        if vector["case"] == "seed" {
            assert!(
                expected["validated"].as_str().unwrap().starts_with("ok:"),
                "{label}"
            );
            seeds_accepted += 1;
        }
    }
    assert_eq!(seeds_accepted, 18, "every seed record is valid");
}

#[test]
fn forged_signatures_are_rejected() {
    let document = vectors();
    let mut forged = 0;
    for vector in document["records"].as_array().unwrap() {
        let Some(key) = vector["forgedObservation"].as_str() else {
            continue;
        };
        forged += 1;
        let kind = Kind::parse(vector["kind"].as_str().unwrap());
        let observed = ours_side::observe(
            kind,
            vector["uri"].as_str().unwrap(),
            vector["id"].as_str().unwrap_or(""),
            vector["input"].as_str().unwrap().as_bytes(),
        );
        let value = observed.iter().find(|(k, _)| *k == key).map(|(_, v)| v);
        assert_eq!(
            value.map(String::as_str),
            Some("err"),
            "{}: {key}",
            vector["case"]
        );
    }
    assert_eq!(forged, 22, "every forgery is in the vectors file");
}

#[test]
fn uri_vectors_match_pubky_app_specs() {
    for vector in vectors()["uris"].as_array().unwrap() {
        let uri = vector["uri"].as_str().unwrap();
        let observed = match ours::ParsedUri::try_from(uri) {
            Ok(parsed) => format!(
                "ok:{} {} {} {:?}",
                parsed.user_id,
                serde_json::to_value(&parsed.resource).unwrap(),
                parsed.resource,
                parsed.resource.id()
            ),
            Err(_) => "err".to_string(),
        };
        assert_eq!(observed, vector["expected"].as_str().unwrap(), "{uri}");
    }
}

#[test]
fn pubky_id_vectors_match_pubky_app_specs() {
    for vector in vectors()["pubkyIds"].as_array().unwrap() {
        let input = vector["input"].as_str().unwrap();
        let observed = outcome(ours::PubkyId::try_from(input).map(|p| p.to_string()));
        assert_eq!(observed, vector["expected"].as_str().unwrap(), "{input:?}");
    }
}

#[test]
fn price_vectors_match_pubky_app_specs() {
    for vector in vectors()["price"].as_array().unwrap() {
        if let Some(currency) = vector.get("currency") {
            let observed = ours::price_decade(
                currency.as_str().unwrap(),
                vector["exponent"].as_i64().unwrap(),
                vector["unitPriceMinor"].as_i64().unwrap(),
            );
            assert_eq!(
                observed.as_deref(),
                vector["expected"].as_str(),
                "price_decade {vector}"
            );
        } else {
            let observed = outcome(ours::price_weight(vector["priceDecade"].as_str()));
            assert_eq!(
                observed,
                vector["expected"].as_str().unwrap(),
                "price_weight {vector}"
            );
        }
    }
}
