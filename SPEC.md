# Pubky Market data model specification

_Version 0.1.0. Wire format identical to the marketplace records of `pubky-app-specs` `0.6.2-marketplace.16`._

## Table of Contents

- [Introduction](#introduction)
- [Concepts](#concepts)
- [Data Models](#data-models)
  - [PubkyMarketShop](#pubkymarketshop)
  - [PubkyMarketListing](#pubkymarketlisting)
  - [PubkyMarketDrop](#pubkymarketdrop)
  - [PubkyMarketReview](#pubkymarketreview)
  - [Purchase Attestation (embedded JWS)](#purchase-attestation-embedded-jws)
  - [PubkyMarketAttestorProfile and PubkyMarketAttestorAnnotation](#pubkymarketattestorprofile-and-pubkymarketattestorannotation)
  - [PubkyMarketReviewResponse](#pubkymarketreviewresponse)
  - [PubkyMarketWatchlist (private)](#pubkymarketwatchlist-private)
  - [PubkyMarketOrderReceipt (private)](#pubkymarketorderreceipt-private)
  - [Order Receipt Attestation (embedded JWS)](#order-receipt-attestation-embedded-jws)
  - [Drop Edition Attestation (embedded JWS)](#drop-edition-attestation-embedded-jws)
  - [Encrypted Private Records (`pubky-priv-aead/v1`)](#encrypted-private-records-pubky-priv-aeadv1)
    - [Badge checkpoints](#badge-checkpoints)
- [Validation Rules](#validation-rules)
- [License](#license)

---

## Introduction

This document specifies the Pubky Market records: shops, listings, drops, reviews, review responses, the private watchlist and order receipts, the attestations embedded in reviews and receipts, and the attestor records. It defines their paths, fields, serialization and validation rules for developers building compatible clients, indexers and services. The Rust models in [`src/`](src/) implement it.

Every record lives under the `pubky.app` app directory of its owner's homeserver: public records under `/pub/pubky.app/marketplace/v1/`, private records under `/priv/pubky.app/marketplace/` (plaintext `v1/` records and the encrypted `v2/` entries of `pubky-priv-aead/v1`). Social records (profiles, posts, tags, follows, files) are specified elsewhere and are not part of this document. A tag on a listing or shop is a social record whose target is the listing or shop URI.

---

## Concepts

- **Timestamp IDs** (listings): 13-character Crockford Base32 encodings of a microsecond UNIX timestamp, after 1 October 2024 and at most two hours in the future.
- **Hash IDs** (reviews): Crockford Base32 encodings of the first 16 bytes of the Blake3 hash of the record's identifying fields, so the same review always lands at the same path.
- **Entity IDs** (drops, attestor annotations): identifiers chosen by the writer, bounded to a path-safe alphabet. Order receipts use the transaction service's receipt UUID.
- **Open-world records**: unknown members are ignored when parsing and are not written back, so record shapes can grow additively. The attestation headers and claim sets are closed-world: an attestation is an opaque signed string, not an evolving record.
- **Money** is always integer minor units: `amountMinor`, `currency`, `exponent`.

---

## Data Models

### PubkyMarketShop

**Description:** Represents a seller's marketplace shop profile (singleton per user). All fields are serialized in camelCase and unknown members are ignored when parsing and are not written back.

**URI:** `/pub/pubky.app/marketplace/v1/shop.json`

| **Field**        | **Type** | **Description**                        | **Validation Rules**                                                              |
| ---------------- | -------- | -------------------------------------- | --------------------------------------------------------------------------------- |
| `schemaVersion`  | Integer  | Marketplace contract version.          | Required. Must be `1`.                                                            |
| `recordType`     | String   | Record discriminator.                  | Required. Must be `"shop"`.                                                       |
| `ownerPubky`     | String   | Pubky of the shop owner.               | Required. 52-character z-base-32 pubky.                                           |
| `revision`       | Integer  | Record revision.                       | Required. Positive safe integer.                                                  |
| `createdAt`      | String   | Creation datetime.                     | Required. ISO-8601 with offset (`Z` or `±HH:MM`).                                 |
| `updatedAt`      | String   | Last-update datetime.                  | Required. ISO-8601 with offset. Must not precede `createdAt`.                     |
| `name`           | String   | Shop display name.                     | Required. Trimmed. Length: 1–60 characters.                                       |
| `bio`            | String   | Shop description.                      | Required (may be empty). Trimmed. Maximum length: 1000 characters.                |
| `location`       | Object   | Public location.                       | Required. `countryCode` ISO 3166-1 alpha-2; optional `region` 1–100 characters.   |
| `avatarUrl`      | String   | Shop avatar media URI.                 | Optional. Must be a Pubky marketplace v1 URI.                                     |
| `bannerUrl`      | String   | Shop banner media URI.                 | Optional. Must be a Pubky marketplace v1 URI.                                     |
| `shippingPolicy` | String   | Default shipping policy.               | Required (may be empty). Trimmed. Maximum length: 4000 characters.                |
| `returnPolicy`   | String   | Default return policy.                 | Required (may be empty). Trimmed. Maximum length: 4000 characters.                |
| `vacationMode`   | Boolean  | Whether the shop is paused (vacation). | Required.                                                                          |
| `transactionService` | String | HTTPS base URL of the marketplace transaction service this shop sells through. | Optional. Must parse as a URL with scheme exactly `https`, no credentials, no query, no fragment; maximum length 300 characters. Absent fields are not serialized (records without it round-trip unchanged). |
| `deliveryAssumeDays` | Integer | Days after shipment when delivery is assumed (the transaction service has no carrier feed). | Optional. Whole days, 1–60. Absent means the transaction service's configured default applies. Absent fields are not serialized. |
| `autoCompleteDays` | Integer | Days after delivery when the order completes unless a return is opened. | Optional. Whole days, 1–60. Absent means the transaction service's configured default applies. Absent fields are not serialized. |

**`transactionService` semantics:** when present, clients MUST resolve transactional commands (checkout, offers, bids, orders) for this shop against this authority, falling back to their configured default when absent. Two different services may register the same public listing — the shop record is the seller's declaration of which one is authoritative.

---

### PubkyMarketListing

**Description:** Represents a marketplace listing published by a seller. All fields are serialized in camelCase and unknown members are ignored when parsing and are not written back. Money is always integer minor units (`amountMinor`, `currency`, `exponent`).

**URI:** `/pub/pubky.app/marketplace/v1/listings/:listing_id`

| **Field**            | **Type** | **Description**                          | **Validation Rules**                                                                                              |
| -------------------- | -------- | ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| `schemaVersion`      | Integer  | Marketplace contract version.            | Required. Must be `1`.                                                                                             |
| `recordType`         | String   | Record discriminator.                    | Required. Must be `"listing"`.                                                                                     |
| `ownerPubky`         | String   | Pubky of the seller.                     | Required. 52-character z-base-32 pubky.                                                                            |
| `revision`           | Integer  | Record revision.                         | Required. Positive safe integer.                                                                                   |
| `createdAt`          | String   | Creation datetime.                       | Required. ISO-8601 with offset.                                                                                    |
| `updatedAt`          | String   | Last-update datetime.                    | Required. ISO-8601 with offset. Must not precede `createdAt`.                                                      |
| `listingId`          | String   | Listing identifier.                      | Required. Path-safe id that must equal the **Timestamp ID** in the record path.                                    |
| `state`              | String   | Listing lifecycle state.                 | Required. One of `active`, `paused`, `ended`, `removed`. Active listings need an enabled variant with stock.       |
| `title`              | String   | Listing title.                           | Required. Trimmed. Length: 3–80 characters.                                                                        |
| `description`        | String   | Listing description.                     | Required. Trimmed. Length: 1–10000 characters.                                                                     |
| `taxonomyVersion`    | Integer  | Category taxonomy version.               | Required. Integer between 1 and 1000000. The category tree and per-category attribute expectations are client configuration keyed by this number. |
| `categoryId`         | String   | Marketplace category.                    | Required. Kebab-case identifier, 1–120 characters.                                                                 |
| `condition`          | String   | Item condition.                          | Required. One of `new`, `like_new`, `excellent`, `good`, `fair`, `for_parts`.                                      |
| `attributes`         | Object   | Item specifics (key/value container).    | Optional. Map of at most 20 keys to a string or an array of 1–10 unique strings. Keys are lowercase alphanumeric identifiers with single `-`/`_` separators, 1–40 characters. Values are trimmed, 1–80 characters. Which keys a category expects is client configuration keyed by `taxonomyVersion`. |
| `conditionDetails`   | String   | Extra condition notes.                   | Optional. Trimmed. Maximum length: 1000 characters.                                                                |
| `tags`               | Array    | Search tags.                             | Required. Up to 10 unique, trimmed strings of 1–40 characters.                                                     |
| `location`           | Object   | Public location.                         | Required. Same rules as the shop location.                                                                         |
| `media`              | Array    | Media attachments.                       | Required. 1–13 entries; 1–12 images and at most 1 video; unique ids; URIs must be seller-owned marketplace media.  |
| `variants`           | Array    | Purchasable variants (SKUs).             | Required. 1–100 entries; unique ids and SKUs; quantity 0–1000000; media references must exist.                     |
| `sale`               | Object   | Sale terms.                              | Required. `format` is `fixed_price` (positive `unitPrice`, `acceptsOffers`) or `auction` (see below).              |
| `fulfillmentMethods` | Array    | Delivery methods.                        | Required. 1–4 unique values of `physical`, `digital`, `pickup`, `shipping`.                                        |
| `package`            | Object   | Package facts (weight/dimensions).       | Required with `physical` fulfillment, forbidden otherwise.                                                         |
| `shippingOptions`    | Array    | Shipping options.                        | Up to 20 unique-id options (`free`, `flat`, `calculated`). Required non-empty with `physical`, forbidden otherwise. |
| `returnPolicy`       | Object   | Return policy.                           | Required. Return window (1–365 days) required iff returns are accepted.                                            |
| `digitalLock`        | Object   | Locks policy for digital delivery.       | Optional with `digital` fulfillment (Locks delivery), forbidden otherwise.                                         |
| `adultOnly`          | Boolean  | Adult-content flag.                      | Required.                                                                                                          |

**Auction rules:** all auction prices must share one asset and exponent; `endsAt` must follow `startsAt`; reserve price must not be below the starting price; buy-now price must exceed the starting price; anti-sniping windows are 0–3600 seconds; auctions require exactly one variant. Variant price overrides and flat shipping prices must use the listing asset.

**Validation Notes:**

- The `listing_id` in the URI must be a valid **Timestamp ID** and must match the record's `listingId` field.

---

### PubkyMarketDrop

**Description:** Represents a marketplace drop: a seller's scheduled, limited-quantity release bundling one or more of their own listings. This is a PUBLIC record (unlike the private order receipt) wired into the URI parser and `PubkyMarketObject` so Nexus can index it. All fields are serialized in camelCase and unknown members are ignored when parsing and are not written back.

**URI:** `/pub/pubky.app/marketplace/v1/drops/:drop_id`

| **Field**        | **Type** | **Description**                              | **Validation Rules**                                                              |
| ---------------- | -------- | -------------------------------------------- | ---------------------------------------------------------------------------------- |
| `schemaVersion`  | Integer  | Marketplace contract version.                | Required. Must be `1`.                                                             |
| `recordType`     | String   | Record discriminator.                        | Required. Must be `"drop"`.                                                        |
| `ownerPubky`     | String   | Pubky of the seller.                         | Required. 52-character z-base-32 pubky.                                            |
| `revision`       | Integer  | Record revision.                             | Required. Positive safe integer.                                                   |
| `createdAt`      | String   | Creation datetime.                           | Required. ISO-8601 with offset (`Z` or `±HH:MM`).                                  |
| `updatedAt`      | String   | Last-update datetime.                        | Required. ISO-8601 with offset. Must not precede `createdAt`.                      |
| `dropId`         | String   | Drop identifier.                             | Required. Path-safe entity ID (1–128 chars of `[A-Za-z0-9_-]`); must equal the id in the record path. |
| `title`          | String   | Drop title.                                  | Required. Trimmed. Length: 1–120 characters.                                       |
| `description`    | String   | Drop description.                            | Required (may be empty). Trimmed. Maximum length: 2000 characters.                 |
| `media`          | Array    | Promotional media URIs.                      | Required (may be empty). At most 10 unique entries; each must be a Pubky marketplace v1 URI owned by the drop's seller (same rule as listing media URLs). |
| `format`         | String   | How the drop sells.                          | Required. Only `"fcfs"` (first-come, first-served) in this version — the enum is closed-world, so any future format is a schema version bump. |
| `startsAt`       | String   | The seller's declared start.                 | Required. ISO-8601 with offset.                                                    |
| `endsAt`         | String   | The seller's declared end.                   | Optional. ISO-8601 with offset; when present must be strictly after `startsAt`. Absent means the drop ends only by sell-out or seller cancellation. Absent fields are not serialized. |
| `listingIds`     | Array    | The seller's OWN listings bundled into the drop. | Required. 1–20 unique path-safe entity IDs. The record owner is the listing owner by definition — no cross-owner reference form exists. |
| `totalQuantity`  | Integer  | Total units across the drop.                 | Required. Integer between 1 and 1000000.                                           |
| `perBuyerLimit`  | Integer  | Per-buyer purchase cap.                      | Required. Integer between 1 and 100; must not exceed `totalQuantity`.              |
| `stockDisplay`   | String   | Declared stock-visibility policy.            | Required. One of `"exact"`, `"bands"`, `"hidden"` — how much remaining-stock detail the seller wants the public projection to reveal. |

**Authority note:** `startsAt`/`endsAt` are the seller's declared schedule intent — the marketplace transaction service enforces the real sale window and the real stock counters. Likewise `stockDisplay` is the seller's declared policy; ENFORCEMENT is server-side (the service, which holds the counters, decides what each stock query answers).

---

### PubkyMarketReview

**Description:** Represents a marketplace review of a trade counterparty. All fields are serialized in camelCase and unknown members are ignored when parsing and are not written back.

**URI:** `/pub/pubky.app/marketplace/v1/reviews/:review_id`

| **Field**                | **Type** | **Description**                        | **Validation Rules**                                                             |
| ------------------------ | -------- | -------------------------------------- | --------------------------------------------------------------------------------- |
| `schemaVersion`          | Integer  | Marketplace contract version.          | Required. Must be `1`.                                                            |
| `recordType`             | String   | Record discriminator.                  | Required. Must be `"review"`.                                                     |
| `ownerPubky`             | String   | Pubky of the reviewer.                 | Required. 52-character z-base-32 pubky.                                           |
| `revision`               | Integer  | Record revision.                       | Required. Positive safe integer.                                                  |
| `createdAt`              | String   | Creation datetime.                     | Required. ISO-8601 with offset.                                                   |
| `updatedAt`              | String   | Last-update datetime.                  | Required. ISO-8601 with offset. Must not precede `createdAt`.                     |
| `reviewId`               | String   | Review identifier.                     | Required. Must equal the **Hash ID** in the record path.                          |
| `subjectPubky`           | String   | Pubky of the reviewed user.            | Required. 52-character z-base-32 pubky.                                           |
| `listingOwnerPubky`      | String   | Pubky of the reviewed listing's owner. | Required. 52-character z-base-32 pubky.                                           |
| `listingId`              | String   | Reviewed listing identifier.           | Required. Path-safe identifier, 1–128 characters.                                 |
| `role`                   | String   | Review direction.                      | Required. `buyer_reviewing_seller` or `seller_reviewing_buyer`.                   |
| `ratings`                | Object   | Star ratings.                          | Required. `overall` 1–5; optional `itemAccuracy`, `shipping`, `communication` 1–5. |
| `text`                   | String   | Review text.                           | Optional; a missing member reads as `""`. Trimmed. Length: 0–5000 characters.     |
| `eligibilityAttestation` | String   | Proof of review eligibility.           | Required. 32–4096 characters of `[A-Za-z0-9._~-]`.                                |
| `orderRef`               | String   | The attested order's `order_ref`.      | Optional. Lowercase 64-character BLAKE3 hex hash. Must equal the attestation's `order_ref` claim. |

**Validation Notes:**

- The `review_id` is a **Hash ID** derived from `"{listing_uri}:{subject_pubky}:{role}:{orderRef}"`, where `listing_uri` is `pubky://<listingOwnerPubky>/pub/pubky.app/marketplace/v1/listings/<listingId>`. Each attested order therefore gets its own review, and a second order of the same listing never overwrites the first.
- A record without `orderRef` (written before per-order reviews) keeps the three-part input `"{listing_uri}:{subject_pubky}:{role}"` and stays valid.
- **Item counting key.** A verified review counts once per attested order, role and reviewed listing: `item_count_key = (iss, order_ref, role, listingOwnerPubky, listingId)`, with `iss` and `order_ref` taken from the attestation. Items of one order therefore count separately. When a record with `orderRef` and one without it carry the same key, the record with `orderRef` counts; the other is left out of counts, averages and review lists. Implementations can use `PubkyMarketReview::item_count_key()` (wasm `itemCountKey`) and `PubkyMarketReview::is_order_scoped()`.
- **Seller rating key.** `ratings.shipping` and `ratings.communication` rate the seller for the whole order. They are written identically on every item review of the order and count once per `seller_rating_key = (iss, order_ref, role)` (`PubkyMarketReview::seller_rating_key()`, wasm `sellerRatingKey`). When the copies differ, the record with the latest `updatedAt` wins; ties go to the lowest `reviewId`. Absent fields are skipped in averages.
- **Key strings.** The `Display` form of both keys (and the wasm return value) joins the parts with `:`; no part can contain one.
- **Price weight.** A counted review's weight comes from its attestation's `price_decade` claim `"{CURRENCY}:{k}"`: `w = price_weight(price_decade) = clamp(k + offset(CURRENCY) − 1, 1, 5)`. The static table fixes each currency's minor unit and its decade offset to a US cent: `USD`, `EUR`, `GBP`, `CHF`, `CAD`, `AUD` (cents) 0; `JPY` (yen) 0; `BTC` (satoshis, exponent 8) −1; `SAT` (satoshis, exponent 0, the `amount_band` code) −1. An attestation without the claim, or with any other currency, weighs 1. So under $10 weighs 1, $10–99.99 weighs 2 and $10,000 or more weighs 5. Exposed as `price_weight()`, `price_decade()` (the issuer's encoding), `PubkyMarketPurchaseAttestation::price_weight()` and `PRICE_WEIGHT_CURRENCIES` (wasm `priceWeight`, `priceDecade`); shared vectors in `src/test_fixtures/price-weight-vectors.json`.

---

### Purchase Attestation (embedded JWS)

**Description:** The normative format of the value carried in a review record's `eligibilityAttestation` when the review is attested by a marketplace transaction authority. It is a compact JWS (RFC 7515) signed with EdDSA/Ed25519 (RFC 8037): `base64url(header).base64url(claims).base64url(signature)`, unpadded. The attestation attests the **purchase**, not the review text: record revisions leave it unchanged, and it carries no expiry.

**Header (closed-world, exactly these fields):**

```json
{ "alg": "EdDSA", "typ": "pubky-purchase-attestation+v1" }
```

**Claims (version `v: 1`, closed-world — unknown claims are rejected):**

| **Claim**      | **Type** | **Description**                             | **Validation Rules**                                                                  |
| -------------- | -------- | ------------------------------------------- | ------------------------------------------------------------------------------------- |
| `v`            | Integer  | Claim-set version.                          | Required. Must be `1`.                                                                 |
| `iss`          | String   | Attestor pubky.                             | Required. 52-character z-base-32 pubky; decodes to the Ed25519 verification key.       |
| `sub`          | String   | Reviewer pubky.                             | Required. Must equal the review record's `ownerPubky`.                                 |
| `cpk`          | String   | Counterparty pubky.                         | Required. Must equal the review record's `subjectPubky`.                               |
| `role`         | String   | Review direction.                           | Required. `buyer_reviewing_seller` or `seller_reviewing_buyer`; must match the record. |
| `listing`      | String   | Canonical listing URI.                      | Required. Must match the record's `listingOwnerPubky` + `listingId`.                   |
| `order_ref`    | String   | Attestor-salted Blake3 of the order UUID.   | Required. 64 lowercase hex characters. Opaque; only the attestor can link it back.     |
| `completed_on` | String   | Order completion date.                      | Required. `YYYY-MM-DD` (day granularity, deliberately no finer).                       |
| `amount_band`  | String   | Log-decade amount band.                     | Optional. `{CURRENCY}:{magnitude}`, e.g. `SAT:5`; magnitude 0–18. Emitted only under both-sides consent (seller standing preference AND per-review buyer opt-in). |
| `price_decade` | String   | Log decade of the line's unit price paid.  | Optional. `{CURRENCY}:{magnitude}` with the listing's own currency code and magnitude `floor(log10(unit price))` counted in the price weight table's minor unit for that currency (cents; yen for `JPY`; satoshis for `BTC`, whatever exponent the listing uses), e.g. `USD:3`, `BTC:4`; magnitude 0–18. Issued on every per-item attestation from `.15`; it discloses only the price's order of magnitude, which the public listing price already shows. Absent on earlier attestations. |
| `iat`          | Integer  | Issuance time (UNIX seconds).               | Required. Positive safe integer.                                                       |

**Verification recipe (offline, no issuer round-trip):**

1. Parse the compact JWS; reject unknown header fields, unknown claims, and unknown versions.
2. Decode `iss` from z-base-32 — that *is* the Ed25519 verification key. Verify the signature over `base64url(header) || '.' || base64url(claims)`.
3. Check bindings against the review record: `sub == ownerPubky`, `cpk == subjectPubky`, `listing` matches, `role` matches, and `order_ref == orderRef` when the record carries `orderRef`.
4. Accept as **verified** only if `iss` is on your own attestor trust list. The signature proves key possession, never legitimacy.

Exact amounts, timestamps finer than a day, addresses, payment identifiers, and bearer material are prohibited in claims.

---

### PubkyMarketAttestorProfile and PubkyMarketAttestorAnnotation

**Description:** Records a purchase attestor publishes on its **own** homeserver, owned by the attestor pubky: the profile says who operates the key, which keys it has issued with and how it issues; one annotation per attested order records refund and return outcomes. Both carry a `proof` so cached copies verify offline. Unknown members are tolerated and covered by the proof.

**URIs:** `/pub/pubky.app/marketplace/v1/attestor/profile` and `/pub/pubky.app/marketplace/v1/attestor/annotations/:order_ref`

**Profile:**

| **Field**                         | **Type** | **Description**                                   | **Validation Rules**                                                              |
| --------------------------------- | -------- | ------------------------------------------------- | --------------------------------------------------------------------------------- |
| `operator.name`                   | String   | Operator name, e.g. `"Pubky Market"`.             | Required. 1–100 characters, no surrounding whitespace.                            |
| `operator.url`                    | String   | Operator website.                                 | Required. https URL, at most 2048 characters.                                     |
| `operator.policyUrl`              | String   | Issuance policy page.                             | Required. https URL, at most 2048 characters.                                     |
| `keys`                            | Array    | Keys the attestor has issued with.                | Required. 1–16 entries, no repeated `pubky`.                                      |
| `keys[].pubky`                    | String   | Key pubky.                                        | Required. 52-character z-base-32 pubky.                                           |
| `keys[].validFrom`                | String   | When the key started issuing.                     | Required. ISO-8601 datetime with offset.                                          |
| `keys[].validUntil`               | String   | When the key stopped issuing.                     | Optional. After `validFrom`.                                                      |
| `keys[].compromisedFrom`          | String   | From when the key's attestations are untrusted.   | Optional. Not before `validFrom`.                                                 |
| `successor`                       | String   | Attestor that takes over.                         | Optional. 52-character z-base-32 pubky.                                           |
| `issuancePolicy.editWindowHours`  | Integer  | Hours in which items may be added to a review.    | Required. 0–8760.                                                                 |
| `issuancePolicy.blindWindowDays`  | Integer  | Days a double-blind review stays sealed.          | Required. 0–365.                                                                  |
| `issuancePolicy.amountBand`       | String   | When `amount_band` is issued.                     | Required. 1–64 characters, e.g. `"both-sides consent"`.                           |
| `issuancePolicy.priceDecade`      | String   | When `price_decade` is issued.                    | Required. 1–64 characters, e.g. `"always"`.                                       |
| `issuancePolicy.minOrderValue`    | Object   | Smallest attested order total.                    | Required, may be `null`. A positive money object.                                 |
| `proof`                           | String   | Proof over the canonical body.                    | Required. `typ` `pubky-attestor-profile+v1`.                                      |

**Annotation:**

| **Field**         | **Type** | **Description**                               | **Validation Rules**                                                       |
| ----------------- | -------- | --------------------------------------------- | -------------------------------------------------------------------------- |
| `orderRef`        | String   | The attested order's `order_ref`.             | Required. Lowercase 64-character hex hash; must equal the path ID.         |
| `outcomes`        | Array    | What happened to the order.                   | Required. 0–16 entries `{kind, on}`: `kind` `refunded` or `returned`, `on` a `YYYY-MM-DD` date. |
| `revision`        | Integer  | Record revision; a higher one replaces.       | Required. Positive safe integer.                                           |
| `proof`           | String   | Proof over the canonical body.                | Required. `typ` `pubky-attestor-annotation+v1`.                            |

**Proof:** a compact JWS (RFC 7515) with a detached payload (Appendix F), signed with EdDSA/Ed25519 (RFC 8037): `base64url(header) || ".." || base64url(signature)`, unpadded. The header is exactly `{"alg":"EdDSA","typ":<typ>}`. The signature covers `base64url(header) || "." || base64url(canonical body)`, where the canonical body is the record's JSON object without `proof`, serialized per RFC 8785 (members sorted by UTF-16 code units, no whitespace; the records hold integers only).

**Verification recipe (offline):**

1. Parse the URI; it must be the profile path, or an annotation path whose ID equals the record's `orderRef`.
2. Check the header `alg` and `typ` against the path, decode the URI's owner pubky as the Ed25519 key, and verify the signature over the canonical body.
3. Validate the record.
4. A verifier may only **narrow** its own trust from a profile: a key's effective end is the earlier of the configured `validUntil` and the profile's `compromisedFrom`. It never adds keys or follows `successor` from a profile, since whoever holds a stolen key can rewrite it.

`PubkyMarketAttestorProfile::from_verified_json(uri, blob)` and `PubkyMarketAttestorAnnotation::from_verified_json(uri, blob)` run the recipe (`PubkyMarketObject::from_uri` calls them; wasm `verifyAttestorProfile`, `verifyAttestorAnnotation`); `sign(secret_key)` and `to_canonical_json()` serve the publisher. `src/test_fixtures/attestor-records-v1.json` holds a canonical vector for each record.

---

### PubkyMarketReviewResponse

**Description:** Represents the review subject's single revisable response to a marketplace review, published on the **responder's** homeserver. All fields are serialized in camelCase and unknown members are ignored when parsing and are not written back.

**URI:** `/pub/pubky.app/marketplace/v1/review_responses/:review_id`

| **Field**       | **Type** | **Description**                          | **Validation Rules**                                                              |
| --------------- | -------- | ---------------------------------------- | ---------------------------------------------------------------------------------- |
| `schemaVersion` | Integer  | Marketplace contract version.            | Required. Must be `1`.                                                             |
| `recordType`    | String   | Record discriminator.                    | Required. Must be `"review_response"`.                                             |
| `ownerPubky`    | String   | Pubky of the responder.                  | Required. 52-character z-base-32 pubky.                                            |
| `revision`      | Integer  | Record revision.                         | Required. Positive safe integer.                                                   |
| `createdAt`     | String   | Creation datetime.                       | Required. ISO-8601 with offset.                                                    |
| `updatedAt`     | String   | Last-update datetime.                    | Required. ISO-8601 with offset. Must not precede `createdAt`.                      |
| `reviewId`      | String   | Subject review's identifier.             | Required. Must equal the ID in the record path.                                    |
| `reviewUri`     | String   | Canonical URI of the subject review.     | Required. Must reference the same `reviewId`; must not be on the responder's own homeserver. |
| `text`          | String   | Response text.                           | Required. Trimmed. Length: 1–5000 characters.                                      |

**Validation Notes:**

- The path ID **equals the subject review's ID** — one response per review, revisable via `revision`.
- **Authorization is structural, not cryptographic:** indexers accept a response only when the response record's `ownerPubky` equals the subject review's `subjectPubky`. An impostor's response fails that check without any signature machinery. No attestation is carried.

---

### PubkyMarketWatchlist (private)

**Description:** The user's private marketplace watchlist — the first record under `/priv/`, the homeserver's authenticated private storage. A watchlist reveals purchase intent, so unlike every `/pub/pubky.app/` record it must not be world-readable, directory-listable, or indexable; the homeserver refuses reads, listings, and writes on `/priv/` paths from anyone but the owner's own sessions. This record is deliberately **not** wired into `PubkyMarketObject` or the URI parser's resource resolution: watchers and indexers never see it. Sessions need the `/priv/pubky.app/:rw` capability to touch it.

**URI:** `/priv/pubky.app/marketplace/v1/watchlist.json`

It is a **single revisioned document** (singleton per user, like `shop.json`) rather than one record per watched listing because: (a) watch/unwatch toggles are high-churn, and a single document makes each sync one `PUT` instead of a create/delete stream; (b) merge needs items and tombstones resolved together atomically — two files could tear; and (c) private storage has no index that would benefit from per-item paths.

| **Field**       | **Type** | **Description**                            | **Validation Rules**                                                              |
| --------------- | -------- | ------------------------------------------ | ---------------------------------------------------------------------------------- |
| `schemaVersion` | Integer  | Marketplace contract version.              | Required. Must be `1`.                                                             |
| `recordType`    | String   | Record discriminator.                      | Required. Must be `"watchlist"`.                                                   |
| `ownerPubky`    | String   | Pubky of the watchlist owner.              | Required. 52-character z-base-32 pubky.                                            |
| `revision`      | Integer  | Document revision.                         | Required. Positive safe integer.                                                   |
| `createdAt`     | String   | Creation datetime.                         | Required. ISO-8601 with offset.                                                    |
| `updatedAt`     | String   | Last-update datetime.                      | Required. ISO-8601 with offset. Must not precede `createdAt`.                      |
| `items`         | Array    | Actively watched listings.                 | Required (may be empty). At most 500 entries.                                      |
| `tombstones`    | Array    | Removed watches retained for merge.        | Required (may be empty). At most 500 entries; clients prune oldest-first.          |

`items[]` entries:

| **Field**           | **Type** | **Validation Rules**                                              |
| ------------------- | -------- | ----------------------------------------------------------------- |
| `listingOwnerPubky` | String   | Required. 52-character z-base-32 pubky of the seller.             |
| `listingId`         | String   | Required. Path-safe entity ID (1–128 chars of `[A-Za-z0-9_-]`).   |
| `watchedAtMs`       | Integer  | Required. Positive safe integer of epoch **milliseconds**.        |

`tombstones[]` entries carry the same `listingOwnerPubky` / `listingId` plus `removedAtMs` under the same rules.

**Validation Notes:**

- **Key uniqueness:** every `(listingOwnerPubky, listingId)` key appears at most once across `items` **and** `tombstones` combined. The document is the post-merge resolved state: a listing is either watched or removed, never both.
- **Entry timestamps are integer milliseconds** (not ISO-8601 like the document-level datetimes) on purpose: they are last-write-wins merge keys that clients compare numerically, immune to offset-formatting differences between writers.
- **Merge rule (normative for clients):** per listing key, the entry with the greater timestamp wins (`watchedAtMs` vs `removedAtMs`); ties resolve to the tombstone (deletion wins). The merged document is written back with `revision` incremented.
- **Encrypted storage:** a client holding the owner's data key stores this document sealed at the `watchlist` entry described in [Encrypted Private Records](#encrypted-private-records-pubky-priv-aeadv1). It moves any plaintext `watchlist.json` into that entry, merged, and deletes the plaintext only after the sealed write reads back.

---

### PubkyMarketOrderReceipt (private)

**Description:** A PRIVATE portable order receipt — the buyer's or seller's own durable copy of a completed order, written to their OWN homeserver. The marketplace transaction service holds the canonical order state, but a service is an operator that can disappear; this record is the **credible exit for orders**: each trade party keeps a signed, self-contained receipt (the embedded `receiptAttestation` JWS is offline-verifiable) on storage they control, so a purchase history survives the operator. All fields are serialized in camelCase and unknown members are ignored when parsing and are not written back.

**URI:** `/priv/pubky.app/marketplace/v1/receipts/:receipt_id`

**Encrypted storage:** a client holding the owner's data key stores each receipt sealed at its `order_receipt` entry (id = `receiptId`), described in [Encrypted Private Records](#encrypted-private-records-pubky-priv-aeadv1). The v1 path above is where pre-encryption clients wrote it, and where encrypting clients move it from.

**Privacy rationale:** like the watchlist, this is a `/priv/` record — an order history reveals counterparties, amounts, and purchase timing, so it must never be world-readable, directory-listable, or indexable; the homeserver refuses reads, listings, and writes on `/priv/` paths from anyone but the owner's own sessions. It is deliberately **not** wired into `PubkyMarketObject` or the URI parser's resource resolution: watchers and indexers never see it. Unlike the watchlist singleton, receipts are one record per order under `receipts/:receipt_id` (the transaction service's receipt UUID, lowercase hyphenated): receipts are immutable facts, not merge targets, and per-id paths let a client sync incrementally instead of rewriting one growing document.

| **Field**            | **Type** | **Description**                              | **Validation Rules**                                                              |
| -------------------- | -------- | -------------------------------------------- | ---------------------------------------------------------------------------------- |
| `schemaVersion`      | Integer  | Marketplace contract version.                | Required. Must be `1`.                                                             |
| `recordType`         | String   | Record discriminator.                        | Required. Must be `"order_receipt"`.                                               |
| `ownerPubky`         | String   | Pubky of the record owner (a trade party).   | Required. 52-character z-base-32 pubky. Must equal `buyerPubky` when `role` is `"buyer"` and `sellerPubky` when `role` is `"seller"`. |
| `revision`           | Integer  | Record revision.                             | Required. Positive safe integer.                                                   |
| `createdAt`          | String   | Creation datetime.                           | Required. ISO-8601 with offset (`Z` or `±HH:MM`).                                  |
| `updatedAt`          | String   | Last-update datetime.                        | Required. ISO-8601 with offset. Must not precede `createdAt`.                      |
| `role`               | String   | The record owner's side of the order.        | Required. `"buyer"` or `"seller"`.                                                 |
| `receiptId`          | String   | The service's receipt UUID.                  | Required. Lowercase hyphenated UUID (8-4-4-4-12). Must equal the id in the record path. |
| `orderId`            | String   | The order UUID the receipt settles.          | Required. Lowercase hyphenated UUID (8-4-4-4-12).                                  |
| `buyerPubky`         | String   | Pubky of the buyer.                          | Required. 52-character z-base-32 pubky. Must differ from `sellerPubky`.            |
| `sellerPubky`        | String   | Pubky of the seller.                         | Required. 52-character z-base-32 pubky.                                            |
| `total`              | Object   | Order total in integer minor units.          | Required. Money object (`amountMinor`, `currency`, `exponent`); positive amount, uppercase 3–12 character asset code, exponent 0–18. |
| `paidAt`             | String   | Payment confirmation / receipt creation.     | Required. ISO-8601 with offset.                                                    |
| `receiptAttestation` | String   | Compact JWS attesting the receipt.           | Required. 32–4096 characters of `[A-Za-z0-9._~-]` (same bounds and charset as the review record's `eligibilityAttestation`). |
| `editionAttestation` | String   | Compact JWS attesting the drop edition (see [Drop Edition Attestation](#drop-edition-attestation-embedded-jws)). | Optional. Same bounds and charset as `receiptAttestation`. Must be present exactly when `drop` is present. Absent fields are not serialized (pre-drop receipts round-trip unchanged). |
| `drop`               | Object   | Drop display object: `{ dropId, edition, of }`. | Optional. `dropId` path-safe entity ID; `edition` positive integer; `of` positive integer, never below `edition`. Must be present exactly when `editionAttestation` is present. Absent fields are not serialized. |

---

### Order Receipt Attestation (embedded JWS)

**Description:** The normative format of the value carried in an order receipt record's `receiptAttestation`. It is a compact JWS (RFC 7515) signed with EdDSA/Ed25519 (RFC 8037): `base64url(header).base64url(claims).base64url(signature)`, unpadded.

**Header (closed-world, exactly these fields):**

```json
{ "alg": "EdDSA", "typ": "pubky-order-receipt+v1" }
```

**Claims (version `v: 1`, closed-world — unknown claims are rejected; issuers serialize claims in exactly this order):**

| **Claim**     | **Type** | **Description**                             | **Validation Rules**                                                                  |
| ------------- | -------- | ------------------------------------------- | -------------------------------------------------------------------------------------- |
| `v`           | Integer  | Claim-set version.                          | Required. Must be `1`.                                                                 |
| `iss`         | String   | Attestor pubky.                             | Required. 52-character z-base-32 pubky; decodes to the Ed25519 verification key.       |
| `buyer`       | String   | Buyer pubky.                                | Required. Must equal the receipt record's `buyerPubky`.                                |
| `seller`      | String   | Seller pubky.                               | Required. Must equal the receipt record's `sellerPubky`.                               |
| `order`       | String   | Raw order UUID.                             | Required. Lowercase hyphenated UUID; must equal the record's `orderId`.                |
| `receipt`     | String   | Receipt UUID.                               | Required. Lowercase hyphenated UUID; must equal the record's `receiptId`.              |
| `total_minor` | Integer  | Order total in integer minor units.         | Required. Positive safe integer; must equal the record total's `amountMinor`.          |
| `currency`    | String   | Asset code.                                 | Required. Same rules as money `currency`; must equal the record total's `currency`.    |
| `exponent`    | Integer  | Minor/major decimal places.                 | Required. 0–18; must equal the record total's `exponent`.                              |
| `paid_at`     | String   | Exact instant of payment confirmation.      | Required. ISO-8601 **UTC** (`Z` offset); must equal the record's `paidAt`.             |
| `iat`         | Integer  | Issuance time (UNIX seconds).               | Required. Must equal the epoch seconds of the `paid_at` instant.                       |

**Why the raw order id (unlike the public purchase attestation):** the purchase attestation travels inside a world-readable review, so its `order_ref` is an attestor-salted hash that nobody but the attestor can link back to an order. Receipts are private documents under `/priv/` that only the trade parties hold — there is no third-party observer to protect the linkage from, and the raw `order` UUID is exactly what makes the receipt actionable against the service (disputes, exports, audits) after the operator disappears.

**Determinism:** issuance is deterministic per receipt. The claims serialize in a fixed field order, `paid_at` is the canonical UTC serialization of the payment instant, `iat` is derived from that same instant (never the signing wall clock), and Ed25519 signatures are deterministic — so a given receipt always yields the same compact JWS, byte for byte.

**Verification recipe (offline, no issuer round-trip):**

1. Parse the compact JWS; reject unknown header fields, unknown claims, and unknown versions.
2. Decode `iss` from z-base-32 — that *is* the Ed25519 verification key. Verify the signature over `base64url(header) || '.' || base64url(claims)`.
3. Check bindings against the receipt record: `buyer == buyerPubky`, `seller == sellerPubky`, `order == orderId`, `receipt == receiptId`, `total_minor`/`currency`/`exponent` equal the record's `total`, and `paid_at == paidAt`.
4. Accept as **verified** only if `iss` is on your own attestor trust list. The signature proves key possession, never legitimacy.

---

### Drop Edition Attestation (embedded JWS)

**Description:** The normative format of the value carried in an order receipt record's `editionAttestation`. It attests that one order bought edition `edition` out of `of` total units of the seller's drop. It is a compact JWS (RFC 7515) signed with EdDSA/Ed25519 (RFC 8037): `base64url(header).base64url(claims).base64url(signature)`, unpadded.

**Header (closed-world, exactly these fields):**

```json
{ "alg": "EdDSA", "typ": "pubky-drop-edition+v1" }
```

**Claims (version `v: 1`, closed-world — unknown claims are rejected; issuers serialize claims in exactly this order):**

| **Claim**  | **Type** | **Description**                              | **Validation Rules**                                                                  |
| ---------- | -------- | -------------------------------------------- | -------------------------------------------------------------------------------------- |
| `v`        | Integer  | Claim-set version.                           | Required. Must be `1`.                                                                 |
| `iss`      | String   | Attestor pubky.                              | Required. 52-character z-base-32 pubky; decodes to the Ed25519 verification key.       |
| `buyer`    | String   | Buyer pubky.                                 | Required. Must equal the receipt record's `buyerPubky`.                                |
| `seller`   | String   | Seller pubky (the drop owner).               | Required. Must equal the receipt record's `sellerPubky`.                               |
| `drop`     | String   | The drop's entity id.                        | Required. Path-safe entity ID; must equal the record's `drop.dropId`.                  |
| `edition`  | Integer  | This order's edition number, 1-based.        | Required. Positive safe integer; must equal the record's `drop.edition`.               |
| `of`       | Integer  | The drop's `totalQuantity` at issuance.      | Required. Positive safe integer, never below `edition`; must equal the record's `drop.of`. |
| `receipt`  | String   | Receipt UUID.                                | Required. Lowercase hyphenated UUID; must equal the record's `receiptId`.              |
| `iat`      | Integer  | Issuance time (UNIX seconds).                | Required. Positive safe integer. Deterministic per receipt — derived from the receipt's payment instant, never the signing wall clock (same doctrine as the receipt attestation). |

**Determinism:** issuance is deterministic per receipt. The claims serialize in a fixed field order, `iat` is derived from the receipt (never the signing wall clock), and Ed25519 signatures are deterministic — so a given receipt always yields the same compact JWS, byte for byte.

**Verification recipe (offline, no issuer round-trip):**

1. Parse the compact JWS; reject unknown header fields, unknown claims, and unknown versions.
2. Decode `iss` from z-base-32 — that *is* the Ed25519 verification key. Verify the signature over `base64url(header) || '.' || base64url(claims)`.
3. Check bindings against the receipt record (which must carry BOTH `editionAttestation` and the `drop` object): `receipt == receiptId`, `buyer == buyerPubky`, `seller == sellerPubky`, and `drop`/`edition`/`of` equal the record's `drop` object fields.
4. Accept as **verified** only if `iss` is on your own attestor trust list. The signature proves key possession, never legitimacy.

---

### Encrypted Private Records (`pubky-priv-aead/v1`)

**Description:** The sealed storage form of the private marketplace records ([watchlist](#pubkymarketwatchlist-private), [order receipts](#pubkymarketorderreceipt-private)) and the owner's [badge checkpoints](#badge-checkpoints). `/priv/` already refuses reads and listings to everyone but the owner's sessions, but the homeserver operator can still read every file. A client that holds the owner's data key seals each record before writing it, so the homeserver stores only ciphertext under names that reveal neither the record type nor its id. The plaintext is the unchanged record document; nothing in the record schemas changes.

**Data keys:** Each owner has one or more random 32-byte data keys. Each key is named by a key id: an opaque random string of 32 lowercase hex characters assigned when the key is created, carrying no meaning. A key is never derived from a session, grant or AuthToken. The owner's keys are ordered oldest first, and one of them is the **current** key. Keys are only ever appended: the first key is never retired or replaced, because every entry path derives from it. The marketplace transaction service holds the keys sealed at rest and releases them only to a session whose verified grant covers `/priv/pubky.app/` with read and write (`GET /v1/me/priv-keys`). Clients keep released keys in memory only.

**Subkeys:** For every data key:

```
record key = HKDF-SHA256(ikm = data key, salt = "pubky-priv-aead/v1", info = "record", length = 32)
path key   = HKDF-SHA256(ikm = data key, salt = "pubky-priv-aead/v1", info = "path",   length = 32)
```

**Families and entry names:**

| **Record**                | **family**                | **Entry name**                                                                 |
| ------------------------- | ------------------------- | ------------------------------------------------------------------------------ |
| Watchlist                 | `watchlist`               | derived from the id `watchlist`                                                |
| Order receipt             | `order_receipt`           | derived from the receipt id (lowercase hyphenated UUID)                        |
| Activity badge checkpoint | `attention_seen/activity` | listed: a fresh random name                                                    |
| Orders badge checkpoint   | `attention_seen/orders`   | listed: a fresh random name                                                    |

**Paths:** Paths use the path key of the owner's **first** (oldest) data key, so they do not move when a newer key becomes current.

```
family segment = base64url(HMAC-SHA256(path key, "family|" + family))
entry segment  = base64url(HMAC-SHA256(path key, "id|" + family + "|" + id))

entry path     = /priv/pubky.app/marketplace/v2/s/{family segment}/{entry name}
```

`base64url` is RFC 4648 §5 without padding. A **derived** entry is named by the entry segment of its id. A **listed** entry is named by a fresh random 32-character lowercase hex string. Readers find either kind by listing the family directory, and ignore any name that is neither 43 base64url characters (derived) nor 32 lowercase hex characters (listed).

**Envelope:** The stored document is a JSON object with exactly these fields:

| **Field** | **Type** | **Validation Rules**                                                                      |
| --------- | -------- | ----------------------------------------------------------------------------------------- |
| `enc`     | String   | Required. Must be `"pubky-priv-aead/v1"`.                                                 |
| `kid`     | String   | Required. The key id the record is sealed under; 32 lowercase hex characters.             |
| `nonce`   | String   | Required. base64url (unpadded) of a fresh random 24-byte XChaCha20-Poly1305 nonce.        |
| `ct`      | String   | Required. base64url (unpadded) of the XChaCha20-Poly1305 ciphertext with its 16-byte tag. |

Unknown fields are rejected.

**Associated data:** `{owner}|{family}|{entry name}|{kid}` as UTF-8, where `owner` is the owner's 52-character z-base-32 pubky and `entry name` is the last segment of the entry path. Every field is non-empty and contains no `|`. Binding the entry name rather than the record id lets a reader who holds only the keys list a family and open every entry; a ciphertext moved to another owner, family, entry or key id does not open.

**Sealing (writers):**

1. Serialize the record document as UTF-8 JSON.
2. Encrypt it with XChaCha20-Poly1305 under the **current** key's record key, a fresh random 24-byte nonce from a CSPRNG, and the associated data above. The 192-bit random nonce makes a repeat negligible; a (record key, nonce) pair is never reused.
3. Write the envelope to the entry path, then read it back. A write counts only when the stored envelope opens to the same document, compared as serialized JSON.

**Opening recipe (readers, offline):**

1. Reject anything that is not an envelope with exactly the fields above. `nonce` and `ct` must be unpadded base64url with no other characters, and `nonce` must decode to exactly 24 bytes.
2. Find the data key whose key id equals `kid`. An unknown `kid` is an error, never an empty record.
3. Decrypt `ct` with that key's record key, the envelope's `nonce`, and the associated data built from the name the entry was read from. Authentication failure means the entry was tampered with, moved to another owner, family or entry, or sealed under another key. Treat it as unreadable.
4. Parse the plaintext as the record document and validate it against its record schema. For a derived entry, recompute the entry segment of the record's own id (`watchlist`, or the receipt's `receiptId`) and reject the record unless it equals the entry name. For an order receipt, then run the [order receipt verification recipe](#order-receipt-attestation-embedded-jws).

**Writer obligations:**

- An entry that does not open, or opens to a record that fails step 4, is never overwritten or deleted, and never counts as the stored copy of any record.
- A writer without a data key writes nothing. It never falls back to a plaintext v1 path and never writes an empty document over a sealed one.
- The homeserver has no conditional write. A client serializes the read-merge-write of a whole-document entry (the watchlist) across all of its tabs and processes on one device; concurrent devices converge on their next round through the record's merge rule.
- **Moving plaintext v1 records.** A client that finds a plaintext record at its v1 path (`watchlist.json`, `receipts/:receipt_id`, `attention_seen/{side}/{ms}`) must, in this order:
  1. merge or carry it into the sealed entry;
  2. write the sealed entry and read it back, or, when a sealed entry already exists, confirm that it opens to exactly the carried record;
  3. only then delete the v1 file.
  A v1 file that does not parse, whose content does not match its path, or whose existing sealed entry differs from it, stays in place, and the sealed entry is not overwritten.
- Clients never log an entry path, a family segment, a key id, a key, a plaintext, or a v1 path that carries a receipt id or checkpoint time. Logs carry the redacted prefix (`/priv/pubky.app/marketplace/v2/s/<entry>`) and the error kind only.

#### Badge checkpoints

A badge checkpoint records when the owner last opened the Activity or Orders view, on any device. Its plaintext is:

```json
{ "version": 1, "seenAt": 1790000000000 }
```

`version` is `1`; `seenAt` is a positive integer of milliseconds since the Unix epoch. Each family holds immutable listed entries, and the checkpoint is the largest `seenAt` among the entries that open. A writer adds a new entry only when its value is larger than every entry it read, then deletes the entries it read below the one it wrote; an entry is deleted only when a larger one exists, so the maximum survives concurrent writers. The pre-encryption plaintext layout is `/priv/pubky.app/marketplace/v1/attention_seen/{side}/{ms}`, where `side` is `activity` or `orders` and `ms` is `seenAt` zero-padded to 13 digits; the file holds the same JSON. It is carried into a sealed entry when it holds the newest value and deleted once a sealed entry at least as new exists.

**Recovery key file:** Owners can export their data keys to read their records without the marketplace:

```json
{ "format": "pubky-priv-recovery-key/v1", "enc": "pubky-priv-aead/v1", "owner": "<pubky>", "currentKeyId": "<kid>", "keys": [{ "keyId": "<kid>", "key": "<base64url of 32 bytes>" }] }
```

`keys` is oldest first. With this file and the owner's homeserver data, the recipe above opens every entry: list each family directory and open each entry by its name. No receipt id is needed in advance; each opened receipt carries its `receiptId`, which step 4 checks against the entry name.

The file opens every private record of the owner; store it like a seed phrase. It carries no integrity protection, so load it only from trusted storage.

**What is not hidden:** the number of entries per family, their sizes, and their write times. Records are small documents; the envelope adds a 16-byte tag and the base64url overhead.

**Test vector** (inputs and outputs; readers must reproduce every value):

| **Item**                               | **Value**                                                                                                                                  |
| -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| data key (hex)                         | `000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f`                                                                         |
| key id                                 | `0123456789abcdef0123456789abcdef`                                                                                                         |
| owner                                  | `pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy`                                                                                     |
| family / id                            | `order_receipt` / `018f47d2-6a27-7c23-a49d-6b21bb770201`                                                                                   |
| record key (hex)                       | `dd32d572cb526afb8650d6a8ae6611e75b6db9a41d389a74280507b2f74c6ce2`                                                                         |
| path key (hex)                         | `5029c9cc9b46cf3489acb0ed560bb4a1337eb194d9479931e5182710eafa2b82`                                                                         |
| entry name                             | `drFSr_su7fkWn_6O7mSTHIekbFRfly7hIldHKsSX6ts`                                                                                              |
| path                                   | `/priv/pubky.app/marketplace/v2/s/XYERVz2efvcmNtICv_HgRsgOhhZ_BW5BkPaSRRUmQ_w/drFSr_su7fkWn_6O7mSTHIekbFRfly7hIldHKsSX6ts`                 |
| `attention_seen/orders` family segment | `InCtZ0EnT0foAzvK60hXONp9heYLHj31e4vNDAvuIXw`                                                                                              |
| associated data                        | `pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy\|order_receipt\|drFSr_su7fkWn_6O7mSTHIekbFRfly7hIldHKsSX6ts\|0123456789abcdef0123456789abcdef` |
| nonce (hex)                            | `404142434445464748494a4b4c4d4e4f5051525354555657`                                                                                         |
| plaintext                              | `{"schemaVersion":1,"recordType":"order_receipt"}`                                                                                         |

Envelope:

```json
{"enc": "pubky-priv-aead/v1", "kid": "0123456789abcdef0123456789abcdef", "nonce": "QEFCQ0RFRkdISUpLTE1OT1BRUlNUVVZX", "ct": "RO7jkZWdaLNwhZW-C4Mu6eNxKBGx_CyEPjVIkr024VXH630Zo5FC4sdbnOpsT8S9ZdsC0D0Sfn8bFU73690utQ"}
```

---

## Validation Rules

### Common Rules

1. **Timestamp IDs:** 13-character Crockford Base32 strings derived from timestamps (in microseconds).
2. **Hash IDs:** First half of the bytes from the resulting Blake3-hashed strings encoded in Crockford Base32.
3. **URLs:** All URLs must pass standard validation.

---

## License

MIT. See [LICENSE](LICENSE).
