# pubky-market-specs

Data formats for Pubky Market: shops, listings, drops, reviews, review responses, the private watchlist and order receipts, and their attestations. Built from the Rust crate of the same name with WebAssembly; synchronous, no initialization step.

Records keep the paths and JSON of the marketplace records in `pubky-app-specs` `0.6.2-marketplace.16`.

```js
import { buildReview, buildMarketUri, verifyPurchaseAttestation } from "pubky-market-specs";

const { review, meta } = buildReview(ownerPubky, reviewJson);
// meta.id, meta.path, meta.url; review.toJson() is the record to write
const listingUri = buildMarketUri(sellerPubky, "listing", listingId);
const claims = verifyPurchaseAttestation(reviewJson);
```

- `buildShop`, `buildListing`, `buildReview`, `buildReviewResponse`, `buildWatchlist`, `buildOrderReceipt`, `buildDrop`: `(ownerPubky, record) -> { <record>, meta }`. Each sanitizes and validates the record and derives its ID where it has one.
- `buildMarketUri(ownerPubky, kind, id?)` for `shop`, `listing`, `drop`, `review`, `review_response`, `watchlist`, `order_receipt`, `attestor_profile`, `attestor_annotation`; concatenates without validating, as the `pubky-app-specs` builders do.
- `parseMarketUri(uri)`: `{ user_id, resource, resource_id }` for Market records, resource `unknown` for other `pubky.app` paths.
- Attestations: `parsePurchaseAttestation`, `verifyPurchaseAttestation`, `itemCountKey`, `sellerRatingKey`, `priceDecade`, `priceWeight`, `parseOrderReceiptAttestation`, `verifyOrderReceiptAttestation`, `parseDropEditionAttestation`, `verifyDropEditionAttestation`.
- Attestor records: `verifyAttestorProfile(uri, json)`, `verifyAttestorAnnotation(uri, json)`.
- Record classes with `fromJson` / `toJson`: `PubkyMarketShop`, `PubkyMarketListing`, `PubkyMarketDrop`, `PubkyMarketReview`, `PubkyMarketReviewResponse`, `PubkyMarketWatchlist`, `PubkyMarketOrderReceipt`.

Specification: <https://github.com/pubky/pubky-market-specs/blob/main/SPEC.md>

MIT licensed.
