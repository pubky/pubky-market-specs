use crate::{
    models::listing::PubkyMarketShippingOption,
    models::marketplace::{
        validate_base_record, validate_entity_id, PubkyMarketMoney, MAX_SAFE_INTEGER,
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

const SHIPPING_PROFILE_RECORD_TYPE: &str = "shipping_profile";
const MIN_NAME_LENGTH: usize = 1;
const MAX_NAME_LENGTH: usize = 100;
const MAX_LABEL_LENGTH: usize = 100;
const MAX_ZONES: usize = 50;
const MAX_RATES_PER_ZONE: usize = 10;
const MAX_COUNTRIES: usize = 250;
const MAX_ORIGINS: usize = 50;
const MAX_DELIVERY_ESTIMATE_DAYS: i64 = 365;
/// The most units one shipping group may quote.
pub const MAX_SHIPPING_GROUP_UNITS: i64 = 1_000_000;

fn validate_country_code(code: &str, field: &str) -> Result<(), String> {
    if code.len() != 2 || !code.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err(format!(
            "Validation Error: {field} must be ISO 3166-1 alpha-2 country codes"
        ));
    }
    Ok(())
}

/// Validates a list of unique ISO 3166-1 alpha-2 codes of at most `max` entries.
pub(crate) fn validate_country_list(
    codes: &[String],
    field: &str,
    max: usize,
) -> Result<(), String> {
    if codes.len() > max {
        return Err(format!(
            "Validation Error: {field} lists at most {max} countries"
        ));
    }
    let mut seen = HashSet::new();
    for code in codes {
        validate_country_code(code, field)?;
        if !seen.insert(code.as_str()) {
            return Err(format!("Validation Error: {field} must be unique"));
        }
    }
    Ok(())
}

/// One way to ship to the countries of a zone. The pricing is closed-world: a
/// new pricing kind is a new spec version, so a reader never prices a rate it
/// does not understand.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "pricing", rename_all = "snake_case")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub enum PubkyMarketShippingRate {
    /// No shipping charge.
    #[serde(rename_all = "camelCase")]
    Free {
        rate_id: String,
        label: String,
        estimated_min_days: i64,
        estimated_max_days: i64,
    },
    /// One charge per shipping group, whatever the number of units.
    #[serde(rename_all = "camelCase")]
    Flat {
        rate_id: String,
        label: String,
        amount_minor: i64,
        estimated_min_days: i64,
        estimated_max_days: i64,
    },
    /// `first_minor` for the first unit plus `additional_minor` for each
    /// further unit of the shipping group.
    #[serde(rename_all = "camelCase")]
    PerItem {
        rate_id: String,
        label: String,
        first_minor: i64,
        additional_minor: i64,
        estimated_min_days: i64,
        estimated_max_days: i64,
    },
}

impl PubkyMarketShippingRate {
    /// The identifier of this rate within its zone.
    pub fn rate_id(&self) -> &str {
        match self {
            Self::Free { rate_id, .. }
            | Self::Flat { rate_id, .. }
            | Self::PerItem { rate_id, .. } => rate_id,
        }
    }

    /// The label shown to the buyer.
    pub fn label(&self) -> &str {
        match self {
            Self::Free { label, .. } | Self::Flat { label, .. } | Self::PerItem { label, .. } => {
                label
            }
        }
    }

    /// The `(min, max)` delivery estimate in days.
    pub fn estimated_days(&self) -> (i64, i64) {
        match self {
            Self::Free {
                estimated_min_days,
                estimated_max_days,
                ..
            }
            | Self::Flat {
                estimated_min_days,
                estimated_max_days,
                ..
            }
            | Self::PerItem {
                estimated_min_days,
                estimated_max_days,
                ..
            } => (*estimated_min_days, *estimated_max_days),
        }
    }

    /// What one unit costs under this rate, in minor units.
    pub fn first_item_minor(&self) -> i64 {
        match self {
            Self::Free { .. } => 0,
            Self::Flat { amount_minor, .. } => *amount_minor,
            Self::PerItem { first_minor, .. } => *first_minor,
        }
    }

    fn sanitize(self) -> Self {
        match self {
            Self::Free {
                rate_id,
                label,
                estimated_min_days,
                estimated_max_days,
            } => Self::Free {
                rate_id,
                label: label.trim().to_string(),
                estimated_min_days,
                estimated_max_days,
            },
            Self::Flat {
                rate_id,
                label,
                amount_minor,
                estimated_min_days,
                estimated_max_days,
            } => Self::Flat {
                rate_id,
                label: label.trim().to_string(),
                amount_minor,
                estimated_min_days,
                estimated_max_days,
            },
            Self::PerItem {
                rate_id,
                label,
                first_minor,
                additional_minor,
                estimated_min_days,
                estimated_max_days,
            } => Self::PerItem {
                rate_id,
                label: label.trim().to_string(),
                first_minor,
                additional_minor,
                estimated_min_days,
                estimated_max_days,
            },
        }
    }

    fn validate(&self) -> Result<(), String> {
        validate_entity_id(self.rate_id(), "shipping rate id")?;
        let label_length = self.label().chars().count();
        if !(1..=MAX_LABEL_LENGTH).contains(&label_length) {
            return Err(format!(
                "Validation Error: shipping rate label must be 1-{MAX_LABEL_LENGTH} characters"
            ));
        }
        let amounts: &[i64] = match self {
            Self::Free { .. } => &[],
            Self::Flat { amount_minor, .. } => &[*amount_minor],
            Self::PerItem {
                first_minor,
                additional_minor,
                ..
            } => &[*first_minor, *additional_minor],
        };
        for amount in amounts {
            if !(0..=MAX_SAFE_INTEGER).contains(amount) {
                return Err(
                    "Validation Error: shipping rate amounts must be non-negative safe integers"
                        .into(),
                );
            }
        }
        let (min_days, max_days) = self.estimated_days();
        if !(0..=MAX_DELIVERY_ESTIMATE_DAYS).contains(&min_days)
            || !(0..=MAX_DELIVERY_ESTIMATE_DAYS).contains(&max_days)
        {
            return Err(format!(
                "Validation Error: delivery estimates must be between 0 and {MAX_DELIVERY_ESTIMATE_DAYS} days"
            ));
        }
        if max_days < min_days {
            return Err(
                "Validation Error: maximum delivery estimate must not precede the minimum".into(),
            );
        }
        Ok(())
    }
}

/// The countries one set of rates applies to. `rest_of_world` makes it the
/// zone for every country no zone names; at most one zone may.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketShippingZone {
    pub zone_id: String,
    pub name: String,
    #[serde(default)]
    pub countries: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub rest_of_world: bool,
    pub rates: Vec<PubkyMarketShippingRate>,
}

impl PubkyMarketShippingZone {
    fn sanitize(self) -> Self {
        Self {
            name: self.name.trim().to_string(),
            rates: self
                .rates
                .into_iter()
                .map(PubkyMarketShippingRate::sanitize)
                .collect(),
            ..self
        }
    }

    fn validate(&self) -> Result<(), String> {
        validate_entity_id(&self.zone_id, "shipping zone id")?;
        let name_length = self.name.chars().count();
        if !(MIN_NAME_LENGTH..=MAX_NAME_LENGTH).contains(&name_length) {
            return Err(format!(
                "Validation Error: shipping zone name must be {MIN_NAME_LENGTH}-{MAX_NAME_LENGTH} characters"
            ));
        }
        validate_country_list(&self.countries, "shipping zone countries", MAX_COUNTRIES)?;
        if self.countries.is_empty() && !self.rest_of_world {
            return Err("Validation Error: a shipping zone needs countries or restOfWorld".into());
        }
        if !(1..=MAX_RATES_PER_ZONE).contains(&self.rates.len()) {
            return Err(format!(
                "Validation Error: a shipping zone requires 1-{MAX_RATES_PER_ZONE} rates"
            ));
        }
        let mut seen = HashSet::new();
        for rate in &self.rates {
            rate.validate()?;
            if !seen.insert(rate.rate_id()) {
                return Err("Validation Error: shipping rate ids must be unique".into());
            }
        }
        Ok(())
    }
}

fn sanitize_zones(zones: Vec<PubkyMarketShippingZone>) -> Vec<PubkyMarketShippingZone> {
    zones
        .into_iter()
        .map(PubkyMarketShippingZone::sanitize)
        .collect()
}

/// Validates a zone list: 1-50 zones, unique zone ids, one country in at most
/// one zone and at most one rest-of-world zone.
pub(crate) fn validate_zones(zones: &[PubkyMarketShippingZone]) -> Result<(), String> {
    if !(1..=MAX_ZONES).contains(&zones.len()) {
        return Err(format!(
            "Validation Error: shipping requires 1-{MAX_ZONES} zones"
        ));
    }
    let mut zone_ids = HashSet::new();
    let mut countries = HashSet::new();
    let mut rest_of_world = 0;
    for zone in zones {
        zone.validate()?;
        if !zone_ids.insert(zone.zone_id.as_str()) {
            return Err("Validation Error: shipping zone ids must be unique".into());
        }
        for country in &zone.countries {
            if !countries.insert(country.as_str()) {
                return Err(
                    "Validation Error: a country may belong to only one shipping zone".into(),
                );
            }
        }
        if zone.rest_of_world {
            rest_of_world += 1;
        }
    }
    if rest_of_world > 1 {
        return Err("Validation Error: at most one shipping zone may be restOfWorld".into());
    }
    Ok(())
}

/// A seller's reusable shipping rules: where they ship from, which countries
/// they ship to and what each zone charges. Listings reference a profile by
/// its id, or carry their own zones.
///
/// URI: /pub/pubky.app/marketplace/v1/shipping_profiles/:profile_id
///
/// Amounts are integer minor units of the profile's `currency` and
/// `exponent`; a listing can use the profile only in the same asset.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketShippingProfile {
    /// Marketplace contract version, always `1`.
    pub schema_version: i64,
    /// Record discriminator, always `"shipping_profile"`.
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
    /// Must match the ID in the record's path.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub profile_id: String,
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub name: String,
    /// Uppercase asset code of every amount in the profile.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub currency: String,
    /// Decimal places between the minor and major unit of `currency`.
    pub exponent: i64,
    /// Countries the goods may ship from. Display and filter data.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    #[serde(default)]
    pub origins: Vec<String>,
    /// Countries never shipped to, even when a zone or `restOfWorld` covers them.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    #[serde(default)]
    pub excluded_countries: Vec<String>,
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub zones: Vec<PubkyMarketShippingZone>,
}

#[cfg(target_arch = "wasm32")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl PubkyMarketShippingProfile {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = fromJson))]
    pub fn from_json(js_value: &JsValue) -> Result<Self, String> {
        Self::import_json(js_value)
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = toJson))]
    pub fn to_json(&self) -> Result<JsValue, String> {
        self.export_json()
    }

    /// Getter for `profile_id`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter))]
    pub fn profile_id(&self) -> String {
        self.profile_id.clone()
    }

    /// Getter for `owner_pubky`.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(getter))]
    pub fn owner_pubky(&self) -> String {
        self.owner_pubky.clone()
    }
}

#[cfg(target_arch = "wasm32")]
impl Json for PubkyMarketShippingProfile {}

impl HasIdPath for PubkyMarketShippingProfile {
    const PATH_SEGMENT: &'static str = "marketplace/v1/shipping_profiles/";

    fn create_path(id: &str) -> String {
        [PUBLIC_PATH, APP_PATH, Self::PATH_SEGMENT, id].concat()
    }
}

impl PubkyMarketShippingProfile {
    /// The profile's asset as a zero amount, for money comparisons.
    pub fn asset(&self) -> PubkyMarketMoney {
        PubkyMarketMoney {
            amount_minor: 0,
            currency: self.currency.clone(),
            exponent: self.exponent,
        }
    }

    /// The single `shippingOptions` entry that older readers price a
    /// listing with. See [`legacy_shipping_option`].
    pub fn legacy_shipping_option(
        &self,
        option_id: &str,
        label: &str,
    ) -> Result<PubkyMarketShippingOption, String> {
        legacy_shipping_option(&self.zones, &self.asset(), option_id, label)
    }
}

impl Validatable for PubkyMarketShippingProfile {
    fn sanitize(self) -> Self {
        Self {
            name: self.name.trim().to_string(),
            zones: sanitize_zones(self.zones),
            ..self
        }
    }

    fn validate(&self, id: Option<&str>) -> Result<(), String> {
        if let Some(id) = id {
            validate_entity_id(id, "shipping profile path id")?;
            if self.profile_id != id {
                return Err(
                    "Validation Error: profileId does not match the shipping profile path ID"
                        .into(),
                );
            }
        }
        validate_base_record(
            self.schema_version,
            &self.record_type,
            SHIPPING_PROFILE_RECORD_TYPE,
            &self.owner_pubky,
            self.revision,
            &self.created_at,
            &self.updated_at,
        )?;
        validate_entity_id(&self.profile_id, "profileId")?;
        let name_length = self.name.chars().count();
        if !(MIN_NAME_LENGTH..=MAX_NAME_LENGTH).contains(&name_length) {
            return Err(format!(
                "Validation Error: shipping profile name must be {MIN_NAME_LENGTH}-{MAX_NAME_LENGTH} characters"
            ));
        }
        self.asset().validate_non_negative("shipping profile")?;
        validate_country_list(&self.origins, "origins", MAX_ORIGINS)?;
        validate_country_list(&self.excluded_countries, "excludedCountries", MAX_COUNTRIES)?;
        validate_zones(&self.zones)
    }
}

/// Sanitizes inline zones the way a profile's zones are sanitized.
pub(crate) fn sanitize_inline_zones(
    zones: Option<Vec<PubkyMarketShippingZone>>,
) -> Option<Vec<PubkyMarketShippingZone>> {
    zones.map(sanitize_zones)
}

/// The one `shippingOptions` entry written beside zones so readers that
/// predate them still parse and price the listing: a `free` or `flat` option
/// at the highest of the zones' cheapest first-unit prices, with the widest
/// delivery estimate. A reader that predates zones can only price that
/// option, which cannot express zone differences or per-unit charges;
/// readers that understand zones price from the zones and ignore it.
pub fn legacy_shipping_option(
    zones: &[PubkyMarketShippingZone],
    asset: &PubkyMarketMoney,
    option_id: &str,
    label: &str,
) -> Result<PubkyMarketShippingOption, String> {
    validate_zones(zones)?;
    let mut price = 0;
    let mut min_days = i64::MAX;
    let mut max_days = 0;
    for zone in zones {
        let cheapest = zone
            .rates
            .iter()
            .map(PubkyMarketShippingRate::first_item_minor)
            .min()
            .unwrap_or(0);
        price = price.max(cheapest);
        for rate in &zone.rates {
            let (rate_min, rate_max) = rate.estimated_days();
            min_days = min_days.min(rate_min);
            max_days = max_days.max(rate_max);
        }
    }
    let option = if price == 0 {
        PubkyMarketShippingOption::Free {
            id: option_id.to_string(),
            label: label.trim().to_string(),
            estimated_min_days: min_days,
            estimated_max_days: max_days,
        }
    } else {
        PubkyMarketShippingOption::Flat {
            id: option_id.to_string(),
            label: label.trim().to_string(),
            price: PubkyMarketMoney {
                amount_minor: price,
                currency: asset.currency.clone(),
                exponent: asset.exponent,
            },
            estimated_min_days: min_days,
            estimated_max_days: max_days,
        }
    };
    option.validate()?;
    Ok(option)
}

/// What one rate charges a shipping group.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PubkyMarketShippingQuote {
    pub zone_id: String,
    pub rate_id: String,
    pub label: String,
    pub amount_minor: i64,
    pub estimated_min_days: i64,
    pub estimated_max_days: i64,
}

/// Why a shipping quote could not be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PubkyMarketShippingQuoteError {
    /// The destination is not an uppercase ISO 3166-1 alpha-2 code.
    InvalidDestination,
    /// The unit count is outside `1..=MAX_SHIPPING_GROUP_UNITS`.
    InvalidQuantity,
    /// The destination is excluded, or no zone covers it.
    NotShipped,
    /// The zone has no rate with the requested id.
    UnknownRate,
    /// The quote does not fit a safe integer.
    Overflow,
}

impl PubkyMarketShippingQuoteError {
    /// The stable code the transaction service refuses with.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidDestination => "INVALID_DESTINATION",
            Self::InvalidQuantity => "INVALID_QUANTITY",
            Self::NotShipped => "SHIPPING_UNAVAILABLE_TO_COUNTRY",
            Self::UnknownRate => "UNKNOWN_SHIPPING_RATE",
            Self::Overflow => "SHIPPING_OVERFLOW",
        }
    }
}

impl std::fmt::Display for PubkyMarketShippingQuoteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for PubkyMarketShippingQuoteError {}

/// The zone that serves `destination`: an excluded country is never served;
/// an explicit country wins over `restOfWorld`; a country no zone covers is
/// not served.
pub fn shipping_zone_for_destination<'a>(
    zones: &'a [PubkyMarketShippingZone],
    excluded_countries: &[String],
    destination: &str,
) -> Result<&'a PubkyMarketShippingZone, PubkyMarketShippingQuoteError> {
    validate_country_code(destination, "destination")
        .map_err(|_| PubkyMarketShippingQuoteError::InvalidDestination)?;
    if excluded_countries.iter().any(|code| code == destination) {
        return Err(PubkyMarketShippingQuoteError::NotShipped);
    }
    zones
        .iter()
        .find(|zone| zone.countries.iter().any(|code| code == destination))
        .or_else(|| zones.iter().find(|zone| zone.rest_of_world))
        .ok_or(PubkyMarketShippingQuoteError::NotShipped)
}

fn quote_rate(
    zone: &PubkyMarketShippingZone,
    rate: &PubkyMarketShippingRate,
    units: i64,
) -> Result<PubkyMarketShippingQuote, PubkyMarketShippingQuoteError> {
    if !(1..=MAX_SHIPPING_GROUP_UNITS).contains(&units) {
        return Err(PubkyMarketShippingQuoteError::InvalidQuantity);
    }
    let amount_minor = match rate {
        PubkyMarketShippingRate::Free { .. } => 0,
        PubkyMarketShippingRate::Flat { amount_minor, .. } => *amount_minor,
        PubkyMarketShippingRate::PerItem {
            first_minor,
            additional_minor,
            ..
        } => (units - 1)
            .checked_mul(*additional_minor)
            .and_then(|additional| additional.checked_add(*first_minor))
            .ok_or(PubkyMarketShippingQuoteError::Overflow)?,
    };
    if !(0..=MAX_SAFE_INTEGER).contains(&amount_minor) {
        return Err(PubkyMarketShippingQuoteError::Overflow);
    }
    let (estimated_min_days, estimated_max_days) = rate.estimated_days();
    Ok(PubkyMarketShippingQuote {
        zone_id: zone.zone_id.clone(),
        rate_id: rate.rate_id().to_string(),
        label: rate.label().to_string(),
        amount_minor,
        estimated_min_days,
        estimated_max_days,
    })
}

/// Quotes one shipping group of `units` units to `destination` under the
/// rate `rate_id` of the zone that serves it. A group is the lines of one
/// seller order that share one profile (or one listing's inline zones).
pub fn quote_shipping(
    zones: &[PubkyMarketShippingZone],
    excluded_countries: &[String],
    destination: &str,
    rate_id: &str,
    units: i64,
) -> Result<PubkyMarketShippingQuote, PubkyMarketShippingQuoteError> {
    let zone = shipping_zone_for_destination(zones, excluded_countries, destination)?;
    let rate = zone
        .rates
        .iter()
        .find(|rate| rate.rate_id() == rate_id)
        .ok_or(PubkyMarketShippingQuoteError::UnknownRate)?;
    quote_rate(zone, rate, units)
}

/// Quotes every rate of the zone that serves `destination`, in the zone's
/// order, for the buyer's rate choice.
pub fn quote_all_shipping_rates(
    zones: &[PubkyMarketShippingZone],
    excluded_countries: &[String],
    destination: &str,
    units: i64,
) -> Result<Vec<PubkyMarketShippingQuote>, PubkyMarketShippingQuoteError> {
    let zone = shipping_zone_for_destination(zones, excluded_countries, destination)?;
    zone.rates
        .iter()
        .map(|rate| quote_rate(zone, rate, units))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWNER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";

    fn rate_per_item(id: &str, first: i64, additional: i64) -> PubkyMarketShippingRate {
        PubkyMarketShippingRate::PerItem {
            rate_id: id.into(),
            label: "Standard".into(),
            first_minor: first,
            additional_minor: additional,
            estimated_min_days: 4,
            estimated_max_days: 8,
        }
    }

    fn rate_flat(id: &str, amount: i64) -> PubkyMarketShippingRate {
        PubkyMarketShippingRate::Flat {
            rate_id: id.into(),
            label: "Express".into(),
            amount_minor: amount,
            estimated_min_days: 2,
            estimated_max_days: 4,
        }
    }

    fn zones() -> Vec<PubkyMarketShippingZone> {
        vec![
            PubkyMarketShippingZone {
                zone_id: "us".into(),
                name: "United States".into(),
                countries: vec!["US".into()],
                rest_of_world: false,
                rates: vec![rate_per_item("std", 479, 240), rate_flat("exp", 1499)],
            },
            PubkyMarketShippingZone {
                zone_id: "eu".into(),
                name: "Europe".into(),
                countries: vec!["DE".into(), "PT".into()],
                rest_of_world: false,
                rates: vec![rate_per_item("std", 899, 450)],
            },
            PubkyMarketShippingZone {
                zone_id: "row".into(),
                name: "Rest of world".into(),
                countries: vec![],
                rest_of_world: true,
                rates: vec![PubkyMarketShippingRate::Free {
                    rate_id: "free".into(),
                    label: "Free".into(),
                    estimated_min_days: 10,
                    estimated_max_days: 20,
                }],
            },
        ]
    }

    fn profile() -> PubkyMarketShippingProfile {
        PubkyMarketShippingProfile {
            schema_version: 1,
            record_type: "shipping_profile".into(),
            owner_pubky: OWNER.into(),
            revision: 1,
            created_at: "2026-11-05T10:00:00Z".into(),
            updated_at: "2026-11-05T10:00:00Z".into(),
            profile_id: "printful".into(),
            name: " Printful worldwide ".into(),
            currency: "USD".into(),
            exponent: 2,
            origins: vec!["US".into(), "LV".into()],
            excluded_countries: vec!["RU".into()],
            zones: zones(),
        }
    }

    #[test]
    fn valid_profile_roundtrips_and_sanitizes() {
        let blob = serde_json::to_vec(&profile()).unwrap();
        let parsed = <PubkyMarketShippingProfile as Validatable>::try_from(&blob, "printful")
            .expect("valid profile");
        assert_eq!(parsed.name, "Printful worldwide");
        let value = serde_json::to_value(&parsed).unwrap();
        assert_eq!(value["recordType"], "shipping_profile");
        assert_eq!(value["zones"][0]["rates"][0]["pricing"], "per_item");
        assert_eq!(value["zones"][0]["rates"][0]["firstMinor"], 479);
        assert!(value["zones"][0].get("restOfWorld").is_none());
        assert_eq!(value["zones"][2]["restOfWorld"], true);
    }

    #[test]
    fn profile_rejects_each_invalid_shape() {
        type Mutation = Box<dyn Fn(&mut PubkyMarketShippingProfile)>;
        let cases: Vec<(&str, Mutation)> = vec![
            ("path id", Box::new(|p| p.profile_id = "other".into())),
            ("currency", Box::new(|p| p.currency = "usd".into())),
            ("exponent", Box::new(|p| p.exponent = 19)),
            ("no zones", Box::new(|p| p.zones.clear())),
            (
                "duplicate zone id",
                Box::new(|p| p.zones[1].zone_id = "us".into()),
            ),
            (
                "country in two zones",
                Box::new(|p| p.zones[1].countries.push("US".into())),
            ),
            (
                "two rest of world",
                Box::new(|p| p.zones[1].rest_of_world = true),
            ),
            (
                "empty zone",
                Box::new(|p| {
                    p.zones[1].countries.clear();
                }),
            ),
            ("no rates", Box::new(|p| p.zones[0].rates.clear())),
            (
                "duplicate rate id",
                Box::new(|p| p.zones[0].rates[1] = rate_flat("std", 1)),
            ),
            (
                "negative amount",
                Box::new(|p| p.zones[0].rates[1] = rate_flat("exp", -1)),
            ),
            (
                "unsafe amount",
                Box::new(|p| p.zones[0].rates[1] = rate_flat("exp", MAX_SAFE_INTEGER + 1)),
            ),
            (
                "lowercase country",
                Box::new(|p| p.origins = vec!["us".into()]),
            ),
            (
                "duplicate origin",
                Box::new(|p| p.origins = vec!["US".into(), "US".into()]),
            ),
            (
                "estimate order",
                Box::new(|p| {
                    p.zones[0].rates[1] = PubkyMarketShippingRate::Flat {
                        rate_id: "exp".into(),
                        label: "Express".into(),
                        amount_minor: 1,
                        estimated_min_days: 5,
                        estimated_max_days: 4,
                    }
                }),
            ),
            (
                "estimate bound",
                Box::new(|p| {
                    p.zones[0].rates[1] = PubkyMarketShippingRate::Flat {
                        rate_id: "exp".into(),
                        label: "Express".into(),
                        amount_minor: 1,
                        estimated_min_days: 0,
                        estimated_max_days: 366,
                    }
                }),
            ),
            (
                "empty label",
                Box::new(|p| {
                    p.zones[0].rates[1] = {
                        PubkyMarketShippingRate::Flat {
                            rate_id: "exp".into(),
                            label: "   ".into(),
                            amount_minor: 1,
                            estimated_min_days: 0,
                            estimated_max_days: 1,
                        }
                    }
                }),
            ),
            ("name", Box::new(|p| p.name = "  ".into())),
        ];
        for (label, mutate) in cases {
            let mut candidate = profile();
            mutate(&mut candidate);
            let blob = serde_json::to_vec(&candidate).unwrap();
            assert!(
                <PubkyMarketShippingProfile as Validatable>::try_from(&blob, "printful").is_err(),
                "{label} must be rejected"
            );
        }
    }

    #[test]
    fn unknown_pricing_is_rejected_not_ignored() {
        let mut value = serde_json::to_value(profile()).unwrap();
        value["zones"][0]["rates"][0]["pricing"] = "by_weight".into();
        let blob = serde_json::to_vec(&value).unwrap();
        assert!(<PubkyMarketShippingProfile as Validatable>::try_from(&blob, "printful").is_err());
    }

    #[test]
    fn unknown_members_are_ignored() {
        let mut value = serde_json::to_value(profile()).unwrap();
        value["futureField"] = "x".into();
        value["zones"][0]["futureZoneField"] = 1.into();
        let blob = serde_json::to_vec(&value).unwrap();
        assert!(<PubkyMarketShippingProfile as Validatable>::try_from(&blob, "printful").is_ok());
    }

    #[test]
    fn zone_selection_follows_the_precedence() {
        let z = zones();
        let excluded = vec!["DE".to_string(), "RU".to_string()];
        assert_eq!(
            shipping_zone_for_destination(&z, &excluded, "US")
                .unwrap()
                .zone_id,
            "us"
        );
        assert_eq!(
            shipping_zone_for_destination(&z, &excluded, "PT")
                .unwrap()
                .zone_id,
            "eu"
        );
        assert_eq!(
            shipping_zone_for_destination(&z, &excluded, "BR")
                .unwrap()
                .zone_id,
            "row"
        );
        assert_eq!(
            shipping_zone_for_destination(&z, &excluded, "DE"),
            Err(PubkyMarketShippingQuoteError::NotShipped)
        );
        assert_eq!(
            shipping_zone_for_destination(&z, &excluded, "RU"),
            Err(PubkyMarketShippingQuoteError::NotShipped)
        );
        assert_eq!(
            shipping_zone_for_destination(&z[..2], &[], "BR"),
            Err(PubkyMarketShippingQuoteError::NotShipped)
        );
        for bad in ["", "us", "USA", "U1", "ü"] {
            assert_eq!(
                shipping_zone_for_destination(&z, &[], bad),
                Err(PubkyMarketShippingQuoteError::InvalidDestination),
                "{bad}"
            );
        }
    }

    #[test]
    fn quotes_charge_flat_once_and_per_item_first_plus_additional() {
        let z = zones();
        let quote = |dest: &str, rate: &str, units: i64| {
            quote_shipping(&z, &[], dest, rate, units).map(|q| q.amount_minor)
        };
        assert_eq!(quote("US", "std", 1), Ok(479));
        assert_eq!(quote("US", "std", 3), Ok(479 + 2 * 240));
        assert_eq!(quote("US", "exp", 1), Ok(1499));
        assert_eq!(quote("US", "exp", 10), Ok(1499));
        assert_eq!(quote("BR", "free", 7), Ok(0));
        assert_eq!(
            quote("US", "free", 1),
            Err(PubkyMarketShippingQuoteError::UnknownRate)
        );
        assert_eq!(
            quote("US", "std", 0),
            Err(PubkyMarketShippingQuoteError::InvalidQuantity)
        );
        assert_eq!(
            quote("US", "std", -1),
            Err(PubkyMarketShippingQuoteError::InvalidQuantity)
        );
        assert_eq!(
            quote("US", "std", MAX_SHIPPING_GROUP_UNITS + 1),
            Err(PubkyMarketShippingQuoteError::InvalidQuantity)
        );
    }

    #[test]
    fn per_item_overflow_is_refused() {
        let zone = PubkyMarketShippingZone {
            zone_id: "z".into(),
            name: "Z".into(),
            countries: vec!["US".into()],
            rest_of_world: false,
            rates: vec![rate_per_item("std", 0, MAX_SAFE_INTEGER)],
        };
        assert_eq!(
            quote_shipping(std::slice::from_ref(&zone), &[], "US", "std", 2)
                .map(|q| q.amount_minor),
            Ok(MAX_SAFE_INTEGER)
        );
        assert_eq!(
            quote_shipping(&[zone], &[], "US", "std", 3),
            Err(PubkyMarketShippingQuoteError::Overflow)
        );
        let huge = PubkyMarketShippingZone {
            zone_id: "z".into(),
            name: "Z".into(),
            countries: vec!["US".into()],
            rest_of_world: false,
            rates: vec![rate_per_item("std", MAX_SAFE_INTEGER, 1)],
        };
        assert_eq!(
            quote_shipping(&[huge], &[], "US", "std", 2),
            Err(PubkyMarketShippingQuoteError::Overflow)
        );
    }

    #[test]
    fn all_rates_are_quoted_in_zone_order() {
        let quotes = quote_all_shipping_rates(&zones(), &[], "US", 2).unwrap();
        let summary: Vec<(&str, i64)> = quotes
            .iter()
            .map(|q| (q.rate_id.as_str(), q.amount_minor))
            .collect();
        assert_eq!(summary, vec![("std", 719), ("exp", 1499)]);
        assert_eq!(quotes[0].estimated_min_days, 4);
        assert_eq!(quotes[0].zone_id, "us");
    }

    #[test]
    fn legacy_option_takes_the_highest_cheapest_price_and_widest_estimate() {
        let option = profile()
            .legacy_shipping_option("legacy", " Worldwide ")
            .unwrap();
        match option {
            PubkyMarketShippingOption::Flat {
                id,
                label,
                price,
                estimated_min_days,
                estimated_max_days,
            } => {
                assert_eq!(id, "legacy");
                assert_eq!(label, "Worldwide");
                assert_eq!(price.amount_minor, 899);
                assert_eq!(price.currency, "USD");
                assert_eq!(price.exponent, 2);
                assert_eq!(estimated_min_days, 2);
                assert_eq!(estimated_max_days, 20);
            }
            other => panic!("expected flat, got {other:?}"),
        }
        let free_zone = vec![PubkyMarketShippingZone {
            zone_id: "all".into(),
            name: "All".into(),
            countries: vec![],
            rest_of_world: true,
            rates: vec![PubkyMarketShippingRate::Free {
                rate_id: "free".into(),
                label: "Free".into(),
                estimated_min_days: 3,
                estimated_max_days: 9,
            }],
        }];
        let option =
            legacy_shipping_option(&free_zone, &profile().asset(), "legacy", "Free").unwrap();
        assert!(matches!(option, PubkyMarketShippingOption::Free { .. }));
    }

    #[test]
    fn uri_path_uses_the_collection_segment() {
        assert_eq!(
            PubkyMarketShippingProfile::create_path("printful"),
            "/pub/pubky.app/marketplace/v1/shipping_profiles/printful"
        );
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ZoneSet {
        zones: Vec<PubkyMarketShippingZone>,
        excluded_countries: Vec<String>,
    }

    fn vectors() -> serde_json::Value {
        serde_json::from_str(include_str!("../test_fixtures/shipping-quotes-v1.json"))
            .expect("shipping vectors parse")
    }

    fn zone_set(document: &serde_json::Value, name: &str) -> ZoneSet {
        serde_json::from_value(document["zoneSets"][name].clone()).expect("zone set parses")
    }

    fn expected_result(expected: &serde_json::Value) -> Result<serde_json::Value, String> {
        match expected.get("error") {
            Some(code) => Err(code.as_str().unwrap().to_string()),
            None => Ok(expected.clone()),
        }
    }

    #[test]
    fn shared_quote_vectors_hold() {
        let document = vectors();
        for (name, set) in document["zoneSets"].as_object().unwrap() {
            let set: ZoneSet = serde_json::from_value(set.clone()).unwrap();
            let mut candidate = profile();
            candidate.zones = set.zones;
            candidate.excluded_countries = set.excluded_countries;
            let blob = serde_json::to_vec(&candidate).unwrap();
            assert!(
                <PubkyMarketShippingProfile as Validatable>::try_from(&blob, "printful").is_ok(),
                "zone set {name} is a valid profile"
            );
        }
        let cases = document["quotes"].as_array().unwrap();
        assert!(cases.len() >= 100);
        for case in cases {
            let set = zone_set(&document, case["zones"].as_str().unwrap());
            let result = quote_shipping(
                &set.zones,
                &set.excluded_countries,
                case["destination"].as_str().unwrap(),
                case["rateId"].as_str().unwrap(),
                case["units"].as_i64().unwrap(),
            )
            .map(|quote| serde_json::to_value(quote).unwrap())
            .map_err(|error| error.code().to_string());
            assert_eq!(result, expected_result(&case["expected"]), "{case}");
        }
    }

    #[test]
    fn shared_all_rates_vectors_hold() {
        let document = vectors();
        for case in document["allRates"].as_array().unwrap() {
            let set = zone_set(&document, case["zones"].as_str().unwrap());
            let result = quote_all_shipping_rates(
                &set.zones,
                &set.excluded_countries,
                case["destination"].as_str().unwrap(),
                case["units"].as_i64().unwrap(),
            )
            .map(|quotes| serde_json::json!({ "quotes": quotes }))
            .map_err(|error| error.code().to_string());
            assert_eq!(result, expected_result(&case["expected"]), "{case}");
        }
    }

    #[test]
    fn shared_legacy_vectors_hold() {
        let document = vectors();
        let asset = PubkyMarketMoney {
            amount_minor: 0,
            currency: document["currency"].as_str().unwrap().to_string(),
            exponent: document["exponent"].as_i64().unwrap(),
        };
        for case in document["legacy"].as_array().unwrap() {
            let zones: Vec<PubkyMarketShippingZone> =
                serde_json::from_value(case["zones"].clone()).unwrap();
            let option = legacy_shipping_option(&zones, &asset, "legacy", "Shipping").unwrap();
            assert_eq!(serde_json::to_value(option).unwrap(), case["expected"]);
        }
    }
}
