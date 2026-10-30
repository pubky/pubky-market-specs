use crate::{traits::Validatable, APP_PATH, MARKETPLACE_PATH, PROTOCOL, PUBLIC_PATH};
use serde::{Deserialize, Serialize};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

/// Version literal shared by every marketplace record (`schemaVersion`).
pub const MARKETPLACE_SCHEMA_VERSION: i64 = 1;
/// Largest integer safely representable by JS clients (`Number.MAX_SAFE_INTEGER`).
pub const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

// Shared validation limits (mirroring the pubky-app commerce config)
const MIN_CURRENCY_LENGTH: usize = 3;
const MAX_CURRENCY_LENGTH: usize = 12;
const MAX_MONEY_EXPONENT: i64 = 18;
const MIN_ENTITY_ID_LENGTH: usize = 1;
const MAX_ENTITY_ID_LENGTH: usize = 128;
const MAX_REGION_LENGTH: usize = 100;
const PUBKY_LENGTH: usize = 52;

/// z-base-32 alphabet used by pubky identifiers.
const Z_BASE_32_ALPHABET: &str = "ybndrfg8ejkmcpqxot1uwisza345h769";

/// Integer money in minor units, e.g. cents or satoshis.
///
/// `amount_minor` is always an integer amount of the smallest unit,
/// `currency` is an uppercase asset code and `exponent` the number of
/// decimal places between minor and major units.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketMoney {
    pub amount_minor: i64,
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub currency: String,
    pub exponent: i64,
}

impl PubkyMarketMoney {
    /// Validates a non-negative amount (`amountMinor >= 0`).
    pub fn validate_non_negative(&self, field: &str) -> Result<(), String> {
        if !(0..=MAX_SAFE_INTEGER).contains(&self.amount_minor) {
            return Err(format!(
                "Validation Error: {field} amountMinor must be a non-negative safe integer"
            ));
        }
        self.validate_asset(field)
    }

    /// Validates a strictly positive amount (`amountMinor > 0`).
    pub fn validate_positive(&self, field: &str) -> Result<(), String> {
        if !(1..=MAX_SAFE_INTEGER).contains(&self.amount_minor) {
            return Err(format!(
                "Validation Error: {field} amountMinor must be a positive safe integer"
            ));
        }
        self.validate_asset(field)
    }

    /// Returns true when both amounts use the same currency code and exponent.
    pub fn same_asset(&self, other: &PubkyMarketMoney) -> bool {
        self.currency == other.currency && self.exponent == other.exponent
    }

    fn validate_asset(&self, field: &str) -> Result<(), String> {
        let length = self.currency.chars().count();
        if !(MIN_CURRENCY_LENGTH..=MAX_CURRENCY_LENGTH).contains(&length) {
            return Err(format!(
                "Validation Error: {field} currency must be {MIN_CURRENCY_LENGTH}-{MAX_CURRENCY_LENGTH} characters"
            ));
        }
        let mut chars = self.currency.chars();
        let first_is_upper = chars.next().is_some_and(|c| c.is_ascii_uppercase());
        if !first_is_upper || !chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
            return Err(format!(
                "Validation Error: {field} currency must be an uppercase asset code"
            ));
        }
        if !(0..=MAX_MONEY_EXPONENT).contains(&self.exponent) {
            return Err(format!(
                "Validation Error: {field} exponent must be between 0 and {MAX_MONEY_EXPONENT}"
            ));
        }
        Ok(())
    }
}

/// Coarse public location attached to shops and listings.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketLocation {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub country_code: String,
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
}

impl PubkyMarketLocation {
    pub(crate) fn sanitize(self) -> Self {
        PubkyMarketLocation {
            country_code: self.country_code,
            region: self.region.map(|region| region.trim().to_string()),
        }
    }
}

impl Validatable for PubkyMarketLocation {
    fn validate(&self, _id: Option<&str>) -> Result<(), String> {
        if self.country_code.chars().count() != 2
            || !self.country_code.chars().all(|c| c.is_ascii_uppercase())
        {
            return Err(
                "Validation Error: location countryCode must be an ISO 3166-1 alpha-2 code".into(),
            );
        }
        if let Some(region) = &self.region {
            let length = region.chars().count();
            if !(1..=MAX_REGION_LENGTH).contains(&length) {
                return Err(format!(
                    "Validation Error: location region must be 1-{MAX_REGION_LENGTH} characters"
                ));
            }
        }
        Ok(())
    }
}

/// Validates a path-safe commerce identifier: 1-128 chars of `[A-Za-z0-9_-]`.
pub(crate) fn validate_entity_id(value: &str, field: &str) -> Result<(), String> {
    let length = value.chars().count();
    if !(MIN_ENTITY_ID_LENGTH..=MAX_ENTITY_ID_LENGTH).contains(&length) {
        return Err(format!(
            "Validation Error: {field} must be {MIN_ENTITY_ID_LENGTH}-{MAX_ENTITY_ID_LENGTH} characters"
        ));
    }
    if !value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(format!(
            "Validation Error: {field} must only contain path-safe characters [A-Za-z0-9_-]"
        ));
    }
    Ok(())
}

/// Validates a 52-character z-base-32 pubky identifier.
pub(crate) fn validate_pubky(value: &str, field: &str) -> Result<(), String> {
    if value.chars().count() != PUBKY_LENGTH
        || !value.chars().all(|c| Z_BASE_32_ALPHABET.contains(c))
    {
        return Err(format!(
            "Validation Error: {field} must be a 52-character z-base-32 pubky"
        ));
    }
    Ok(())
}

/// Validates a lowercase hyphenated UUID (`8-4-4-4-12` lowercase hex),
/// the identifier form used by the marketplace transaction service.
pub(crate) fn validate_uuid(value: &str, field: &str) -> Result<(), String> {
    let error =
        format!("Validation Error: {field} must be a lowercase hyphenated UUID (8-4-4-4-12)");
    let bytes = value.as_bytes();
    if bytes.len() != 36 {
        return Err(error);
    }
    for (index, byte) in bytes.iter().enumerate() {
        match index {
            8 | 13 | 18 | 23 => {
                if *byte != b'-' {
                    return Err(error);
                }
            }
            _ => {
                if !byte.is_ascii_digit() && !(b'a'..=b'f').contains(byte) {
                    return Err(error);
                }
            }
        }
    }
    Ok(())
}

/// Validates a lowercase 64-character hexadecimal BLAKE3 hash.
pub(crate) fn validate_hex_hash(value: &str, field: &str) -> Result<(), String> {
    if value.chars().count() != 64
        || !value
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
    {
        return Err(format!(
            "Validation Error: {field} must be a lowercase 64-character BLAKE3 hex hash"
        ));
    }
    Ok(())
}

fn is_uri_path_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '/' || c == '-'
}

/// Validates a `pubky://<pubky>/pub/pubky.app/marketplace/v1/...` URI.
pub(crate) fn validate_marketplace_uri(value: &str, field: &str) -> Result<(), String> {
    let error = format!("Validation Error: {field} must be a Pubky marketplace v1 URI");
    let rest = value.strip_prefix(PROTOCOL).ok_or_else(|| error.clone())?;
    let slash_index = rest.find('/').ok_or_else(|| error.clone())?;
    let (host, path) = rest.split_at(slash_index);
    validate_pubky(host, field).map_err(|_| error.clone())?;
    let expected_prefix = [PUBLIC_PATH, APP_PATH, MARKETPLACE_PATH].concat();
    let tail = path
        .strip_prefix(expected_prefix.as_str())
        .ok_or_else(|| error.clone())?;
    if tail.is_empty() || !tail.chars().all(is_uri_path_char) {
        return Err(error);
    }
    Ok(())
}

/// Validates a `pubky://<pubky>/pub/locks.app/....json` URI.
pub(crate) fn validate_locks_uri(value: &str, field: &str) -> Result<(), String> {
    let error = format!("Validation Error: {field} must be a public Locks policy URI");
    let rest = value.strip_prefix(PROTOCOL).ok_or_else(|| error.clone())?;
    let slash_index = rest.find('/').ok_or_else(|| error.clone())?;
    let (host, path) = rest.split_at(slash_index);
    validate_pubky(host, field).map_err(|_| error.clone())?;
    let tail = path
        .strip_prefix("/pub/locks.app/")
        .ok_or_else(|| error.clone())?;
    let stem = tail.strip_suffix(".json").ok_or_else(|| error.clone())?;
    if stem.is_empty() || !tail.chars().all(is_uri_path_char) {
        return Err(error);
    }
    Ok(())
}

/// Returns the required prefix for marketplace media owned by `owner_pubky`.
pub(crate) fn marketplace_media_prefix(owner_pubky: &str) -> String {
    [
        PROTOCOL,
        owner_pubky,
        PUBLIC_PATH,
        APP_PATH,
        MARKETPLACE_PATH,
        "media/",
    ]
    .concat()
}

/// Validates the fields shared by every marketplace record.
///
/// Forward-compat contract (aligned with the social/v1 rule): marketplace
/// record shapes are OPEN-WORLD — unknown members are tolerated on parse so
/// records can grow additively without breaking older readers. The one
/// deliberate exception is the JWS attestation header/claim structs, which
/// stay closed-world as a verification-boundary choice: an attestation is an
/// opaque signed string, not an evolving record.
pub(crate) fn validate_base_record(
    schema_version: i64,
    record_type: &str,
    expected_record_type: &str,
    owner_pubky: &str,
    revision: i64,
    created_at: &str,
    updated_at: &str,
) -> Result<(), String> {
    if schema_version != MARKETPLACE_SCHEMA_VERSION {
        return Err(format!(
            "Validation Error: schemaVersion must be {MARKETPLACE_SCHEMA_VERSION}"
        ));
    }
    if record_type != expected_record_type {
        return Err(format!(
            "Validation Error: recordType must be '{expected_record_type}'"
        ));
    }
    validate_pubky(owner_pubky, "ownerPubky")?;
    if !(1..=MAX_SAFE_INTEGER).contains(&revision) {
        return Err("Validation Error: revision must be a positive safe integer".into());
    }
    let created =
        parse_rfc3339_millis(created_at).map_err(|e| format!("Validation Error: createdAt {e}"))?;
    let updated =
        parse_rfc3339_millis(updated_at).map_err(|e| format!("Validation Error: updatedAt {e}"))?;
    if updated < created {
        return Err("Validation Error: updatedAt must not precede createdAt".into());
    }
    Ok(())
}

/// Parses an RFC 3339 / ISO-8601 datetime with an explicit offset
/// (`Z` or `±HH:MM`) into milliseconds since the UNIX epoch.
pub(crate) fn parse_rfc3339_millis(value: &str) -> Result<i64, String> {
    let bytes = value.as_bytes();
    let error = "must be an ISO-8601 datetime with offset".to_string();

    let digits = |range: std::ops::Range<usize>| -> Result<i64, String> {
        let slice = bytes.get(range).ok_or_else(|| error.clone())?;
        if !slice.iter().all(u8::is_ascii_digit) {
            return Err(error.clone());
        }
        Ok(slice
            .iter()
            .fold(0i64, |acc, b| acc * 10 + i64::from(b - b'0')))
    };

    // Fixed layout: YYYY-MM-DDTHH:MM:SS
    if bytes.len() < 20
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return Err(error);
    }
    let year = digits(0..4)?;
    let month = digits(5..7)?;
    let day = digits(8..10)?;
    let hour = digits(11..13)?;
    let minute = digits(14..16)?;
    let second = digits(17..19)?;

    if !(1..=12).contains(&month) || !(1..=days_in_month(year, month)).contains(&day) {
        return Err(error);
    }
    if hour > 23 || minute > 59 || second > 59 {
        return Err(error);
    }

    // Optional fractional seconds
    let mut cursor = 19;
    let mut millis_fraction = 0i64;
    if bytes.get(cursor) == Some(&b'.') {
        let fraction_start = cursor + 1;
        let mut fraction_end = fraction_start;
        while bytes.get(fraction_end).is_some_and(u8::is_ascii_digit) {
            fraction_end += 1;
        }
        if fraction_end == fraction_start || fraction_end - fraction_start > 9 {
            return Err(error);
        }
        for (index, byte) in bytes[fraction_start..fraction_end.min(fraction_start + 3)]
            .iter()
            .enumerate()
        {
            millis_fraction += i64::from(byte - b'0') * 10i64.pow(2 - index as u32);
        }
        cursor = fraction_end;
    }

    // Required offset: 'Z' or ±HH:MM
    let offset_seconds = match bytes.get(cursor) {
        Some(b'Z') if cursor + 1 == bytes.len() => 0,
        Some(sign @ (b'+' | b'-')) => {
            if cursor + 6 != bytes.len() || bytes[cursor + 3] != b':' {
                return Err(error);
            }
            let offset_hour = digits(cursor + 1..cursor + 3)?;
            let offset_minute = digits(cursor + 4..cursor + 6)?;
            if offset_hour > 23 || offset_minute > 59 {
                return Err(error);
            }
            let seconds = offset_hour * 3600 + offset_minute * 60;
            if *sign == b'+' {
                seconds
            } else {
                -seconds
            }
        }
        _ => return Err(error),
    };

    let days = days_from_civil(year, month, day);
    let epoch_seconds = days * 86_400 + hour * 3_600 + minute * 60 + second - offset_seconds;
    Ok(epoch_seconds * 1_000 + millis_fraction)
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        _ => 28,
    }
}

/// Days since the UNIX epoch for a civil date (Howard Hinnant's algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let adjusted_year = if month <= 2 { year - 1 } else { year };
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let adjusted_month = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * adjusted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rfc3339_epoch() {
        assert_eq!(parse_rfc3339_millis("1970-01-01T00:00:00Z").unwrap(), 0);
    }

    #[test]
    fn test_parse_rfc3339_known_timestamp() {
        // 2024-10-01T00:00:00Z == 1727740800000 ms
        assert_eq!(
            parse_rfc3339_millis("2024-10-01T00:00:00Z").unwrap(),
            1_727_740_800_000
        );
    }

    #[test]
    fn test_parse_rfc3339_with_offset() {
        // +02:00 is two hours earlier in UTC
        assert_eq!(
            parse_rfc3339_millis("2024-10-01T02:00:00+02:00").unwrap(),
            1_727_740_800_000
        );
        assert_eq!(
            parse_rfc3339_millis("2024-09-30T22:00:00-02:00").unwrap(),
            1_727_740_800_000
        );
    }

    #[test]
    fn test_parse_rfc3339_with_fraction() {
        assert_eq!(
            parse_rfc3339_millis("1970-01-01T00:00:00.123Z").unwrap(),
            123
        );
        assert_eq!(
            parse_rfc3339_millis("1970-01-01T00:00:00.123456Z").unwrap(),
            123
        );
    }

    #[test]
    fn test_parse_rfc3339_leap_year() {
        assert!(parse_rfc3339_millis("2024-02-29T00:00:00Z").is_ok());
        assert!(parse_rfc3339_millis("2023-02-29T00:00:00Z").is_err());
    }

    #[test]
    fn test_parse_rfc3339_rejects_invalid() {
        assert!(parse_rfc3339_millis("2024-13-01T00:00:00Z").is_err());
        assert!(parse_rfc3339_millis("2024-01-32T00:00:00Z").is_err());
        assert!(parse_rfc3339_millis("2024-01-01T24:00:00Z").is_err());
        assert!(parse_rfc3339_millis("2024-01-01T00:00:00").is_err()); // missing offset
        assert!(parse_rfc3339_millis("2024-01-01 00:00:00Z").is_err()); // missing 'T'
        assert!(parse_rfc3339_millis("not a date").is_err());
        assert!(parse_rfc3339_millis("2024-01-01T00:00:00+0200").is_err()); // offset needs colon
    }

    #[test]
    fn test_money_validation() {
        let money = PubkyMarketMoney {
            amount_minor: 1000,
            currency: "USD".to_string(),
            exponent: 2,
        };
        assert!(money.validate_positive("price").is_ok());
        assert!(money.validate_non_negative("price").is_ok());

        let zero = PubkyMarketMoney {
            amount_minor: 0,
            ..money.clone()
        };
        assert!(zero.validate_non_negative("price").is_ok());
        assert!(zero.validate_positive("price").is_err());

        let negative = PubkyMarketMoney {
            amount_minor: -1,
            ..money.clone()
        };
        assert!(negative.validate_non_negative("price").is_err());

        let bad_currency = PubkyMarketMoney {
            currency: "usd".to_string(),
            ..money.clone()
        };
        assert!(bad_currency.validate_positive("price").is_err());

        let short_currency = PubkyMarketMoney {
            currency: "US".to_string(),
            ..money.clone()
        };
        assert!(short_currency.validate_positive("price").is_err());

        let bad_exponent = PubkyMarketMoney {
            exponent: 19,
            ..money.clone()
        };
        assert!(bad_exponent.validate_positive("price").is_err());

        let sats = PubkyMarketMoney {
            amount_minor: 21,
            currency: "BTC".to_string(),
            exponent: 8,
        };
        assert!(!money.same_asset(&sats));
        assert!(money.same_asset(&money.clone()));
    }

    #[test]
    fn test_location_validation() {
        let location = PubkyMarketLocation {
            country_code: "US".to_string(),
            region: Some("California".to_string()),
        };
        assert!(location.validate(None).is_ok());

        let bad_country = PubkyMarketLocation {
            country_code: "usa".to_string(),
            region: None,
        };
        assert!(bad_country.validate(None).is_err());

        let empty_region = PubkyMarketLocation {
            country_code: "US".to_string(),
            region: Some("   ".to_string()),
        }
        .sanitize();
        assert!(empty_region.validate(None).is_err());
    }

    #[test]
    fn test_validate_entity_id() {
        assert!(validate_entity_id("boots_01", "listingId").is_ok());
        assert!(validate_entity_id("A-Z_09", "listingId").is_ok());
        assert!(validate_entity_id("", "listingId").is_err());
        assert!(validate_entity_id("has space", "listingId").is_err());
        assert!(validate_entity_id(&"a".repeat(129), "listingId").is_err());
    }

    #[test]
    fn test_validate_uuid() {
        assert!(validate_uuid("a7fc7d5d-0b2a-4083-b278-47193f8fe536", "orderId").is_ok());
        assert!(validate_uuid("00000000-0000-0000-0000-000000000000", "orderId").is_ok());
        // Uppercase hex is rejected: the transaction service emits lowercase.
        assert!(validate_uuid("A7FC7D5D-0B2A-4083-B278-47193F8FE536", "orderId").is_err());
        // Unhyphenated form is rejected.
        assert!(validate_uuid("a7fc7d5d0b2a4083b27847193f8fe536", "orderId").is_err());
        assert!(validate_uuid("a7fc7d5d-0b2a-4083-b278-47193f8fe53", "orderId").is_err());
        assert!(validate_uuid("a7fc7d5d-0b2a-4083-b278-47193f8fe5366", "orderId").is_err());
        assert!(validate_uuid("a7fc7d5d-0b2a-4083-b278-47193f8fe53g", "orderId").is_err());
        assert!(validate_uuid("a7fc7d5d0-b2a-4083-b278-47193f8fe536", "orderId").is_err());
        assert!(validate_uuid("", "orderId").is_err());
    }

    #[test]
    fn test_validate_marketplace_uri() {
        let owner = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
        let valid = format!("pubky://{owner}/pub/pubky.app/marketplace/v1/media/image_01");
        assert!(validate_marketplace_uri(&valid, "url").is_ok());

        let wrong_app = format!("pubky://{owner}/pub/other.app/marketplace/v1/media/image_01");
        assert!(validate_marketplace_uri(&wrong_app, "url").is_err());

        let no_tail = format!("pubky://{owner}/pub/pubky.app/marketplace/v1/");
        assert!(validate_marketplace_uri(&no_tail, "url").is_err());

        let bad_host = "pubky://short/pub/pubky.app/marketplace/v1/media/image_01";
        assert!(validate_marketplace_uri(bad_host, "url").is_err());
    }

    #[test]
    fn test_validate_locks_uri() {
        let owner = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
        let valid = format!("pubky://{owner}/pub/locks.app/policies/standard.json");
        assert!(validate_locks_uri(&valid, "policyUri").is_ok());

        let not_json = format!("pubky://{owner}/pub/locks.app/policies/standard");
        assert!(validate_locks_uri(&not_json, "policyUri").is_err());

        let empty_stem = format!("pubky://{owner}/pub/locks.app/.json");
        assert!(validate_locks_uri(&empty_stem, "policyUri").is_err());
    }

    #[test]
    fn test_validate_base_record() {
        let owner = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
        assert!(validate_base_record(
            1,
            "shop",
            "shop",
            owner,
            1,
            "2025-01-01T00:00:00Z",
            "2025-01-02T00:00:00Z",
        )
        .is_ok());

        // updatedAt precedes createdAt
        assert!(validate_base_record(
            1,
            "shop",
            "shop",
            owner,
            1,
            "2025-01-02T00:00:00Z",
            "2025-01-01T00:00:00Z",
        )
        .is_err());

        // wrong schema version
        assert!(validate_base_record(
            2,
            "shop",
            "shop",
            owner,
            1,
            "2025-01-01T00:00:00Z",
            "2025-01-01T00:00:00Z",
        )
        .is_err());

        // wrong record type
        assert!(validate_base_record(
            1,
            "listing",
            "shop",
            owner,
            1,
            "2025-01-01T00:00:00Z",
            "2025-01-01T00:00:00Z",
        )
        .is_err());

        // zero revision
        assert!(validate_base_record(
            1,
            "shop",
            "shop",
            owner,
            0,
            "2025-01-01T00:00:00Z",
            "2025-01-01T00:00:00Z",
        )
        .is_err());
    }
}
