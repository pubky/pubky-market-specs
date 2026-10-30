// JS parity: the pubky-market-specs npm package against the pubky-app-specs
// marketplace package (0.6.2-marketplace.16) on every record case of the
// Rust parity corpus, the URI builders and the price functions.
//
// usage: node parity.mjs <pubky-app-specs package dir> <pubky-market-specs package dir> <corpus.json>

import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
import path from "node:path";

const [forkDir, oursDir, corpusPath] = process.argv.slice(2);
if (!forkDir || !oursDir || !corpusPath) {
  console.error("usage: node parity.mjs <fork pkg dir> <ours pkg dir> <corpus.json>");
  process.exit(2);
}
const load = (dir) => import(pathToFileURL(path.resolve(dir, "index.js")).href);
const fork = await load(forkDir);
const ours = await load(oursDir);
const { cases, uris } = JSON.parse(readFileSync(corpusPath, "utf8"));

// A lossless description of a value as the JS caller sees it: `Map`s,
// `undefined`, BigInts and key order all show up, so two values describe the
// same only if a caller cannot tell them apart.
const describe = (value) =>
  JSON.stringify(value, (_key, v) => {
    if (v === undefined) return "<undefined>";
    if (typeof v === "bigint") return `<bigint ${v}>`;
    if (v instanceof Map) return { "<map>": [...v.entries()] };
    return v;
  }) ?? "<undefined>";

const attempt = (fn) => {
  try {
    return `ok:${describe(fn())}`;
  } catch {
    return "err";
  }
};

const record = (result, field) => {
  const json = result[field].toJson();
  const meta = result.meta;
  return { json, meta: { id: meta.id, path: meta.path, url: meta.url } };
};

const CROCKFORD = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
// A listing id is the 8-byte big-endian microsecond timestamp in Crockford
// Base32 (13 characters, the last carrying one bit of data).
const decodeTimestampId = (id) => {
  let bits = 0n;
  for (const c of id) {
    const v = CROCKFORD.indexOf(c);
    if (v < 0) return null;
    bits = (bits << 5n) | BigInt(v);
  }
  return id.length === 13 ? bits >> 1n : null;
};

// The generated id is replaced by whether it encodes the build time, so the
// comparison covers the derivation without depending on the clock.
const withoutListingId = (built, before, after) => {
  const { json, meta } = built;
  const micros = decodeTimestampId(meta.id);
  return {
    json: { ...json, listingId: "<generated>" },
    meta: { id: "<generated>", path: "<generated>", url: "<generated>" },
    encodesBuildTime: micros !== null && micros >= before && micros <= after,
    pathMatches:
      meta.path === `/pub/pubky.app/marketplace/v1/listings/${meta.id}` &&
      meta.url.endsWith(meta.path) &&
      json.listingId === meta.id,
  };
};
const nowMicros = () => BigInt(Date.now()) * 1000n;
const timed = (build) => {
  const before = nowMicros();
  const built = build();
  const after = nowMicros() + 1000n;
  return withoutListingId(built, before, after);
};

const classes = {
  shop: ["PubkyAppShop", "PubkyMarketShop"],
  listing: ["PubkyAppListing", "PubkyMarketListing"],
  drop: ["PubkyAppMarketplaceDrop", "PubkyMarketDrop"],
  review: ["PubkyAppMarketplaceReview", "PubkyMarketReview"],
  review_response: ["PubkyAppReviewResponse", "PubkyMarketReviewResponse"],
  watchlist: ["PubkyAppWatchlist", "PubkyMarketWatchlist"],
  order_receipt: ["PubkyAppMarketplaceOrderReceipt", "PubkyMarketOrderReceipt"],
};

const builders = {
  shop: [(b, i) => record(b.createShop(i), "shop"), (o, i) => record(ours.buildShop(o, i), "shop")],
  listing: [
    (b, i) => timed(() => record(b.createListing(i), "listing")),
    (o, i) => timed(() => record(ours.buildListing(o, i), "listing")),
  ],
  drop: [(b, i) => record(b.createMarketplaceDrop(i), "marketplace_drop"), (o, i) => record(ours.buildDrop(o, i), "drop")],
  review: [
    (b, i) => record(b.createMarketplaceReview(i), "marketplace_review"),
    (o, i) => record(ours.buildReview(o, i), "review"),
  ],
  review_response: [
    (b, i) => record(b.createReviewResponse(i), "review_response"),
    (o, i) => record(ours.buildReviewResponse(o, i), "review_response"),
  ],
  watchlist: [(b, i) => record(b.createWatchlist(i), "watchlist"), (o, i) => record(ours.buildWatchlist(o, i), "watchlist")],
  order_receipt: [
    (b, i) => record(b.createMarketplaceOrderReceipt(i), "order_receipt"),
    (o, i) => record(ours.buildOrderReceipt(o, i), "order_receipt"),
  ],
};

const forgeryCheck = {
  review: { verified: "verifyPurchaseAttestation" },
  order_receipt: { verified: "verifyOrderReceiptAttestation", "edition verified": "verifyDropEditionAttestation" },
};

let compared = 0;
let accepted = 0;
let forgeries = 0;
const verdicts = (...outcomes) => outcomes.map((o) => (o === "err" ? "err" : "accepted")).join(" ");
const differences = [];
const compare = (label, a, b) => {
  compared += 1;
  if (a !== b) differences.push(`${label}\n    fork ${a.slice(0, 240)}\n    ours ${b.slice(0, 240)}`);
};

for (const item of cases) {
  const label = `${item.seed} ${item.case}`;
  let input;
  try {
    input = JSON.parse(item.input);
  } catch {
    continue;
  }
  const isObject = input !== null && typeof input === "object" && !Array.isArray(input);

  if (item.kind === "attestor_profile" || item.kind === "attestor_annotation") {
    const fn = item.kind === "attestor_profile" ? "verifyAttestorProfile" : "verifyAttestorAnnotation";
    const a = attempt(() => fork[fn](item.uri, item.input));
    const b = attempt(() => ours[fn](item.uri, item.input));
    compare(`${label}: ${fn}`, a, b);
    if (a.startsWith("ok:")) accepted += 1;
    if (item.forgedObservation) {
      forgeries += 1;
      compare(`${label}: forgery rejected`, "err err", verdicts(a, b));
    }
    continue;
  }

  const [forkClass, oursClass] = classes[item.kind];
  const a = attempt(() => fork[forkClass].fromJson(input).toJson());
  compare(`${label}: fromJson`, a, attempt(() => ours[oursClass].fromJson(input).toJson()));
  if (a.startsWith("ok:")) accepted += 1;

  const [forkBuild, oursBuild] = builders[item.kind];
  compare(
    `${label}: build`,
    attempt(() => forkBuild(new fork.PubkySpecsBuilder(item.owner), input)),
    attempt(() => oursBuild(item.owner, input))
  );

  if (item.forgedObservation) {
    forgeries += 1;
    const fn = forgeryCheck[item.kind]?.[item.forgedObservation];
    const outcome = fn ? verdicts(attempt(() => fork[fn](input)), attempt(() => ours[fn](input))) : "no check";
    compare(`${label}: forgery rejected by ${fn}`, "err err", outcome);
  }

  if (item.kind === "review" && isObject) {
    for (const fn of ["verifyPurchaseAttestation", "itemCountKey", "sellerRatingKey"]) {
      compare(`${label}: ${fn}`, attempt(() => fork[fn](input)), attempt(() => ours[fn](input)));
    }
    if (typeof input.eligibilityAttestation === "string") {
      compare(
        `${label}: parsePurchaseAttestation`,
        attempt(() => fork.parsePurchaseAttestation(input.eligibilityAttestation)),
        attempt(() => ours.parsePurchaseAttestation(input.eligibilityAttestation))
      );
    }
  }
  if (item.kind === "order_receipt" && isObject) {
    for (const fn of ["verifyOrderReceiptAttestation", "verifyDropEditionAttestation"]) {
      compare(`${label}: ${fn}`, attempt(() => fork[fn](input)), attempt(() => ours[fn](input)));
    }
    if (typeof input.receiptAttestation === "string") {
      compare(
        `${label}: parseOrderReceiptAttestation`,
        attempt(() => fork.parseOrderReceiptAttestation(input.receiptAttestation)),
        attempt(() => ours.parseOrderReceiptAttestation(input.receiptAttestation))
      );
    }
    if (typeof input.editionAttestation === "string") {
      compare(
        `${label}: parseDropEditionAttestation`,
        attempt(() => fork.parseDropEditionAttestation(input.editionAttestation)),
        attempt(() => ours.parseDropEditionAttestation(input.editionAttestation))
      );
    }
  }
}

const owners = [...new Set(cases.map((c) => c.owner)), "not-a-key", ""];
const ids = ["0032SSN7Q4EVG", "spring-drop-01", "a%20b", "\u00e9", "x/y", "", undefined, null];
const uriBuilders = [
  ["shop", (o) => fork.shopUriBuilder(o)],
  ["watchlist", (o) => fork.watchlistUriBuilder(o)],
  ["attestor_profile", (o) => fork.attestorProfileUriBuilder(o)],
  ["listing", (o, id) => fork.listingUriBuilder(o, id)],
  ["drop", (o, id) => fork.dropUriBuilder(o, id)],
  ["review", (o, id) => fork.marketplaceReviewUriBuilder(o, id)],
  ["review_response", (o, id) => fork.reviewResponseUriBuilder(o, id)],
  ["order_receipt", (o, id) => fork.orderReceiptUriBuilder(o, id)],
  ["attestor_annotation", (o, id) => fork.attestorAnnotationUriBuilder(o, id)],
];
for (const owner of owners) {
  for (const [kind, build] of uriBuilders) {
    const singleton = build.length === 1;
    for (const id of singleton ? [undefined, "ignored"] : ids) {
      compare(`uri ${kind} ${owner} ${id}`, attempt(() => build(owner, id)), attempt(() => ours.buildMarketUri(owner, kind, id)));
    }
  }
}

// parseMarketUri gives the fork's parse_uri result for Market resources and
// `unknown` for every other pubky.app path (including social paths whose id
// the fork rejects).
const parsed = (parse, uri) =>
  attempt(() => {
    const r = parse(uri);
    return { user_id: r.user_id, resource: r.resource, resource_id: r.resource_id ?? null };
  });
for (const uri of uris) {
  const a = parsed(fork.parse_uri, uri);
  const b = parsed(ours.parseMarketUri, uri);
  const forkMarket = a.startsWith("ok:") && JSON.parse(a.slice(3)).resource.startsWith("marketplace/");
  const socialPath = !uri.includes("/pub/pubky.app/marketplace/");
  let expected = a;
  if (a.startsWith("ok:") && !forkMarket) {
    expected = `ok:${describe({ ...JSON.parse(a.slice(3)), resource: "unknown", resource_id: null })}`;
  } else if (a === "err" && socialPath && b.startsWith("ok:") && JSON.parse(b.slice(3)).resource === "unknown") {
    expected = b;
  }
  compare(`parseMarketUri ${uri}`, expected, b);
}

const currencies = ["USD", "EUR", "GBP", "CHF", "CAD", "AUD", "JPY", "BTC", "SAT", "XYZ", "usd", ""];
const amounts = [0, 1, 9, 10, 99, 100, 12345, 25000, Number.MAX_SAFE_INTEGER, -1, 1.5, 2n ** 62n];
for (const currency of currencies) {
  for (const exponent of [0, 1, 2, 3, 8, 10, 18, 19]) {
    for (const amount of amounts) {
      compare(
        `priceDecade ${currency} ${exponent} ${amount}`,
        attempt(() => fork.priceDecade(currency, exponent, amount)),
        attempt(() => ours.priceDecade(currency, exponent, amount))
      );
    }
  }
}
for (const decade of [undefined, null, "", "USD:2", "BTC:4", "SAT:5", "JPY:3", "XYZ:3", "usd:2", "USD:x", "USD:-1", "BTC:99"]) {
  compare(`priceWeight ${decade}`, attempt(() => fork.priceWeight(decade)), attempt(() => ours.priceWeight(decade)));
}

console.log(`js parity against ${path.resolve(forkDir)}`);
console.log(
  `  ${cases.length} record cases (${accepted} accepted by pubky-app-specs, ${forgeries} forgeries), ${uris.length} URIs, ${compared} comparisons`
);
if (differences.length === 0) {
  console.log("js parity: no differences");
} else {
  console.log(`js parity: ${differences.length} differences (first 50 shown)`);
  for (const difference of differences.slice(0, 50)) console.log(`  ${difference}`);
  process.exit(1);
}
