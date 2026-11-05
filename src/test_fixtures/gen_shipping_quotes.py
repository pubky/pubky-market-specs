#!/usr/bin/env python3
"""Regenerates shipping-quotes-v1.json.

The expected values come from the pricing rules in SPEC.md (Shipping
profiles), written here without reference to the Rust implementation:

    python3 src/test_fixtures/gen_shipping_quotes.py
"""
import json
import pathlib

MAX = 9007199254740991
MAX_UNITS = 1000000
OUT = pathlib.Path(__file__).with_name("shipping-quotes-v1.json")


def rate(kind, rate_id, label, min_days, max_days, **amounts):
    return {
        "pricing": kind,
        "rateId": rate_id,
        "label": label,
        "estimatedMinDays": min_days,
        "estimatedMaxDays": max_days,
        **amounts,
    }


ZONES = [
    {"zoneId": "us", "name": "United States", "countries": ["US"], "rates": [
        rate("per_item", "std", "Standard", 4, 8, firstMinor=479, additionalMinor=240),
        rate("flat", "exp", "Express", 2, 4, amountMinor=1499)]},
    {"zoneId": "eu", "name": "Europe", "countries": ["DE", "PT", "FR"], "rates": [
        rate("per_item", "std", "Standard", 6, 12, firstMinor=899, additionalMinor=450)]},
    {"zoneId": "row", "name": "Rest of world", "countries": [], "restOfWorld": True, "rates": [
        rate("free", "free", "Free", 10, 20)]},
]
EXCLUDED = ["FR", "RU"]
BIG = [{"zoneId": "big", "name": "Big", "countries": ["US"], "rates": [
    rate("per_item", "std", "Standard", 1, 2, firstMinor=MAX - 5, additionalMinor=3),
    rate("per_item", "huge", "Huge", 1, 2, firstMinor=0, additionalMinor=MAX)]}]
FREE = [{"zoneId": "all", "name": "All", "countries": [], "restOfWorld": True,
         "rates": [rate("free", "free", "Free", 3, 9)]}]


def zone_for(zones, excluded, destination):
    if len(destination) != 2 or not all("A" <= c <= "Z" for c in destination):
        return None, "INVALID_DESTINATION"
    if destination in excluded:
        return None, "SHIPPING_UNAVAILABLE_TO_COUNTRY"
    for zone in zones:
        if destination in zone["countries"]:
            return zone, None
    for zone in zones:
        if zone.get("restOfWorld"):
            return zone, None
    return None, "SHIPPING_UNAVAILABLE_TO_COUNTRY"


def price(r, units):
    if r["pricing"] == "free":
        return 0
    if r["pricing"] == "flat":
        return r["amountMinor"]
    return r["firstMinor"] + (units - 1) * r["additionalMinor"]


def quote(zones, excluded, destination, rate_id, units):
    zone, error = zone_for(zones, excluded, destination)
    if error:
        return {"error": error}
    chosen = next((r for r in zone["rates"] if r["rateId"] == rate_id), None)
    if chosen is None:
        return {"error": "UNKNOWN_SHIPPING_RATE"}
    if not 1 <= units <= MAX_UNITS:
        return {"error": "INVALID_QUANTITY"}
    amount = price(chosen, units)
    if amount > MAX:
        return {"error": "SHIPPING_OVERFLOW"}
    return {"zoneId": zone["zoneId"], "rateId": rate_id, "label": chosen["label"],
            "amountMinor": amount, "estimatedMinDays": chosen["estimatedMinDays"],
            "estimatedMaxDays": chosen["estimatedMaxDays"]}


def legacy(zones, currency, exponent):
    highest_cheapest, min_days, max_days = 0, 10**9, 0
    for zone in zones:
        highest_cheapest = max(highest_cheapest, min(price(r, 1) for r in zone["rates"]))
        for r in zone["rates"]:
            min_days = min(min_days, r["estimatedMinDays"])
            max_days = max(max_days, r["estimatedMaxDays"])
    base = {"id": "legacy", "label": "Shipping",
            "estimatedMinDays": min_days, "estimatedMaxDays": max_days}
    if highest_cheapest == 0:
        return {"pricing": "free", **base}
    return {"pricing": "flat", **base,
            "price": {"amountMinor": highest_cheapest, "currency": currency, "exponent": exponent}}


def main():
    sets = {"main": (ZONES, EXCLUDED), "big": (BIG, [])}
    quotes = []

    def add(name, destination, rate_id, units):
        zones, excluded = sets[name]
        quotes.append({"zones": name, "destination": destination, "rateId": rate_id,
                       "units": units,
                       "expected": quote(zones, excluded, destination, rate_id, units)})

    for destination in ["US", "DE", "PT", "FR", "RU", "BR", "JP"]:
        for rate_id in ["std", "exp", "free", "missing"]:
            for units in [1, 2, 5]:
                add("main", destination, rate_id, units)
    for destination in ["us", "USA", "", "U1"]:
        add("main", destination, "std", 1)
    for units in [0, -1, 1000001, 1000000]:
        add("main", "US", "std", units)
    for rate_id in ["std", "huge"]:
        for units in [1, 2, 3, 1000000]:
            add("big", "US", rate_id, units)

    all_rates = []
    for destination, units in [("US", 1), ("US", 3), ("DE", 2), ("BR", 4), ("FR", 1), ("JP", 1000000)]:
        zone, error = zone_for(ZONES, EXCLUDED, destination)
        expected = {"error": error} if error else {
            "quotes": [quote(ZONES, EXCLUDED, destination, r["rateId"], units) for r in zone["rates"]]}
        all_rates.append({"zones": "main", "destination": destination, "units": units,
                          "expected": expected})

    document = {
        "description": "Shipping quote vectors for quote_shipping, quote_all_shipping_rates and legacy_shipping_option. Expected values are computed by gen_shipping_quotes.py from the pricing rules in SPEC.md. Errors carry the stable service code.",
        "currency": "USD",
        "exponent": 2,
        "zoneSets": {name: {"zones": z, "excludedCountries": e} for name, (z, e) in sets.items()},
        "quotes": quotes,
        "allRates": all_rates,
        "legacy": [{"zones": ZONES, "expected": legacy(ZONES, "USD", 2)},
                   {"zones": FREE, "expected": legacy(FREE, "USD", 2)}],
    }
    OUT.write_text(json.dumps(document, indent=2))


main()
