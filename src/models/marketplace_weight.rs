//! The price weight of a counted review (counting rules v2): a 1-5 weight
//! from the price decade the purchase attestation signs, used by the
//! "weighted by purchase value" rating and the "highest-value purchases
//! first" sort.
//!
//! The attestation's `price_decade` claim is `"{CURRENCY}:{k}"`, the
//! encoding of `amount_band`, where `CURRENCY` is the listing's own currency
//! code and `k = floor(log10(unit price paid))` counted in that currency's
//! table minor unit ([`PRICE_WEIGHT_CURRENCIES`]): cents for USD, yen for
//! JPY, satoshis for BTC. A price written at another exponent shifts `k` by
//! the exponent difference, so `BTC` is always decaded in sats.
//!
//! The weight is `w = clamp(k + offset(CURRENCY) - 1, 1, 5)`, where the
//! offset turns the table minor unit's decade into a US-cent decade: under
//! $10 weighs 1, $10-99 weighs 2, $100-999 weighs 3, $1,000-9,999 weighs 4
//! and $10,000 or more weighs 5. An attestation without the claim, or with a
//! currency the table lacks, weighs 1.

/// Lowest price weight; also the weight of a missing claim or an unknown
/// currency.
pub const MIN_PRICE_WEIGHT: u8 = 1;
/// Highest price weight.
pub const MAX_PRICE_WEIGHT: u8 = 5;

/// i64 minor units cap a decade at 18.
const MAX_DECADE_MAGNITUDE: u32 = 18;

/// A currency the price weight table knows: its minor unit (`exponent`
/// decimal places below the major unit) and the decade offset from that
/// minor unit to a US cent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PriceWeightCurrency {
    pub code: &'static str,
    pub exponent: i64,
    pub decade_offset: i32,
}

/// Every currency the stack prices in. The table is static and normative:
/// every indexer and recompute tool derives the same weight from the same
/// claim, so it does not follow exchange rates.
///
/// - `USD`, `EUR`, `GBP`, `CHF`, `CAD`, `AUD` (cents, exponent 2): one minor
///   unit is within a decade of one US cent, offset 0.
/// - `JPY` (yen, exponent 0): one yen is about 0.65 US cents, offset 0.
/// - `BTC` (satoshis, exponent 8) and `SAT` (satoshis, exponent 0, the
///   `amount_band` code): one satoshi is (BTC/USD) / 10^6 US cents, offset
///   -1, the nearest decade while one bitcoin trades between about $31,600
///   and $316,000.
pub const PRICE_WEIGHT_CURRENCIES: &[PriceWeightCurrency] = &[
    PriceWeightCurrency {
        code: "USD",
        exponent: 2,
        decade_offset: 0,
    },
    PriceWeightCurrency {
        code: "EUR",
        exponent: 2,
        decade_offset: 0,
    },
    PriceWeightCurrency {
        code: "GBP",
        exponent: 2,
        decade_offset: 0,
    },
    PriceWeightCurrency {
        code: "CHF",
        exponent: 2,
        decade_offset: 0,
    },
    PriceWeightCurrency {
        code: "CAD",
        exponent: 2,
        decade_offset: 0,
    },
    PriceWeightCurrency {
        code: "AUD",
        exponent: 2,
        decade_offset: 0,
    },
    PriceWeightCurrency {
        code: "JPY",
        exponent: 0,
        decade_offset: 0,
    },
    PriceWeightCurrency {
        code: "BTC",
        exponent: 8,
        decade_offset: -1,
    },
    PriceWeightCurrency {
        code: "SAT",
        exponent: 0,
        decade_offset: -1,
    },
];

/// The table entry for `currency`, or `None` when the table has no entry.
pub fn price_weight_currency(currency: &str) -> Option<&'static PriceWeightCurrency> {
    PRICE_WEIGHT_CURRENCIES
        .iter()
        .find(|entry| entry.code == currency)
}

/// The decade offset of `currency`, or `None` when the table has no entry.
pub fn price_weight_decade_offset(currency: &str) -> Option<i32> {
    price_weight_currency(currency).map(|entry| entry.decade_offset)
}

/// The `price_decade` claim value an issuer writes for a unit price of
/// `unit_price_minor` at `exponent` decimal places of `currency` (a listing
/// price's `amountMinor`, `exponent` and `currency`), e.g.
/// `("USD", 2, 4_999)` -> `"USD:3"` and `("BTC", 8, 25_000)` -> `"BTC:4"`.
///
/// For a table currency the decade counts the table's minor unit: a price at
/// another exponent shifts it by `table exponent - exponent` (exact, since
/// it scales by a power of ten), so `("BTC", 0, 1)`, one whole bitcoin, is
/// `"BTC:8"`. A currency outside the table keeps the decade of its own minor
/// units (it weighs 1 either way). Decades above 18 are written as 18.
///
/// `None` when there is no decade to sign: a non-positive price, or one
/// below a single table minor unit (such as a fraction of a satoshi). The
/// claim is then left out and the review weighs 1.
pub fn price_decade(currency: &str, exponent: i64, unit_price_minor: i64) -> Option<String> {
    if unit_price_minor < 1 {
        return None;
    }
    let shift = price_weight_currency(currency).map_or(0, |entry| entry.exponent - exponent);
    let decade = i64::from(unit_price_minor.ilog10()) + shift;
    if decade < 0 {
        return None;
    }
    let decade = decade.min(i64::from(MAX_DECADE_MAGNITUDE));
    Some(format!("{currency}:{decade}"))
}

/// The price weight of a review whose attestation carries `price_decade`
/// (`None` when the claim is absent, which weighs 1). Errors when the value
/// is not a `{CURRENCY}:{magnitude}` decade.
pub fn price_weight(price_decade: Option<&str>) -> Result<u8, String> {
    let Some(value) = price_decade else {
        return Ok(MIN_PRICE_WEIGHT);
    };
    let (currency, magnitude) = parse_log_decade(value, "price_decade")?;
    let Some(offset) = price_weight_decade_offset(currency) else {
        return Ok(MIN_PRICE_WEIGHT);
    };
    let decade = magnitude as i32 + offset;
    Ok((decade - 1).clamp(i32::from(MIN_PRICE_WEIGHT), i32::from(MAX_PRICE_WEIGHT)) as u8)
}

/// Parses a `{CURRENCY}:{magnitude}` log decade (`amount_band`,
/// `price_decade`): an uppercase 3-12 character asset code that starts with
/// a letter, and a magnitude of 0-18 written in at most two digits.
pub(crate) fn parse_log_decade<'a>(value: &'a str, field: &str) -> Result<(&'a str, u32), String> {
    let error =
        || format!("Validation Error: {field} must be {{CURRENCY}}:{{magnitude}}, e.g. SAT:5");
    let (currency, magnitude) = value.split_once(':').ok_or_else(error)?;
    let currency_length = currency.chars().count();
    if !(3..=12).contains(&currency_length) {
        return Err(error());
    }
    let mut chars = currency.chars();
    let first_is_upper = chars.next().is_some_and(|c| c.is_ascii_uppercase());
    if !first_is_upper || !chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
        return Err(error());
    }
    match magnitude.parse::<u32>() {
        Ok(m) if magnitude.len() <= 2 && m <= MAX_DECADE_MAGNITUDE => Ok((currency, m)),
        _ => Err(error()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Vector {
        currency: String,
        exponent: i64,
        unit_price_minor: i64,
        price_decade: Option<String>,
        weight: u8,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Vectors {
        #[allow(dead_code)]
        description: String,
        vectors: Vec<Vector>,
    }

    fn weight_of(currency: &str, exponent: i64, unit_price_minor: i64) -> u8 {
        price_weight(price_decade(currency, exponent, unit_price_minor).as_deref()).unwrap()
    }

    #[test]
    fn test_shared_vectors() {
        let fixture: Vectors =
            serde_json::from_str(include_str!("../test_fixtures/price-weight-vectors.json"))
                .expect("vector fixture parses");
        for currency in [
            "USD", "EUR", "GBP", "CHF", "CAD", "AUD", "JPY", "BTC", "SAT",
        ] {
            assert!(
                fixture.vectors.iter().any(|v| v.currency == currency),
                "no vector for {currency}"
            );
        }
        for vector in fixture.vectors {
            let label = format!(
                "{} {} at exponent {}",
                vector.currency, vector.unit_price_minor, vector.exponent
            );
            let decade = price_decade(&vector.currency, vector.exponent, vector.unit_price_minor);
            assert_eq!(decade, vector.price_decade, "{label}");
            assert_eq!(
                price_weight(decade.as_deref()),
                Ok(vector.weight),
                "{label}"
            );
        }
    }

    #[test]
    fn test_cent_decade_edges() {
        for currency in ["USD", "EUR", "GBP", "CHF", "CAD", "AUD"] {
            for (unit_price_minor, weight) in [
                (1, 1),
                (999, 1),       // 9.99
                (1_000, 2),     // 10.00
                (9_999, 2),     // 99.99
                (10_000, 3),    // 100.00
                (99_999, 3),    // 999.99
                (100_000, 4),   // 1,000.00
                (999_999, 4),   // 9,999.99
                (1_000_000, 5), // 10,000.00
                (i64::MAX, 5),  // clamped
            ] {
                assert_eq!(
                    weight_of(currency, 2, unit_price_minor),
                    weight,
                    "{currency} {unit_price_minor}"
                );
            }
        }
    }

    #[test]
    fn test_yen_decade_edges() {
        for (yen, weight) in [
            (1, 1),
            (999, 1),
            (1_000, 2),
            (9_999, 2),
            (10_000, 3),
            (100_000, 4),
            (1_000_000, 5),
        ] {
            assert_eq!(weight_of("JPY", 0, yen), weight, "JPY {yen}");
        }
        assert_eq!(price_decade("JPY", 0, 4_500).as_deref(), Some("JPY:3"));
    }

    #[test]
    fn test_satoshi_decade_edges() {
        for (sats, weight) in [
            (1, 1),
            (9_999, 1),
            (10_000, 2),
            (99_999, 2),
            (100_000, 3),
            (999_999, 3),
            (1_000_000, 4),
            (9_999_999, 4),
            (10_000_000, 5),
            (2_100_000_000_000_000, 5),
        ] {
            assert_eq!(weight_of("BTC", 8, sats), weight, "BTC {sats}");
            assert_eq!(weight_of("SAT", 0, sats), weight, "SAT {sats}");
        }
        assert_eq!(price_decade("BTC", 8, 25_000).as_deref(), Some("BTC:4"));
        assert_eq!(price_decade("SAT", 0, 25_000).as_deref(), Some("SAT:4"));
    }

    #[test]
    fn test_other_exponents_decade_in_the_table_unit() {
        // One whole bitcoin, written at exponent 0, is 10^8 sats.
        assert_eq!(price_decade("BTC", 0, 1).as_deref(), Some("BTC:8"));
        // 1,000 millisats (exponent 11) is one sat.
        assert_eq!(price_decade("BTC", 11, 1_000).as_deref(), Some("BTC:0"));
        // Below one sat there is no decade to sign.
        assert_eq!(price_decade("BTC", 11, 999), None);
        // $10.000 at exponent 3 is 1,000 cents.
        assert_eq!(price_decade("USD", 3, 10_000).as_deref(), Some("USD:3"));
        // 450 yen written at exponent 2 is still 450 yen.
        assert_eq!(price_decade("JPY", 2, 45_000).as_deref(), Some("JPY:2"));
        // Decades above 18 are written as 18.
        assert_eq!(price_decade("BTC", 0, i64::MAX).as_deref(), Some("BTC:18"));
        // A currency outside the table keeps its own minor-unit decade.
        assert_eq!(price_decade("XYZ", 5, 1_000).as_deref(), Some("XYZ:3"));
    }

    #[test]
    fn test_unknown_currency_and_missing_claim_weigh_one() {
        assert_eq!(price_weight(Some("XYZ:9")), Ok(MIN_PRICE_WEIGHT));
        assert_eq!(price_weight(Some("XYZ:0")), Ok(MIN_PRICE_WEIGHT));
        assert_eq!(price_weight(None), Ok(MIN_PRICE_WEIGHT));
        assert_eq!(price_weight_decade_offset("XYZ"), None);
    }

    #[test]
    fn test_malformed_price_decade_rejected() {
        for value in [
            "", "USD", "USD:", ":3", "usd:3", "US:3", "1USD:3", "USD:19", "USD:007", "USD:-1",
            "USD:3.0", "USD:3:4", "USD 3",
        ] {
            assert!(price_weight(Some(value)).is_err(), "{value:?}");
        }
    }

    #[test]
    fn test_non_positive_price_has_no_decade() {
        assert_eq!(price_decade("USD", 2, 0), None);
        assert_eq!(price_decade("USD", 2, -1_000), None);
    }

    #[test]
    fn test_currency_table() {
        let expected = [
            ("USD", 2, 0),
            ("EUR", 2, 0),
            ("GBP", 2, 0),
            ("CHF", 2, 0),
            ("CAD", 2, 0),
            ("AUD", 2, 0),
            ("JPY", 0, 0),
            ("BTC", 8, -1),
            ("SAT", 0, -1),
        ];
        assert_eq!(PRICE_WEIGHT_CURRENCIES.len(), expected.len());
        for (code, exponent, decade_offset) in expected {
            assert_eq!(
                price_weight_currency(code),
                Some(&PriceWeightCurrency {
                    code,
                    exponent,
                    decade_offset
                }),
                "{code}"
            );
        }
    }
}
