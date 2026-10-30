//! Parity between `pubky-market-specs` and the marketplace build of
//! `pubky-app-specs` it replaces (tag `v0.6.2-marketplace.16`, rev `5ef6e31e`).
//!
//! `check` (default) compares both crates on every seed record, on every
//! structural mutation of every seed, on attestations re-signed after their
//! claims or header were mutated, on attestor records re-signed after
//! mutation, on URIs, public keys, price decades and weights, constants and
//! paths, and confirms `test-vectors/market-records-v1.json` is current.
//! `write-vectors` regenerates that file from `pubky-app-specs`.
//! `corpus <file>` writes every record case as JSON for the JS parity check.

mod mutate;

include!("../../tests/support/observe.rs");

mod fork_side {
    use super::*;
    observer!(
        observe, pubky_app_specs,
        object: PubkyAppObject,
        shop: PubkyAppShop,
        listing: PubkyAppListing,
        drop: PubkyAppMarketplaceDrop,
        review: PubkyAppMarketplaceReview,
        response: PubkyAppReviewResponse,
        watchlist: PubkyAppWatchlist,
        receipt: PubkyAppMarketplaceOrderReceipt,
        profile: PubkyAppAttestorProfile,
        annotation: PubkyAppAttestorAnnotation,
        purchase: PubkyAppPurchaseAttestation,
        receipt_attestation: PubkyAppOrderReceiptAttestation,
        edition_attestation: PubkyAppDropEditionAttestation
    );
}

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

use ed25519_dalek::{Signer, SigningKey};
use pubky_app_specs as fork;
use pubky_market_specs as ours;
use serde_json::{json, Value};
use std::path::PathBuf;

const FORK_TAG: &str = "v0.6.2-marketplace.16";
const FORK_REV: &str = "5ef6e31e89530a58e66044a08fb6baf704adabc7";
/// The key the seed attestations are signed with (test-only).
const TEST_ATTESTOR_SECRET: [u8; 32] = [7u8; 32];
/// A second test-only key: a valid signature by it is a forgery.
const OTHER_SECRET: [u8; 32] = [9u8; 32];
const JWS_FIELDS: [&str; 3] = [
    "eligibilityAttestation",
    "receiptAttestation",
    "editionAttestation",
];
const VECTORS_FILE: &str = "test-vectors/market-records-v1.json";
/// One in this many mutation cases of each seed goes into the vectors file;
/// `check` compares all of them live.
const VECTOR_SAMPLE_EVERY: usize = 20;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("parity/ lives inside the repo")
        .to_path_buf()
}

struct Seed {
    name: String,
    kind_name: String,
    kind: Kind,
    owner: String,
    id: Option<String>,
    record: Value,
}

fn load_seeds() -> Vec<Seed> {
    let dir = repo_root().join("test-vectors/seeds");
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    manifest["seeds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            let name = entry["name"].as_str().unwrap().to_string();
            let record: Value = serde_json::from_str(
                &std::fs::read_to_string(dir.join(format!("{name}.json"))).unwrap(),
            )
            .unwrap();
            let kind_name = entry["kind"].as_str().unwrap().to_string();
            Seed {
                kind: Kind::parse(&kind_name),
                kind_name,
                owner: entry["owner"].as_str().unwrap().to_string(),
                id: entry["id"].as_str().map(str::to_string),
                record,
                name,
            }
        })
        .collect()
}

struct Case {
    seed: String,
    label: String,
    kind_name: String,
    kind: Kind,
    owner: String,
    id: Option<String>,
    blob: Vec<u8>,
    /// For a forgery: the observation that must be `err` in both crates
    /// while it is `ok` for the seed.
    forged: Option<&'static str>,
}

fn base64url_decode(input: &str) -> Option<Vec<u8>> {
    let mut bits: u32 = 0;
    let mut count = 0;
    let mut out = Vec::new();
    for c in input.bytes() {
        let value = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return None,
        } as u32;
        bits = (bits << 6) | value;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    Some(out)
}

fn sign_compact(header: &Value, claims: &Value, key: &SigningKey) -> String {
    let header_b64 = ours::base64url_encode(&serde_json::to_vec(header).unwrap());
    let payload_b64 = ours::base64url_encode(&serde_json::to_vec(claims).unwrap());
    let signing_input = format!("{header_b64}.{payload_b64}");
    let signature = key.sign(signing_input.as_bytes());
    format!(
        "{signing_input}.{}",
        ours::base64url_encode(&signature.to_bytes())
    )
}

/// The JWS fields of `seed` signed with the test key, as (field, header, claims).
fn test_signed_jws(seed: &Seed) -> Vec<(&'static str, Value, Value)> {
    let key = SigningKey::from_bytes(&TEST_ATTESTOR_SECRET);
    let mut out = Vec::new();
    for field in [
        "eligibilityAttestation",
        "receiptAttestation",
        "editionAttestation",
    ] {
        let Some(jws) = seed.record.get(field).and_then(Value::as_str) else {
            continue;
        };
        let parts: Vec<&str> = jws.split('.').collect();
        if parts.len() != 3 {
            continue;
        }
        let (Some(header), Some(payload)) =
            (base64url_decode(parts[0]), base64url_decode(parts[1]))
        else {
            continue;
        };
        let (Ok(header), Ok(claims)) = (
            serde_json::from_slice::<Value>(&header),
            serde_json::from_slice::<Value>(&payload),
        ) else {
            continue;
        };
        if sign_compact(&header, &claims, &key) == jws {
            out.push((field, header, claims));
        }
    }
    out
}

fn attestor_secret() -> [u8; 32] {
    let fixture: Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("src/test_fixtures/attestor-records-v1.json"))
            .unwrap(),
    )
    .unwrap();
    let hex = fixture["attestorSecretKeyHex"].as_str().unwrap();
    let mut secret = [0u8; 32];
    for (i, byte) in secret.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap();
    }
    secret
}

/// Re-signs a mutated attestor record with `pubky-app-specs`, so the proof
/// stays valid and only the mutation is under test. Records the fork cannot
/// parse are kept as they are (both crates must reject them).
fn resign_attestor(kind: Kind, record: &Value, secret: &[u8; 32]) -> Vec<u8> {
    let blob = serde_json::to_vec(record).unwrap();
    let signed = match kind {
        Kind::AttestorProfile => serde_json::from_slice::<fork::PubkyAppAttestorProfile>(&blob)
            .ok()
            .and_then(|r| r.sign(secret).ok())
            .map(|r| serde_json::to_vec(&r).unwrap()),
        Kind::AttestorAnnotation => {
            serde_json::from_slice::<fork::PubkyAppAttestorAnnotation>(&blob)
                .ok()
                .and_then(|r| r.sign(secret).ok())
                .map(|r| serde_json::to_vec(&r).unwrap())
        }
        _ => None,
    };
    signed.unwrap_or(blob)
}

/// The observation that carries the signature check of a JWS field.
fn signature_observation(field: &str) -> &'static str {
    match field {
        "editionAttestation" => "edition verified",
        _ => "verified",
    }
}

/// Records whose only defect is a bad signature or proof: the claims and
/// bindings are those of the valid seed. Each comes with the observation
/// that must reject it.
fn forgeries(seed: &Seed) -> Vec<(String, Value, &'static str)> {
    let other = SigningKey::from_bytes(&OTHER_SECRET);
    let mut out = Vec::new();
    for field in JWS_FIELDS {
        let Some(jws) = seed.record.get(field).and_then(Value::as_str) else {
            continue;
        };
        let parts: Vec<&str> = jws.split('.').collect();
        if parts.len() != 3 {
            continue;
        }
        let Some(mut signature) = base64url_decode(parts[2]) else {
            continue;
        };
        for (byte, bit) in [(0, 0x01), (31, 0x80), (63, 0x40)] {
            if let Some(b) = signature.get_mut(byte) {
                *b ^= bit;
                let mut record = seed.record.clone();
                record[field] = json!(format!(
                    "{}.{}.{}",
                    parts[0],
                    parts[1],
                    ours::base64url_encode(&signature)
                ));
                out.push((
                    format!("forged: {field} signature byte {byte} bit flipped"),
                    record,
                    signature_observation(field),
                ));
                let b = signature.get_mut(byte).unwrap();
                *b ^= bit;
            }
        }
    }
    for (field, header, claims) in test_signed_jws(seed) {
        let mut record = seed.record.clone();
        record[field] = json!(sign_compact(&header, &claims, &other));
        out.push((
            format!("forged: {field} signed by another key"),
            record,
            signature_observation(field),
        ));
    }
    if matches!(seed.kind, Kind::AttestorProfile | Kind::AttestorAnnotation) {
        let resigned = resign_attestor(seed.kind, &seed.record, &OTHER_SECRET);
        let record: Value = serde_json::from_slice(&resigned).unwrap();
        assert_ne!(
            record["proof"], seed.record["proof"],
            "re-signing changes the proof"
        );
        out.push((
            "forged: proof by another key".to_string(),
            record,
            "validated",
        ));
        let mut flipped = seed.record.clone();
        let proof = flipped["proof"].as_str().unwrap_or_default().to_string();
        if let Some((head, signature)) = proof.rsplit_once('.') {
            if let Some(mut bytes) = base64url_decode(signature) {
                if let Some(b) = bytes.first_mut() {
                    *b ^= 0x01;
                    flipped["proof"] = json!(format!("{head}.{}", ours::base64url_encode(&bytes)));
                    out.push((
                        "forged: proof signature bit flipped".to_string(),
                        flipped,
                        "validated",
                    ));
                }
            }
        }
    }
    out
}

fn build_cases(seeds: &[Seed]) -> Vec<Case> {
    let secret = attestor_secret();
    let key = SigningKey::from_bytes(&TEST_ATTESTOR_SECRET);
    let mut cases = Vec::new();
    for seed in seeds {
        let attestor = matches!(seed.kind, Kind::AttestorProfile | Kind::AttestorAnnotation);
        let push_case =
            |cases: &mut Vec<Case>, label: String, blob: Vec<u8>, forged: Option<&'static str>| {
                cases.push(Case {
                    seed: seed.name.clone(),
                    label,
                    kind_name: seed.kind_name.clone(),
                    kind: seed.kind,
                    owner: seed.owner.clone(),
                    id: seed.id.clone(),
                    blob,
                    forged,
                })
            };
        let push = |cases: &mut Vec<Case>, label: String, blob: Vec<u8>| {
            push_case(cases, label, blob, None)
        };
        push(
            &mut cases,
            "seed".into(),
            serde_json::to_vec(&seed.record).unwrap(),
        );
        for (label, mutated) in mutate::mutations(&seed.record) {
            let blob = if attestor && !label.starts_with("$.proof ") {
                resign_attestor(seed.kind, &mutated, &secret)
            } else {
                serde_json::to_vec(&mutated).unwrap()
            };
            push(&mut cases, label, blob);
        }
        for (field, header, claims) in test_signed_jws(seed) {
            for (label, mutated) in mutate::mutations(&claims) {
                let mut record = seed.record.clone();
                record[field] = json!(sign_compact(&header, &mutated, &key));
                push(
                    &mut cases,
                    format!("{field} claims {label} (re-signed)"),
                    serde_json::to_vec(&record).unwrap(),
                );
            }
            for (label, mutated) in mutate::mutations(&header) {
                let mut record = seed.record.clone();
                record[field] = json!(sign_compact(&mutated, &claims, &key));
                push(
                    &mut cases,
                    format!("{field} header {label} (re-signed)"),
                    serde_json::to_vec(&record).unwrap(),
                );
            }
        }
        for (label, record, observation) in forgeries(seed) {
            push_case(
                &mut cases,
                label,
                serde_json::to_vec(&record).unwrap(),
                Some(observation),
            );
        }
    }
    cases
}

fn case_uri(case: &Case) -> String {
    record_uri!(
        pubky_app_specs,
        case.kind,
        case.owner,
        case.id.clone().unwrap_or_default()
    )
}

#[derive(Default)]
struct Report {
    lines: Vec<String>,
    mismatches: Vec<String>,
}

impl Report {
    fn section(&mut self, name: &str, compared: usize, extra: &str) {
        self.lines
            .push(format!("{name}: {compared} compared{extra}"));
    }
    fn mismatch(&mut self, what: String) {
        if self.mismatches.len() < 200 {
            self.mismatches.push(what);
        }
    }
}

fn check_records(cases: &[Case], report: &mut Report) {
    let mut observations = 0;
    let mut accepted = 0;
    let mut verified = 0;
    let mut failures = 0;
    let mut forged = 0;
    let observed = |case: &Case| {
        let uri = case_uri(case);
        fork_side::observe(
            case.kind,
            &uri,
            case.id.as_deref().unwrap_or(""),
            &case.blob,
        )
    };
    let seed_observations: std::collections::HashMap<&str, Vec<(&'static str, String)>> = cases
        .iter()
        .filter(|case| case.label == "seed")
        .map(|case| (case.seed.as_str(), observed(case)))
        .collect();
    for case in cases {
        let uri = case_uri(case);
        let ours_uri = record_uri!(
            pubky_market_specs,
            case.kind,
            case.owner,
            case.id.clone().unwrap_or_default()
        );
        if uri != ours_uri {
            failures += 1;
            report.mismatch(format!(
                "{} {}: uri {uri} != {ours_uri}",
                case.seed, case.label
            ));
        }
        let id = case.id.as_deref().unwrap_or("");
        let fork = fork_side::observe(case.kind, &uri, id, &case.blob);
        let ours = ours_side::observe(case.kind, &uri, id, &case.blob);
        if fork.first().is_some_and(|(_, v)| v.starts_with("ok:")) {
            accepted += 1;
        }
        verified += fork
            .iter()
            .filter(|(k, v)| k.ends_with("verified") && v.starts_with("ok:"))
            .count();
        observations += fork.len();
        if let Some(key) = case.forged {
            forged += 1;
            let value = |obs: &[(&'static str, String)]| {
                obs.iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone())
            };
            let seed_value = value(&seed_observations[case.seed.as_str()]);
            let forged_value = value(&fork);
            if !seed_value.as_deref().is_some_and(|v| v.starts_with("ok:"))
                || forged_value.as_deref() != Some("err")
            {
                failures += 1;
                report.mismatch(format!(
                    "{} {}: harness: pubky-app-specs must accept the seed and reject this forgery on `{key}` (seed {seed_value:?}, forgery {forged_value:?})",
                    case.seed, case.label
                ));
            }
        }
        if fork != ours {
            failures += 1;
            let detail = fork
                .iter()
                .zip(ours.iter())
                .find(|(a, b)| a != b)
                .map(|((k, a), (_, b))| format!("{k}: fork {a:.160} | ours {b:.160}"))
                .unwrap_or_else(|| "different observation sets".to_string());
            report.mismatch(format!("{} {}: {detail}", case.seed, case.label));
        }
    }
    report.section(
        "records",
        cases.len(),
        &format!(
            " ({accepted} accepted by pubky-app-specs, {verified} attestations verified, {forged} forgeries rejected, {observations} observations, {failures} differ)"
        ),
    );
}

const OWNERS: [&str; 3] = [
    "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo",
    "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy",
    "7jfgaa9nutjyixzikb7tgmsf9gkwq7iqz498zr1nd5ig1fng4esy",
];

fn uri_cases() -> Vec<String> {
    let ids = [
        "0032SSN7Q4EVG",
        "spring-drop-01",
        "8Z8CWH8NVYQY39ZEBFGKQWWEKG",
        "",
        "a%20b",
        "x?y=1",
        "x#frag",
        "UPPER",
        "a.b",
        "\u{00e9}",
    ];
    let mut paths: Vec<String> = Vec::new();
    for id in ids {
        for collection in [
            "listings",
            "drops",
            "reviews",
            "review_responses",
            "orders",
            "media",
            "shop.json",
        ] {
            paths.push(format!("marketplace/v1/{collection}/{id}"));
            paths.push(format!("marketplace/v1/{collection}/{id}/extra"));
            paths.push(format!("marketplace/v2/{collection}/{id}"));
        }
        paths.push(format!("marketplace/v1/attestor/annotations/{id}"));
        paths.push(format!("marketplace/v1/attestor/annotations/{id}/extra"));
        for social in ["posts", "tags", "bookmarks", "files", "blobs", "feeds"] {
            paths.push(format!("{social}/{id}"));
        }
    }
    for fixed in [
        "",
        "marketplace",
        "marketplace/v1",
        "marketplace/v1/",
        "marketplace/v1/shop.json",
        "marketplace/v1/shop.json/",
        "marketplace/v1/attestor",
        "marketplace/v1/attestor/profile",
        "marketplace/v1/attestor/profile/",
        "marketplace/v1/attestor/other",
        "marketplace/v1/attestor/annotations",
        "marketplace/v1/attestor/annotations/",
        "marketplace/v1/watchlist.json",
        "marketplace/v1/receipts/abc",
        "profile.json",
        "last_read",
        "follows/pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy",
        "follows/not-a-key",
        "mutes/not-a-key",
    ] {
        paths.push(fixed.to_string());
    }
    let mut uris = Vec::new();
    for owner in OWNERS.iter().copied().chain([
        "not-a-key",
        "OPERRR8WSBPR3UE9D4QJ41GE1KCC6R7FDIY6O3UGJRRHI4Y77RDO",
    ]) {
        for path in &paths {
            uris.push(format!("pubky://{owner}/pub/pubky.app/{path}"));
        }
        uris.push(format!(
            "PUBKY://{owner}/pub/pubky.app/marketplace/v1/shop.json"
        ));
        uris.push(format!(
            "pubky://{owner}/priv/pubky.app/marketplace/v1/watchlist.json"
        ));
        uris.push(format!(
            "pubky://{owner}/pub/other.app/marketplace/v1/shop.json"
        ));
        uris.push(format!(
            "pubky://{owner}//pub/pubky.app/marketplace/v1/shop.json"
        ));
        uris.push(format!(
            "https://{owner}/pub/pubky.app/marketplace/v1/shop.json"
        ));
        uris.push(format!(
            "pubky://{owner}/pub/pubky.app/marketplace/v1/listings/0032SSN7Q4EVG?x=1"
        ));
        uris.push(format!(
            "pubky://{owner}/pub/pubky.app/marketplace/v1/listings/0032SSN7Q4EVG#x"
        ));
    }
    uris.push("pubky:///pub/pubky.app/marketplace/v1/shop.json".to_string());
    uris.push("not a url".to_string());
    uris
}

const MARKET_RESOURCES: [&str; 7] = [
    "Shop",
    "Listing",
    "Drop",
    "MarketplaceReview",
    "ReviewResponse",
    "AttestorProfile",
    "AttestorAnnotation",
];

/// The fork's parse result, with every non-Market resource mapped to what
/// this crate must return for it (`Unknown`).
fn fork_uri_outcome(uri: &str) -> (String, bool) {
    match fork::ParsedUri::try_from(uri) {
        Ok(parsed) => {
            let resource = serde_json::to_value(&parsed.resource).unwrap();
            let name = match &resource {
                Value::String(s) => s.clone(),
                Value::Object(map) => map.keys().next().cloned().unwrap_or_default(),
                _ => String::new(),
            };
            if MARKET_RESOURCES.contains(&name.as_str()) {
                let display = parsed.resource.to_string();
                let id = parsed.resource.id();
                (
                    format!("ok:{} {} {display} {id:?}", parsed.user_id, resource),
                    true,
                )
            } else {
                (
                    format!("ok:{} \"Unknown\" unknown None", parsed.user_id),
                    false,
                )
            }
        }
        Err(_) => ("err".to_string(), false),
    }
}

fn ours_uri_outcome(uri: &str) -> String {
    match ours::ParsedUri::try_from(uri) {
        Ok(parsed) => format!(
            "ok:{} {} {} {:?}",
            parsed.user_id,
            serde_json::to_value(&parsed.resource).unwrap(),
            parsed.resource,
            parsed.resource.id()
        ),
        Err(_) => "err".to_string(),
    }
}

fn is_social_path(uri: &str) -> bool {
    !uri.contains("/pub/pubky.app/marketplace/")
}

fn check_uris(report: &mut Report) -> Vec<Value> {
    let mut vectors = Vec::new();
    let mut market = 0;
    let mut social_errors = 0;
    let uris = uri_cases();
    for uri in &uris {
        let (fork, is_market) = fork_uri_outcome(uri);
        let ours = ours_uri_outcome(uri);
        market += usize::from(is_market);
        // A social path whose id the fork rejects (a follow or mute of an
        // invalid key) is an error there; here it is not a Market path.
        let allowed = fork == ours
            || (fork == "err"
                && is_social_path(uri)
                && ours.ends_with(" \"Unknown\" unknown None"));
        if fork == "err" && ours != "err" {
            social_errors += 1;
        }
        if !allowed {
            report.mismatch(format!("uri {uri}: fork {fork} | ours {ours}"));
        }
        if !(fork == "err" && ours != "err") {
            vectors.push(json!({ "uri": uri, "expected": fork }));
        }
        let round_trip = ours::ParsedUri::try_from(uri.as_str())
            .ok()
            .and_then(|p| p.try_to_uri_str().ok());
        let fork_round_trip = if is_market {
            fork::ParsedUri::try_from(uri.as_str())
                .ok()
                .and_then(|p| p.try_to_uri_str().ok())
        } else {
            None
        };
        if is_market && round_trip != fork_round_trip {
            report.mismatch(format!(
                "uri round trip {uri}: fork {fork_round_trip:?} | ours {round_trip:?}"
            ));
        }
    }
    report.section(
        "uris",
        uris.len(),
        &format!(
            " ({market} are Market resources; {social_errors} social paths the fork rejects parse as Unknown here)"
        ),
    );
    vectors
}

fn pubky_id_cases() -> Vec<String> {
    let mut out = Vec::new();
    for i in 0u32..3000 {
        let bytes = *blake3::hash(&i.to_be_bytes()).as_bytes();
        out.push(base32::encode(base32::Alphabet::Z, &bytes));
    }
    const ALPHABET: &str = "ybndrfg8ejkmcpqxot1uwisza345h769";
    for key in OWNERS
        .iter()
        .map(|k| k.to_string())
        .chain(out[..20].to_vec())
    {
        for c in ALPHABET.chars() {
            let mut variant: String = key.chars().take(51).collect();
            variant.push(c);
            out.push(variant);
        }
        out.push(key.to_uppercase());
        out.push(key[..51].to_string());
        out.push(format!("{key}y"));
        out.push(format!("pk:{key}"));
        out.push(format!(" {}", &key[1..]));
    }
    out.push(String::new());
    out
}

fn check_pubky_ids(report: &mut Report) -> Vec<Value> {
    let cases = pubky_id_cases();
    let mut accepted = 0;
    let mut vectors = Vec::new();
    for (i, s) in cases.iter().enumerate() {
        let fork = outcome(fork::PubkyId::try_from(s).map(|p| p.to_string()));
        let ours = outcome(ours::PubkyId::try_from(s).map(|p| p.to_string()));
        accepted += usize::from(fork.starts_with("ok:"));
        if fork != ours {
            report.mismatch(format!("pubky id {s:?}: fork {fork} | ours {ours}"));
        }
        if i % 25 == 0 || i >= 3000 {
            vectors.push(json!({ "input": s, "expected": fork }));
        }
    }
    report.section("pubky ids", cases.len(), &format!(" ({accepted} accepted)"));
    vectors
}

fn check_price(report: &mut Report) -> Vec<Value> {
    let currencies = [
        "USD", "EUR", "GBP", "CHF", "CAD", "AUD", "JPY", "BTC", "SAT", "XYZ", "usd", "", "US",
    ];
    let exponents = [0_i64, 1, 2, 3, 8, 10, 18, 19, -1];
    let amounts = [
        0_i64,
        1,
        9,
        10,
        99,
        100,
        12_345,
        25_000,
        9_007_199_254_740_991,
        -1,
        i64::MAX,
        i64::MIN,
    ];
    let mut vectors = Vec::new();
    let mut compared = 0;
    let mut decades: Vec<Option<String>> = vec![None];
    for currency in currencies {
        for exponent in exponents {
            for amount in amounts {
                compared += 1;
                let fork = fork::price_decade(currency, exponent, amount);
                let ours = ours::price_decade(currency, exponent, amount);
                if fork != ours {
                    report.mismatch(format!(
                        "price_decade({currency:?}, {exponent}, {amount}): fork {fork:?} | ours {ours:?}"
                    ));
                }
                vectors.push(json!({
                    "currency": currency, "exponent": exponent, "unitPriceMinor": amount, "expected": fork
                }));
                decades.push(fork);
            }
        }
    }
    for malformed in [
        "", "USD", "USD:", "USD:x", "USD:-1", "usd:2", "BTC:99", "XYZ:3", "SAT:-1", "USD:1:2",
        " USD:2", "USD: 2", "USD:+2", "USD:02", "JPY:0",
    ] {
        decades.push(Some(malformed.to_string()));
    }
    decades.sort();
    decades.dedup();
    let mut weights = Vec::new();
    for decade in &decades {
        compared += 1;
        let fork = outcome(fork::price_weight(decade.as_deref()));
        let ours = outcome(ours::price_weight(decade.as_deref()));
        if fork != ours {
            report.mismatch(format!(
                "price_weight({decade:?}): fork {fork} | ours {ours}"
            ));
        }
        weights.push(json!({ "priceDecade": decade, "expected": fork }));
    }
    report.section("price decades and weights", compared, "");
    vectors.extend(weights);
    vectors
}

/// Listing ids are microsecond timestamps taken when the record is built:
/// both crates must encode the current time, in the same format.
fn check_listing_ids(seeds: &[Seed], report: &mut Report) {
    use fork::traits::TimestampId as ForkTimestampId;
    use ours::traits::TimestampId as OursTimestampId;
    let now = || {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as i64
    };
    let decode = |id: &str| ours::validate_crockford_id(id).map(i64::from_be_bytes);
    let mut compared = 0;
    for seed in seeds.iter().filter(|s| s.kind == Kind::Listing) {
        let fork_listing: fork::PubkyAppListing =
            serde_json::from_value(seed.record.clone()).unwrap();
        let ours_listing: ours::PubkyMarketListing =
            serde_json::from_value(seed.record.clone()).unwrap();
        for _ in 0..100 {
            compared += 1;
            let before = now();
            let fork_id = fork_listing.create_id();
            let ours_id = ours_listing.create_id();
            let after = now();
            let (Ok(fork_micros), Ok(ours_micros)) = (decode(&fork_id), decode(&ours_id)) else {
                report.mismatch(format!(
                    "listing id: undecodable fork {fork_id} | ours {ours_id}"
                ));
                continue;
            };
            let fork_decoded_by_fork =
                fork::validate_crockford_id(&fork_id).map(i64::from_be_bytes);
            let in_window = |micros: i64| (before..=after).contains(&micros);
            if !in_window(fork_micros)
                || !in_window(ours_micros)
                || fork_decoded_by_fork != Ok(fork_micros)
                || ours_id.len() != fork_id.len()
                || ForkTimestampId::validate_id(&fork_listing, &ours_id).is_err()
                || OursTimestampId::validate_id(&ours_listing, &fork_id).is_err()
            {
                report.mismatch(format!(
                    "listing id: window {before}..{after}, fork {fork_id} ({fork_micros}), ours {ours_id} ({ours_micros})"
                ));
            }
        }
    }
    report.section("generated listing ids", compared, "");
}

fn check_constants(report: &mut Report) {
    use fork::traits::{HasIdPath as ForkIdPath, HasPath as ForkPath};
    use ours::traits::{HasIdPath as OursIdPath, HasPath as OursPath};
    let pairs: Vec<(&str, String, String)> = vec![
        (
            "PUBLIC_PATH",
            fork::PUBLIC_PATH.into(),
            ours::PUBLIC_PATH.into(),
        ),
        (
            "PRIVATE_PATH",
            fork::PRIVATE_PATH.into(),
            ours::PRIVATE_PATH.into(),
        ),
        ("APP_PATH", fork::APP_PATH.into(), ours::APP_PATH.into()),
        ("PROTOCOL", fork::PROTOCOL.into(), ours::PROTOCOL.into()),
        (
            "MARKETPLACE_PATH",
            fork::MARKETPLACE_PATH.into(),
            ours::MARKETPLACE_PATH.into(),
        ),
        (
            "MARKETPLACE_SCHEMA_VERSION",
            fork::MARKETPLACE_SCHEMA_VERSION.to_string(),
            ours::MARKETPLACE_SCHEMA_VERSION.to_string(),
        ),
        (
            "PURCHASE_ATTESTATION_TYP",
            fork::PURCHASE_ATTESTATION_TYP.into(),
            ours::PURCHASE_ATTESTATION_TYP.into(),
        ),
        (
            "PURCHASE_ATTESTATION_VERSION",
            fork::PURCHASE_ATTESTATION_VERSION.to_string(),
            ours::PURCHASE_ATTESTATION_VERSION.to_string(),
        ),
        (
            "ORDER_RECEIPT_ATTESTATION_TYP",
            fork::ORDER_RECEIPT_ATTESTATION_TYP.into(),
            ours::ORDER_RECEIPT_ATTESTATION_TYP.into(),
        ),
        (
            "ORDER_RECEIPT_ATTESTATION_V2_TYP",
            fork::ORDER_RECEIPT_ATTESTATION_V2_TYP.into(),
            ours::ORDER_RECEIPT_ATTESTATION_V2_TYP.into(),
        ),
        (
            "ORDER_RECEIPT_ATTESTATION_VERSION",
            fork::ORDER_RECEIPT_ATTESTATION_VERSION.to_string(),
            ours::ORDER_RECEIPT_ATTESTATION_VERSION.to_string(),
        ),
        (
            "ORDER_RECEIPT_ATTESTATION_V2_VERSION",
            fork::ORDER_RECEIPT_ATTESTATION_V2_VERSION.to_string(),
            ours::ORDER_RECEIPT_ATTESTATION_V2_VERSION.to_string(),
        ),
        (
            "DROP_EDITION_ATTESTATION_TYP",
            fork::DROP_EDITION_ATTESTATION_TYP.into(),
            ours::DROP_EDITION_ATTESTATION_TYP.into(),
        ),
        (
            "DROP_EDITION_ATTESTATION_VERSION",
            fork::DROP_EDITION_ATTESTATION_VERSION.to_string(),
            ours::DROP_EDITION_ATTESTATION_VERSION.to_string(),
        ),
        (
            "ATTESTOR_PROFILE_TYP",
            fork::ATTESTOR_PROFILE_TYP.into(),
            ours::ATTESTOR_PROFILE_TYP.into(),
        ),
        (
            "ATTESTOR_ANNOTATION_TYP",
            fork::ATTESTOR_ANNOTATION_TYP.into(),
            ours::ATTESTOR_ANNOTATION_TYP.into(),
        ),
        (
            "AMOUNT_BAND_POLICY_BOTH_SIDES_CONSENT",
            fork::AMOUNT_BAND_POLICY_BOTH_SIDES_CONSENT.into(),
            ours::AMOUNT_BAND_POLICY_BOTH_SIDES_CONSENT.into(),
        ),
        (
            "PRICE_DECADE_POLICY_ALWAYS",
            fork::PRICE_DECADE_POLICY_ALWAYS.into(),
            ours::PRICE_DECADE_POLICY_ALWAYS.into(),
        ),
        (
            "MAX_WATCHLIST_ITEMS",
            fork::MAX_WATCHLIST_ITEMS.to_string(),
            ours::MAX_WATCHLIST_ITEMS.to_string(),
        ),
        (
            "MAX_WATCHLIST_TOMBSTONES",
            fork::MAX_WATCHLIST_TOMBSTONES.to_string(),
            ours::MAX_WATCHLIST_TOMBSTONES.to_string(),
        ),
        (
            "MIN_PRICE_WEIGHT",
            fork::MIN_PRICE_WEIGHT.to_string(),
            ours::MIN_PRICE_WEIGHT.to_string(),
        ),
        (
            "MAX_PRICE_WEIGHT",
            fork::MAX_PRICE_WEIGHT.to_string(),
            ours::MAX_PRICE_WEIGHT.to_string(),
        ),
        (
            "PRICE_WEIGHT_CURRENCIES",
            format!(
                "{:?}",
                fork::PRICE_WEIGHT_CURRENCIES
                    .iter()
                    .map(|c| (c.code, c.exponent, c.decade_offset))
                    .collect::<Vec<_>>()
            ),
            format!(
                "{:?}",
                ours::PRICE_WEIGHT_CURRENCIES
                    .iter()
                    .map(|c| (c.code, c.exponent, c.decade_offset))
                    .collect::<Vec<_>>()
            ),
        ),
        (
            "shop path",
            <fork::PubkyAppShop as ForkPath>::create_path(),
            <ours::PubkyMarketShop as OursPath>::create_path(),
        ),
        (
            "watchlist path",
            <fork::PubkyAppWatchlist as ForkPath>::create_path(),
            <ours::PubkyMarketWatchlist as OursPath>::create_path(),
        ),
        (
            "attestor profile path",
            <fork::PubkyAppAttestorProfile as ForkPath>::create_path(),
            <ours::PubkyMarketAttestorProfile as OursPath>::create_path(),
        ),
        (
            "listing path",
            <fork::PubkyAppListing as ForkIdPath>::create_path("ID"),
            <ours::PubkyMarketListing as OursIdPath>::create_path("ID"),
        ),
        (
            "drop path",
            <fork::PubkyAppMarketplaceDrop as ForkIdPath>::create_path("ID"),
            <ours::PubkyMarketDrop as OursIdPath>::create_path("ID"),
        ),
        (
            "review path",
            <fork::PubkyAppMarketplaceReview as ForkIdPath>::create_path("ID"),
            <ours::PubkyMarketReview as OursIdPath>::create_path("ID"),
        ),
        (
            "review response path",
            <fork::PubkyAppReviewResponse as ForkIdPath>::create_path("ID"),
            <ours::PubkyMarketReviewResponse as OursIdPath>::create_path("ID"),
        ),
        (
            "order receipt path",
            <fork::PubkyAppMarketplaceOrderReceipt as ForkIdPath>::create_path("ID"),
            <ours::PubkyMarketOrderReceipt as OursIdPath>::create_path("ID"),
        ),
        (
            "attestor annotation path",
            <fork::PubkyAppAttestorAnnotation as ForkIdPath>::create_path("ID"),
            <ours::PubkyMarketAttestorAnnotation as OursIdPath>::create_path("ID"),
        ),
    ];
    for (name, fork, ours) in &pairs {
        if fork != ours {
            report.mismatch(format!("constant {name}: fork {fork} | ours {ours}"));
        }
    }
    report.section("constants and paths", pairs.len(), "");
}

fn vectors_document(
    seeds: &[Seed],
    cases: &[Case],
    uris: Vec<Value>,
    ids: Vec<Value>,
    price: Vec<Value>,
) -> String {
    let mut records = Vec::new();
    for seed in seeds {
        let seed_cases: Vec<&Case> = cases.iter().filter(|c| c.seed == seed.name).collect();
        for (index, case) in seed_cases.iter().enumerate() {
            if index != 0 && index % VECTOR_SAMPLE_EVERY != 0 && case.forged.is_none() {
                continue;
            }
            let uri = case_uri(case);
            let expected: serde_json::Map<String, Value> = fork_side::observe(
                case.kind,
                &uri,
                case.id.as_deref().unwrap_or(""),
                &case.blob,
            )
            .into_iter()
            .map(|(k, v)| (k.to_string(), json!(v)))
            .collect();
            records.push(json!({
                "seed": case.seed,
                "case": case.label,
                "kind": case.kind_name,
                "owner": case.owner,
                "id": case.id,
                "uri": uri,
                "input": String::from_utf8(case.blob.clone()).unwrap(),
                "forgedObservation": case.forged,
                "expected": expected,
            }));
        }
    }
    let document = json!({
        "description": "Pubky Market record vectors. Each `expected` value is what pubky-app-specs returns for the input: `ok:` followed by the exact serde_json output, or `err`. `validated` is Validatable::try_from(input, id) (attestor records: from_verified_json(uri, input)); `raw` is plain deserialization; `object` is the record read through the object dispatch from `uri`; the other keys are the attestation, ID and counting-key outcomes. Inputs are UTF-8 JSON text.",
        "source": { "crate": "pubky-app-specs", "repository": "https://github.com/BitcoinErrorLog/pubky-app-specs", "tag": FORK_TAG, "rev": FORK_REV },
        "records": records,
        "uris": uris,
        "pubkyIds": ids,
        "price": price,
    });
    let mut text = serde_json::to_string_pretty(&document).unwrap();
    text.push('\n');
    text
}

fn corpus(cases: &[Case]) -> String {
    let items: Vec<Value> = cases
        .iter()
        .map(|case| {
            json!({
                "seed": case.seed,
                "case": case.label,
                "kind": case.kind_name,
                "owner": case.owner,
                "id": case.id,
                "uri": case_uri(case),
                "input": String::from_utf8(case.blob.clone()).unwrap(),
                "forgedObservation": case.forged,
            })
        })
        .collect();
    serde_json::to_string(&json!({ "cases": items, "uris": uri_cases() })).unwrap()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("check");
    let seeds = load_seeds();
    let cases = build_cases(&seeds);
    let mut report = Report::default();

    match command {
        "corpus" => {
            let path = args.get(1).expect("corpus <output file>");
            std::fs::write(path, corpus(&cases)).unwrap();
            println!("wrote {} cases to {path}", cases.len());
        }
        "check" | "write-vectors" => {
            check_records(&cases, &mut report);
            let uris = check_uris(&mut report);
            let ids = check_pubky_ids(&mut report);
            let price = check_price(&mut report);
            check_listing_ids(&seeds, &mut report);
            check_constants(&mut report);
            let document = vectors_document(&seeds, &cases, uris, ids, price);
            let path = repo_root().join(VECTORS_FILE);
            if command == "write-vectors" {
                std::fs::write(&path, &document).unwrap();
                report.lines.push(format!("wrote {VECTORS_FILE}"));
            } else if std::fs::read_to_string(&path).ok().as_deref() != Some(document.as_str()) {
                report.mismatch(format!(
                    "{VECTORS_FILE} is not what pubky-app-specs produces now; run `cargo run --release -- write-vectors`"
                ));
            } else {
                report.lines.push(format!("{VECTORS_FILE} is current"));
            }
            println!("pubky-market-specs parity against pubky-app-specs {FORK_TAG} ({FORK_REV})");
            for line in &report.lines {
                println!("  {line}");
            }
            if report.mismatches.is_empty() {
                println!("parity: no differences");
            } else {
                println!(
                    "parity: {} differences (first 200 shown)",
                    report.mismatches.len()
                );
                for mismatch in &report.mismatches {
                    println!("  {mismatch}");
                }
                std::process::exit(1);
            }
        }
        other => {
            eprintln!("unknown command {other}: use check, write-vectors or corpus <file>");
            std::process::exit(2);
        }
    }
}
