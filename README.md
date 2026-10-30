# pubky-market-specs

Data formats for Pubky Market: shops, listings, drops, reviews, review responses, the private watchlist and order receipts, the purchase, order-receipt and drop-edition attestations, and the attestor records. One Rust crate validates, sanitizes, derives IDs and builds URIs; the same code compiles to WebAssembly for the npm package.

The normative description of every record is [SPEC.md](SPEC.md).

## Wire format

Records keep the paths and JSON of the marketplace records in `pubky-app-specs` `0.6.2-marketplace.16` ([BitcoinErrorLog/pubky-app-specs](https://github.com/BitcoinErrorLog/pubky-app-specs), tag `v0.6.2-marketplace.16`, rev `5ef6e31e`). For the same input both libraries accept or reject alike, and for accepted input they write the same bytes, derive the same IDs and verify the same attestations. Only Rust and TypeScript names change, to the `PubkyMarket` form.

| Record | Path | Rust type | npm builder |
|---|---|---|---|
| Shop | `/pub/pubky.app/marketplace/v1/shop.json` | `PubkyMarketShop` | `buildShop` |
| Listing | `/pub/pubky.app/marketplace/v1/listings/{timestamp id}` | `PubkyMarketListing` | `buildListing` |
| Drop | `/pub/pubky.app/marketplace/v1/drops/{drop id}` | `PubkyMarketDrop` | `buildDrop` |
| Review | `/pub/pubky.app/marketplace/v1/reviews/{hash id}` | `PubkyMarketReview` | `buildReview` |
| Review response | `/pub/pubky.app/marketplace/v1/review_responses/{review id}` | `PubkyMarketReviewResponse` | `buildReviewResponse` |
| Attestor profile | `/pub/pubky.app/marketplace/v1/attestor/profile` | `PubkyMarketAttestorProfile` | `verifyAttestorProfile` |
| Attestor annotation | `/pub/pubky.app/marketplace/v1/attestor/annotations/{order ref}` | `PubkyMarketAttestorAnnotation` | `verifyAttestorAnnotation` |
| Watchlist (private) | `/priv/pubky.app/marketplace/v1/watchlist.json` | `PubkyMarketWatchlist` | `buildWatchlist` |
| Order receipt (private) | `/priv/pubky.app/marketplace/v1/receipts/{receipt id}` | `PubkyMarketOrderReceipt` | `buildOrderReceipt` |

The embedded attestations are `PubkyMarketPurchaseAttestation` (in a review's `eligibilityAttestation`), `PubkyMarketOrderReceiptAttestation` v1 and v2 (in a receipt's `receiptAttestation`) and `PubkyMarketDropEditionAttestation` (in a receipt's `editionAttestation`). The encrypted storage form of private records (`pubky-priv-aead/v1`, under `/priv/pubky.app/marketplace/v2/`) is specified in [SPEC.md](SPEC.md#encrypted-private-records-pubky-priv-aeadv1).

Social records (profiles, posts, tags, follows, files) belong to the social specs and are not in this crate. A tag on a listing or shop is a social record whose target is the listing or shop URI built here.

## Rust

```toml
[dependencies]
pubky-market-specs = { git = "https://github.com/pubky/pubky-market-specs" }
```

```rust
use pubky_market_specs::{shop_uri_builder, ParsedUri, PubkyMarketObject, Resource};

let uri = shop_uri_builder(owner.to_string());
assert_eq!(ParsedUri::try_from(uri.as_str())?.resource, Resource::Shop);

match PubkyMarketObject::from_uri(&uri, blob)? {
    PubkyMarketObject::Shop(shop) => println!("{}", shop.name),
    other => println!("{other:?}"),
}
```

`PubkyMarketObject::from_uri` parses, sanitizes and validates the public records an indexer reads, and verifies attestor records against the URI's owner. `ParsedUri` recognizes the public Market paths and returns `Resource::Unknown` for any other `pubky.app` path; the variant names and `Display` strings of `Resource` are those of `pubky-app-specs`. Private records are never indexed and have no `Resource`.

The `openapi` feature derives `utoipa::ToSchema` for the record types.

## npm

`cargo run --bin bundle_specs_npm` builds the package into `pkg/` (it needs `wasm-pack` and node); `npm pack` in `pkg/` produces the tarball to depend on.

```js
import { buildReview, buildMarketUri, verifyPurchaseAttestation } from "pubky-market-specs";

const { review, meta } = buildReview(ownerPubky, reviewJson);
// write JSON.stringify(review.toJson()) to meta.path on the owner's homeserver

const listingUri = buildMarketUri(sellerPubky, "listing", listingId);
const claims = verifyPurchaseAttestation(reviewJson);
```

Every `build*` function takes the owner's pubky and the record as a camelCase object, sanitizes and validates it, derives its ID where it has one, and returns `{ <record>, meta }` with `meta.id`, `meta.path` and `meta.url`. The record fields are `shop`, `listing`, `drop`, `review`, `review_response`, `watchlist` and `order_receipt`. `buildMarketUri(owner, kind, id?)` covers every kind in the table above (`shop`, `listing`, `drop`, `review`, `review_response`, `watchlist`, `order_receipt`, `attestor_profile`, `attestor_annotation`) and concatenates exactly as the per-kind builders of `pubky-app-specs` do: singletons ignore `id`, the other kinds require one, and neither the owner nor the id is validated. `parseMarketUri(uri)` returns `{ user_id, resource, resource_id }` as `parse_uri` does for Market records, and `unknown` for any other `pubky.app` path. The package also exports `parsePurchaseAttestation`, `verifyPurchaseAttestation`, `itemCountKey`, `sellerRatingKey`, `priceDecade`, `priceWeight`, `parseOrderReceiptAttestation`, `verifyOrderReceiptAttestation`, `parseDropEditionAttestation`, `verifyDropEditionAttestation`, `verifyAttestorProfile` and `verifyAttestorAnnotation`.

## Names in pubky-app-specs

| pubky-app-specs `0.6.2-marketplace.16` | pubky-market-specs |
|---|---|
| `PubkyAppShop`, `PubkyAppListing`, `PubkyAppReviewResponse`, `PubkyAppWatchlist`, … | `PubkyMarketShop`, `PubkyMarketListing`, `PubkyMarketReviewResponse`, `PubkyMarketWatchlist`, … |
| `PubkyAppMarketplaceDrop`, `PubkyAppMarketplaceReview`, `PubkyAppMarketplaceOrderReceipt`, `PubkyAppMarketplaceLocation` | `PubkyMarketDrop`, `PubkyMarketReview`, `PubkyMarketOrderReceipt`, `PubkyMarketLocation` |
| `PubkyAppObject` (social and commerce) | `PubkyMarketObject` (commerce only) |
| `ParsedUri`, `Resource`, `PubkyId`, `*_uri_builder`, `traits::*`, constants | same names |
| `new PubkySpecsBuilder(owner).createShop(json)` and the other `create*` methods | `buildShop(owner, json)`, `buildListing`, `buildReview` (was `createMarketplaceReview`), `buildReviewResponse`, `buildWatchlist`, `buildOrderReceipt` (was `createMarketplaceOrderReceipt`), `buildDrop` (was `createMarketplaceDrop`) |
| result fields `marketplace_review`, `marketplace_drop` | `review`, `drop` |
| `listingUriBuilder(owner, id)` and the other per-kind JS URI builders | `buildMarketUri(owner, "listing", id)` |
| `parse_uri(uri)` | `parseMarketUri(uri)` (Market records only) |

## Parity with pubky-app-specs

[`parity/`](parity/) builds both libraries side by side and compares them on 18 seed records covering every kind, every structural mutation of every seed (other types, boundary numbers, edge-case strings, removed and unknown members), attestations re-signed after each claim or header mutation, attestor records re-signed after mutation, 22 forgeries (signatures with a flipped bit, signatures and attestor proofs by another key) that `pubky-app-specs` must reject, generated listing ids decoded against the clock, 1,582 URIs, 3,852 public keys, the price decade and weight tables, the constants and the paths. Any difference fails the check.

```sh
cd parity && cargo run --release -- check
```

It also writes [`test-vectors/market-records-v1.json`](test-vectors/market-records-v1.json): the seeds and a sample of the mutations with the exact outcome `pubky-app-specs` produced for each. `cargo test` replays the vectors against this crate alone, and the npm tests replay the seeds through the WebAssembly package. [`parity/js/parity.mjs`](parity/js/parity.mjs) compares the two npm packages call by call over the whole mutation corpus, the forgeries, the URI builders and parser, and the generated listing ids.

## Building and testing

```sh
cargo test                                  # unit tests, vectors, doc tests
cargo clippy --all-targets --all-features -- -D warnings
cargo check --target wasm32-unknown-unknown
cargo run --bin bundle_specs_npm            # npm package into pkg/ (needs wasm-pack and node)
cd pkg && npm install && npm test
```

Build the npm package from an APFS, ext4 or other native volume; AppleDouble (`._*`) files on exFAT break `wasm-pack`.

## License

MIT. See [LICENSE](LICENSE).
