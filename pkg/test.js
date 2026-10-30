import {
  PubkyMarketWatchlist,
  PubkyMarketOrderReceipt,
  PubkyMarketDrop,
  PubkyMarketShop,
  PubkyMarketListing,
  PubkyMarketReview,
  PubkyMarketReviewResponse,
  buildShop,
  buildListing,
  buildReview,
  buildReviewResponse,
  buildWatchlist,
  buildOrderReceipt,
  buildDrop,
  buildMarketUri,
  parseMarketUri,
  parseOrderReceiptAttestation,
  verifyOrderReceiptAttestation,
  parseDropEditionAttestation,
  verifyDropEditionAttestation,
  itemCountKey,
  sellerRatingKey,
  priceWeight,
  priceDecade,
  parsePurchaseAttestation,
  verifyPurchaseAttestation,
  verifyAttestorProfile,
  verifyAttestorAnnotation,
} from "./index.js";
import { readFileSync } from "node:fs";
import { createPrivateKey, generateKeyPairSync, sign as cryptoSign } from "node:crypto";
import assert from "assert";

const OTTO = "8kkppkmiubfq4pxn6f73nqrhhhgkb5xyfprntc9si3np9ydbotto";
const RIO = "dzswkfy7ek3bqnoc89jxuqqfbzhjrj6mi8qthgbxxcqkdugm3rio";

const readJson = (relative) => JSON.parse(readFileSync(new URL(relative, import.meta.url), "utf8"));
const seed = (name) => readJson(`../test-vectors/seeds/${name}.json`);

// serde-wasm-bindgen turns `None` into `undefined`, which JSON drops, and Rust
// maps into JS `Map`s; serde_json writes `null` and objects. Compare plain
// JSON with maps as objects and null members removed on both sides.
const withoutNulls = (value) => {
  if (value instanceof Map) return withoutNulls(Object.fromEntries(value));
  if (Array.isArray(value)) return value.map(withoutNulls);
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value)
        .filter(([, v]) => v !== null && v !== undefined)
        .map(([k, v]) => [k, withoutNulls(v)])
    );
  }
  return value;
};
const plain = (value) => JSON.stringify(withoutNulls(value));

describe("pubky-market-specs", () => {
  describe("Watchlist pubky-market-specs (private)", () => {
    const watchlistBody = () => ({
      schemaVersion: 1,
      recordType: "watchlist",
      ownerPubky: OTTO,
      revision: 1,
      createdAt: "2025-01-01T00:00:00Z",
      updatedAt: "2025-01-02T00:00:00Z",
      items: [
        { listingOwnerPubky: RIO, listingId: "0032SSN7Q4EVG", watchedAtMs: 1735689600000 },
      ],
      tombstones: [
        { listingOwnerPubky: RIO, listingId: "0032SSN7Q4EVH", removedAtMs: 1735776000000 },
      ],
    });

    it("should create a private watchlist under /priv", () => {
      const { watchlist, meta } = buildWatchlist(OTTO, watchlistBody());

      assert.strictEqual(
        meta.path,
        "/priv/pubky.app/marketplace/v1/watchlist.json",
        "Watchlist path must live under /priv"
      );
      assert.strictEqual(
        meta.url,
        `pubky://${OTTO}/priv/pubky.app/marketplace/v1/watchlist.json`,
        "Watchlist URL must be the owner's private URI"
      );
      assert.strictEqual(meta.url, buildMarketUri(OTTO, "watchlist"), "URI builder must agree with meta");

      const json = watchlist.toJson();
      assert.strictEqual(json.recordType, "watchlist");
      assert.strictEqual(json.items[0].watchedAtMs, 1735689600000);
      assert.strictEqual(json.tombstones[0].removedAtMs, 1735776000000);

      const roundtrip = PubkyMarketWatchlist.fromJson(json);
      assert.strictEqual(roundtrip.toJson().items[0].listingId, "0032SSN7Q4EVG");
    });

    it("should reject a listing key present in both items and tombstones", () => {
      const body = watchlistBody();
      body.tombstones.push({
        listingOwnerPubky: RIO,
        listingId: "0032SSN7Q4EVG",
        removedAtMs: 1,
      });
      assert.throws(() => buildWatchlist(OTTO, body));
    });
  });

  describe("Marketplace order receipt pubky-market-specs (private)", () => {
    const RECEIPT_ID = "a7fc7d5d-0b2a-4083-b278-47193f8fe536";
    const ORDER_ID = "0e9c2c4a-91d6-4a4e-8db3-2f14c1e8b7aa";
    const PAID_AT = "2026-01-02T03:04:05Z";

    const receiptBody = () => ({
      schemaVersion: 1,
      recordType: "order_receipt",
      ownerPubky: OTTO,
      revision: 1,
      createdAt: PAID_AT,
      updatedAt: PAID_AT,
      role: "buyer",
      receiptId: RECEIPT_ID,
      orderId: ORDER_ID,
      buyerPubky: OTTO,
      sellerPubky: RIO,
      total: { amountMinor: 12000, currency: "USD", exponent: 2 },
      paidAt: PAID_AT,
      receiptAttestation: "a".repeat(64),
    });

    // z-base-32 encoding of raw bytes (the pubky encoding of Ed25519 keys).
    const zbase32 = (bytes) => {
      const alphabet = "ybndrfg8ejkmcpqxot1uwisza345h769";
      let bits = 0, accumulator = 0, out = "";
      for (const byte of bytes) {
        accumulator = (accumulator << 8) | byte;
        bits += 8;
        while (bits >= 5) {
          bits -= 5;
          out += alphabet[(accumulator >> bits) & 31];
        }
      }
      if (bits > 0) out += alphabet[(accumulator << (5 - bits)) & 31];
      return out;
    };

    const b64u = (data) => Buffer.from(data).toString("base64url");

    it("should create a private order receipt under /priv with id = receiptId", () => {
      const { order_receipt, meta } = buildOrderReceipt(OTTO, receiptBody());

      assert.strictEqual(meta.id, RECEIPT_ID, "Meta id must be the receipt UUID");
      assert.strictEqual(
        meta.path,
        `/priv/pubky.app/marketplace/v1/receipts/${RECEIPT_ID}`,
        "Receipt path must live under /priv"
      );
      assert.strictEqual(
        meta.url,
        `pubky://${OTTO}/priv/pubky.app/marketplace/v1/receipts/${RECEIPT_ID}`,
        "Receipt URL must be the owner's private URI"
      );
      assert.strictEqual(
        meta.url,
        buildMarketUri(OTTO, "order_receipt", RECEIPT_ID),
        "URI builder must agree with meta"
      );

      const json = order_receipt.toJson();
      assert.strictEqual(json.recordType, "order_receipt");
      assert.strictEqual(json.role, "buyer");
      assert.strictEqual(json.total.amountMinor, 12000);

      const roundtrip = PubkyMarketOrderReceipt.fromJson(json);
      assert.strictEqual(roundtrip.toJson().orderId, ORDER_ID);
    });

    it("should reject an owner/role mismatch", () => {
      const body = receiptBody();
      body.role = "seller"; // owner OTTO is the buyer, not the seller
      assert.throws(() => buildOrderReceipt(OTTO, body));
    });

    it("should verify a real Ed25519 receipt attestation end to end", () => {
      const { publicKey, privateKey } = generateKeyPairSync("ed25519");
      const rawPublicKey = Buffer.from(publicKey.export({ format: "jwk" }).x, "base64url");
      const iss = zbase32(rawPublicKey);
      assert.strictEqual(iss.length, 52, "iss must be a 52-char pubky");

      const claims = {
        v: 1,
        iss,
        buyer: OTTO,
        seller: RIO,
        order: ORDER_ID,
        receipt: RECEIPT_ID,
        total_minor: 12000,
        currency: "USD",
        exponent: 2,
        paid_at: PAID_AT,
        iat: Date.parse(PAID_AT) / 1000,
      };
      const header = b64u(JSON.stringify({ alg: "EdDSA", typ: "pubky-order-receipt+v1" }));
      const payload = b64u(JSON.stringify(claims));
      const signature = cryptoSign(null, Buffer.from(`${header}.${payload}`), privateKey);
      const jws = `${header}.${payload}.${b64u(signature)}`;

      const parsed = parseOrderReceiptAttestation(jws);
      assert.strictEqual(parsed.iss, iss, "Parsed claims must carry the attestor pubky");

      const body = receiptBody();
      body.receiptAttestation = jws;
      const verified = verifyOrderReceiptAttestation(body);
      assert.strictEqual(verified.receipt, RECEIPT_ID, "Verified claims must bind the receipt");
      assert.strictEqual(verified.total_minor, 12000, "Verified claims must bind the total");
    });

    it("should reject an attestation that does not verify", () => {
      // Structurally charset-valid but not a real JWS.
      assert.throws(() => verifyOrderReceiptAttestation(receiptBody()));
      // Structurally invalid compact form.
      assert.throws(() => parseOrderReceiptAttestation("not.a.jws"));
    });

    it("should parse the captured v2 fixture with the unchanged snake_case return shape", () => {
      const fixture = JSON.parse(readFileSync(
        "../src/test_fixtures/order-receipt-attestation-v2-same-currency.json",
        "utf8"
      ));
      const parsed = parseOrderReceiptAttestation(fixture.receipt_attestation.jws);
      assert.deepStrictEqual(Object.keys(parsed).sort(), [
        "buyer", "iat", "iss", "merchandise_total", "order",
        "paid_at", "receipt", "seller", "settlement_total", "v",
      ]);
      assert.strictEqual(parsed.v, 2);
      assert.strictEqual(parsed.merchandise_total.amount_minor, 13700);
      assert.strictEqual(parsed.settlement_total.currency, "USD");
    });

    it("should reject a v2 fixture with the wrong typ or a tampered payload", () => {
      const fixture = JSON.parse(readFileSync(
        "../src/test_fixtures/order-receipt-attestation-v2-same-currency.json",
        "utf8"
      ));
      const [header, payload, signature] = fixture.receipt_attestation.jws.split(".");
      const wrongTyp = Buffer.from(
        JSON.stringify({ alg: "EdDSA", typ: "pubky-order-receipt+v1" })
      ).toString("base64url");
      assert.throws(() => parseOrderReceiptAttestation(
        `${wrongTyp}.${payload}.${signature}`
      ));
      const tamperedClaims = JSON.parse(Buffer.from(payload, "base64url").toString("utf8"));
      tamperedClaims.merchandise_total.amount_minor += 1;
      const tamperedPayload = Buffer.from(JSON.stringify(tamperedClaims)).toString("base64url");
      const v2Body = {
        schemaVersion: 1,
        recordType: "order_receipt",
        ownerPubky: tamperedClaims.buyer,
        revision: 1,
        createdAt: tamperedClaims.paid_at,
        updatedAt: tamperedClaims.paid_at,
        role: "buyer",
        receiptId: tamperedClaims.receipt,
        orderId: tamperedClaims.order,
        buyerPubky: tamperedClaims.buyer,
        sellerPubky: tamperedClaims.seller,
        total: {
          amountMinor: tamperedClaims.merchandise_total.amount_minor - 1,
          currency: tamperedClaims.merchandise_total.currency,
          exponent: tamperedClaims.merchandise_total.exponent,
        },
        paidAt: tamperedClaims.paid_at,
        receiptAttestation: fixture.receipt_attestation.jws,
      };
      assert.doesNotThrow(() => parseOrderReceiptAttestation(
        `${header}.${tamperedPayload}.${signature}`
      ));
      assert.throws(() => verifyOrderReceiptAttestation({
        ...v2Body,
        receiptAttestation: `${header}.${tamperedPayload}.${signature}`,
      }));
    });

    it("should keep parsing a .7-shaped receipt without the drop-edition fields", () => {
      const { order_receipt } = buildOrderReceipt(OTTO, receiptBody());
      const json = order_receipt.toJson();
      assert.ok(!("editionAttestation" in json) || json.editionAttestation == null,
        "Absent editionAttestation must stay absent");
      assert.ok(!("drop" in json) || json.drop == null, "Absent drop must stay absent");

      const roundtrip = PubkyMarketOrderReceipt.fromJson(json).toJson();
      assert.ok(!("editionAttestation" in roundtrip) || roundtrip.editionAttestation == null,
        "Roundtrip must not invent editionAttestation");
      assert.ok(!("drop" in roundtrip) || roundtrip.drop == null, "Roundtrip must not invent drop");
    });

    it("should reject editionAttestation without the drop object (and vice versa)", () => {
      const lonelyAttestation = receiptBody();
      lonelyAttestation.editionAttestation = "b".repeat(64);
      assert.throws(() => buildOrderReceipt(OTTO, lonelyAttestation));

      const lonelyDrop = receiptBody();
      lonelyDrop.drop = { dropId: "spring-drop-01", edition: 7, of: 500 };
      assert.throws(() => buildOrderReceipt(OTTO, lonelyDrop));
    });

    it("should accept a receipt carrying both drop-edition fields", () => {
      const body = receiptBody();
      body.editionAttestation = "b".repeat(64);
      body.drop = { dropId: "spring-drop-01", edition: 7, of: 500 };
      const { order_receipt } = buildOrderReceipt(OTTO, body);
      const json = order_receipt.toJson();
      assert.strictEqual(json.drop.dropId, "spring-drop-01");
      assert.strictEqual(json.drop.edition, 7);
      assert.strictEqual(json.drop.of, 500);
      assert.strictEqual(json.editionAttestation, "b".repeat(64));
    });
  });

  describe("Marketplace drop pubky-market-specs", () => {
    const DROP_ID = "spring-drop-01";

    const dropBody = () => ({
      schemaVersion: 1,
      recordType: "drop",
      ownerPubky: OTTO,
      revision: 1,
      createdAt: "2026-01-01T00:00:00Z",
      updatedAt: "2026-01-02T00:00:00Z",
      dropId: DROP_ID,
      title: "Spring boot drop",
      description: "Limited spring release.",
      media: [`pubky://${OTTO}/pub/pubky.app/marketplace/v1/media/drop_banner`],
      format: "fcfs",
      startsAt: "2026-02-01T00:00:00Z",
      endsAt: "2026-02-02T00:00:00Z",
      listingIds: ["listing_01", "listing_02"],
      totalQuantity: 500,
      perBuyerLimit: 2,
      stockDisplay: "bands",
    });

    it("should create a public drop with id = dropId", () => {
      const { drop, meta } = buildDrop(OTTO, dropBody());

      assert.strictEqual(meta.id, DROP_ID, "Meta id must be the dropId");
      assert.strictEqual(
        meta.path,
        `/pub/pubky.app/marketplace/v1/drops/${DROP_ID}`,
        "Drop path must live under /pub so it can be indexed"
      );
      assert.strictEqual(
        meta.url,
        `pubky://${OTTO}/pub/pubky.app/marketplace/v1/drops/${DROP_ID}`,
        "Drop URL must be the owner's public URI"
      );
      assert.strictEqual(meta.url, buildMarketUri(OTTO, "drop", DROP_ID), "URI builder must agree with meta");

      const json = drop.toJson();
      assert.strictEqual(json.recordType, "drop");
      assert.strictEqual(json.format, "fcfs");
      assert.strictEqual(json.stockDisplay, "bands");
      assert.strictEqual(json.totalQuantity, 500);
      assert.strictEqual(json.perBuyerLimit, 2);

      const roundtrip = PubkyMarketDrop.fromJson(json);
      assert.strictEqual(roundtrip.toJson().dropId, DROP_ID);
    });

    it("should accept an open-ended drop without endsAt", () => {
      const body = dropBody();
      delete body.endsAt;
      const { drop } = buildDrop(OTTO, body);
      const json = drop.toJson();
      assert.ok(!("endsAt" in json) || json.endsAt == null, "Absent endsAt must stay absent");
    });

    it("should reject endsAt at or before startsAt", () => {
      const body = dropBody();
      body.endsAt = body.startsAt;
      assert.throws(() => buildDrop(OTTO, body));
    });

    it("should reject perBuyerLimit above totalQuantity", () => {
      const body = dropBody();
      body.totalQuantity = 3;
      body.perBuyerLimit = 4;
      assert.throws(() => buildDrop(OTTO, body));
    });

    it("should reject unknown format values (closed-world enum)", () => {
      const body = dropBody();
      body.format = "auction";
      assert.throws(() => buildDrop(OTTO, body));
    });

    it("should reject duplicate listingIds", () => {
      const body = dropBody();
      body.listingIds = ["listing_01", "listing_01"];
      assert.throws(() => buildDrop(OTTO, body));
    });
  });

  describe("Drop edition attestation pubky-market-specs", () => {
    const RECEIPT_ID = "a7fc7d5d-0b2a-4083-b278-47193f8fe536";
    const ORDER_ID = "0e9c2c4a-91d6-4a4e-8db3-2f14c1e8b7aa";
    const DROP_ID = "spring-drop-01";
    const PAID_AT = "2026-01-02T03:04:05Z";

    // z-base-32 encoding of raw bytes (the pubky encoding of Ed25519 keys).
    const zbase32 = (bytes) => {
      const alphabet = "ybndrfg8ejkmcpqxot1uwisza345h769";
      let bits = 0, accumulator = 0, out = "";
      for (const byte of bytes) {
        accumulator = (accumulator << 8) | byte;
        bits += 8;
        while (bits >= 5) {
          bits -= 5;
          out += alphabet[(accumulator >> bits) & 31];
        }
      }
      if (bits > 0) out += alphabet[(accumulator << (5 - bits)) & 31];
      return out;
    };

    const b64u = (data) => Buffer.from(data).toString("base64url");

    const receiptWithDrop = (editionAttestation) => ({
      schemaVersion: 1,
      recordType: "order_receipt",
      ownerPubky: OTTO,
      revision: 1,
      createdAt: PAID_AT,
      updatedAt: PAID_AT,
      role: "buyer",
      receiptId: RECEIPT_ID,
      orderId: ORDER_ID,
      buyerPubky: OTTO,
      sellerPubky: RIO,
      total: { amountMinor: 12000, currency: "USD", exponent: 2 },
      paidAt: PAID_AT,
      receiptAttestation: "a".repeat(64),
      editionAttestation,
      drop: { dropId: DROP_ID, edition: 7, of: 500 },
    });

    it("should verify a real Ed25519 drop edition attestation end to end", () => {
      const { publicKey, privateKey } = generateKeyPairSync("ed25519");
      const rawPublicKey = Buffer.from(publicKey.export({ format: "jwk" }).x, "base64url");
      const iss = zbase32(rawPublicKey);
      assert.strictEqual(iss.length, 52, "iss must be a 52-char pubky");

      const claims = {
        v: 1,
        iss,
        buyer: OTTO,
        seller: RIO,
        drop: DROP_ID,
        edition: 7,
        of: 500,
        receipt: RECEIPT_ID,
        iat: Date.parse(PAID_AT) / 1000,
      };
      const header = b64u(JSON.stringify({ alg: "EdDSA", typ: "pubky-drop-edition+v1" }));
      const payload = b64u(JSON.stringify(claims));
      const signature = cryptoSign(null, Buffer.from(`${header}.${payload}`), privateKey);
      const jws = `${header}.${payload}.${b64u(signature)}`;

      const parsed = parseDropEditionAttestation(jws);
      assert.strictEqual(parsed.iss, iss, "Parsed claims must carry the attestor pubky");
      assert.strictEqual(parsed.edition, 7, "Parsed claims must carry the edition");

      const verified = verifyDropEditionAttestation(receiptWithDrop(jws));
      assert.strictEqual(verified.drop, DROP_ID, "Verified claims must bind the drop");
      assert.strictEqual(verified.of, 500, "Verified claims must bind the drop total");
    });

    it("should reject a receipt missing the drop object or the attestation", () => {
      const noDrop = receiptWithDrop("b".repeat(64));
      delete noDrop.drop;
      delete noDrop.editionAttestation;
      assert.throws(() => verifyDropEditionAttestation(noDrop));

      // Structurally invalid compact form.
      assert.throws(() => parseDropEditionAttestation("not.a.jws"));
    });
  });

  describe("Marketplace review counting keys and price weight", () => {
    const ORDER_REF = "c".repeat(64);
    const ITEM_A = "0032SSN7Q4EVG";
    const ITEM_B = "0032SSN7Q4EVH";

    // z-base-32 encoding of raw bytes (the pubky encoding of Ed25519 keys).
    const zbase32 = (bytes) => {
      const alphabet = "ybndrfg8ejkmcpqxot1uwisza345h769";
      let bits = 0, accumulator = 0, out = "";
      for (const byte of bytes) {
        accumulator = (accumulator << 8) | byte;
        bits += 8;
        while (bits >= 5) {
          bits -= 5;
          out += alphabet[(accumulator >> bits) & 31];
        }
      }
      if (bits > 0) out += alphabet[(accumulator << (5 - bits)) & 31];
      return out;
    };

    const b64u = (data) => Buffer.from(data).toString("base64url");

    const { publicKey, privateKey } = generateKeyPairSync("ed25519");
    const iss = zbase32(Buffer.from(publicKey.export({ format: "jwk" }).x, "base64url"));

    const attestedReview = (listingId, text, priceDecadeClaim) => {
      const claims = {
        v: 1,
        iss,
        sub: OTTO,
        cpk: RIO,
        role: "buyer_reviewing_seller",
        listing: `pubky://${RIO}/pub/pubky.app/marketplace/v1/listings/${listingId}`,
        order_ref: ORDER_REF,
        completed_on: "2026-10-20",
        iat: 1792454400,
        ...(priceDecadeClaim === undefined ? {} : { price_decade: priceDecadeClaim }),
      };
      const header = b64u(JSON.stringify({ alg: "EdDSA", typ: "pubky-purchase-attestation+v1" }));
      const payload = b64u(JSON.stringify(claims));
      const signature = cryptoSign(null, Buffer.from(`${header}.${payload}`), privateKey);
      const { review } = buildReview(OTTO, {
        schemaVersion: 1,
        recordType: "review",
        ownerPubky: OTTO,
        revision: 1,
        createdAt: "2026-10-20T10:00:00Z",
        updatedAt: "2026-10-20T10:00:00Z",
        reviewId: "",
        subjectPubky: RIO,
        listingOwnerPubky: RIO,
        listingId,
        role: "buyer_reviewing_seller",
        ratings: { overall: 5, shipping: 4, communication: 5 },
        text,
        eligibilityAttestation: `${header}.${payload}.${b64u(signature)}`,
        orderRef: ORDER_REF,
      });
      return review.toJson();
    };

    it("should accept a review with empty text", () => {
      const review = attestedReview(ITEM_A, "");
      assert.strictEqual(review.text, "");
    });

    it("should key items of one order apart and share the seller key", () => {
      const reviewA = attestedReview(ITEM_A, "Great boots.");
      const reviewB = attestedReview(ITEM_B, "");

      assert.strictEqual(
        itemCountKey(reviewA),
        `${iss}:${ORDER_REF}:buyer_reviewing_seller:${RIO}:${ITEM_A}`
      );
      assert.notStrictEqual(itemCountKey(reviewA), itemCountKey(reviewB));
      assert.strictEqual(sellerRatingKey(reviewA), `${iss}:${ORDER_REF}:buyer_reviewing_seller`);
      assert.strictEqual(sellerRatingKey(reviewA), sellerRatingKey(reviewB));
    });

    it("should refuse to key a review whose attestation does not verify", () => {
      const review = attestedReview(ITEM_A, "");
      review.listingId = ITEM_B;
      assert.throws(() => itemCountKey(review));
      assert.throws(() => sellerRatingKey(review));
    });

    it("should weigh reviews by the attested price decade", () => {
      assert.strictEqual(priceDecade("USD", 2, 999), "USD:2");
      assert.strictEqual(priceDecade("SAT", 0, 100000n), "SAT:5");
      assert.strictEqual(priceDecade("BTC", 8, 25000), "BTC:4");
      assert.strictEqual(priceDecade("BTC", 0, 1), "BTC:8");
      assert.strictEqual(priceDecade("JPY", 0, 4500), "JPY:3");
      assert.strictEqual(priceDecade("USD", 2, 0), undefined);
      assert.throws(() => priceDecade("USD", 2, 1.5));

      assert.strictEqual(priceWeight("USD:2"), 1);
      assert.strictEqual(priceWeight("USD:3"), 2);
      assert.strictEqual(priceWeight("EUR:6"), 5);
      assert.strictEqual(priceWeight("SAT:5"), 3);
      assert.strictEqual(priceWeight("BTC:5"), 3);
      assert.strictEqual(priceWeight("JPY:3"), 2);
      assert.strictEqual(priceWeight("CAD:3"), 2);
      assert.strictEqual(priceWeight("XYZ:9"), 1);
      assert.strictEqual(priceWeight(undefined), 1);
      assert.strictEqual(priceWeight(null), 1);
      assert.throws(() => priceWeight("usd:3"));
    });

    it("should carry the price_decade claim through attestation parsing", () => {
      const review = attestedReview(ITEM_A, "", "USD:4");
      const claims = parsePurchaseAttestation(review.eligibilityAttestation);
      assert.strictEqual(claims.price_decade, "USD:4");
      assert.strictEqual(priceWeight(claims.price_decade), 3);

      const legacy = parsePurchaseAttestation(attestedReview(ITEM_A, "").eligibilityAttestation);
      assert.strictEqual(legacy.price_decade, undefined);
      assert.strictEqual(priceWeight(legacy.price_decade), 1);
    });
  });

  describe("Attestor profile and annotation records", () => {
    const vectors = JSON.parse(
      readFileSync(new URL("../src/test_fixtures/attestor-records-v1.json", import.meta.url), "utf8")
    );
    const b64u = (data) => Buffer.from(data).toString("base64url");

    // RFC 8785 for the integer-only JSON the records use: members sorted by
    // UTF-16 code units, which is how JavaScript compares strings.
    const canonicalize = (value) => {
      if (Array.isArray(value)) return `[${value.map(canonicalize).join(",")}]`;
      if (value !== null && typeof value === "object") {
        const keys = Object.keys(value).sort();
        return `{${keys.map((key) => `${JSON.stringify(key)}:${canonicalize(value[key])}`).join(",")}}`;
      }
      return JSON.stringify(value);
    };

    const attestorKey = createPrivateKey({
      key: Buffer.concat([
        Buffer.from("302e020100300506032b657004220420", "hex"),
        Buffer.from(vectors.attestorSecretKeyHex, "hex"),
      ]),
      format: "der",
      type: "pkcs8",
    });

    for (const [name, typ] of [
      ["profile", "pubky-attestor-profile+v1"],
      ["annotation", "pubky-attestor-annotation+v1"],
    ]) {
      it(`should reproduce the ${name} vector independently`, () => {
        const vector = vectors[name];
        const { proof, ...body } = vector.record;
        assert.strictEqual(proof, vector.proof);
        assert.strictEqual(canonicalize(body), vector.canonicalBody);

        const header = b64u(JSON.stringify({ alg: "EdDSA", typ }));
        const signature = cryptoSign(
          null,
          Buffer.from(`${header}.${b64u(vector.canonicalBody)}`),
          attestorKey
        );
        assert.strictEqual(`${header}..${b64u(signature)}`, vector.proof);
      });
    }

    it("should build attestor URIs", () => {
      assert.strictEqual(buildMarketUri(vectors.attestorPubky, "attestor_profile"), vectors.profile.uri);
      const orderRef = vectors.annotation.record.orderRef;
      assert.strictEqual(
        buildMarketUri(vectors.attestorPubky, "attestor_annotation", orderRef),
        vectors.annotation.uri
      );
    });

    it("should verify the vector records", () => {
      const profile = verifyAttestorProfile(vectors.profile.uri, JSON.stringify(vectors.profile.record));
      assert.strictEqual(profile.operator.name, "Pubky Market");
      assert.strictEqual(profile.issuancePolicy.priceDecade, "always");

      const annotation = verifyAttestorAnnotation(
        vectors.annotation.uri,
        JSON.stringify(vectors.annotation.record, null, 2)
      );
      assert.strictEqual(annotation.revision, 2);
      assert.deepStrictEqual(annotation.outcomes.map((o) => o.kind), ["returned", "refunded"]);
    });

    it("should reject a wrong owner, a wrong path or a tampered body", () => {
      const record = JSON.stringify(vectors.annotation.record);
      const otherOwner = vectors.annotation.uri.replace(vectors.attestorPubky, OTTO);
      assert.throws(() => verifyAttestorAnnotation(otherOwner, record));
      const otherPath = vectors.annotation.uri.replace(/[0-9a-f]{64}$/, "a".repeat(64));
      assert.throws(() => verifyAttestorAnnotation(otherPath, record));
      assert.throws(() => verifyAttestorProfile(vectors.profile.uri, record));
      const tampered = { ...vectors.annotation.record, revision: 3 };
      assert.throws(() => verifyAttestorAnnotation(vectors.annotation.uri, JSON.stringify(tampered)));
    });
  });

  describe("Shop transactionService pubky-market-specs", () => {
    const shopBody = () => ({
      schemaVersion: 1,
      recordType: "shop",
      ownerPubky: OTTO,
      revision: 1,
      createdAt: "2025-01-01T00:00:00Z",
      updatedAt: "2025-01-02T00:00:00Z",
      name: "Boots & Co",
      bio: "Quality hiking boots.",
      location: { countryCode: "US", region: "Oregon" },
      shippingPolicy: "Ships within 3 business days.",
      returnPolicy: "Returns accepted within 30 days.",
      vacationMode: false,
    });

    it("should accept a shop with an https transactionService", () => {
      const body = shopBody();
      body.transactionService = "https://tx.example.com";
      const { shop } = buildShop(OTTO, body);
      assert.strictEqual(shop.toJson().transactionService, "https://tx.example.com");
    });

    it("should reject a non-https transactionService", () => {
      const body = shopBody();
      body.transactionService = "http://tx.example.com";
      assert.throws(() => buildShop(OTTO, body));
    });

    it("should keep parsing a .6-shaped shop without the field", () => {
      const { shop } = buildShop(OTTO, shopBody());
      const json = shop.toJson();
      assert.ok(
        !("transactionService" in json) || json.transactionService == null,
        "Absent transactionService must stay absent"
      );
    });
  });


  describe("Listing and review response builders", () => {
    it("should build a listing with a fresh timestamp id", () => {
      const body = seed("listing-fixed");
      const owner = body.ownerPubky;
      body.listingId = "";
      const { listing, meta } = buildListing(owner, body);
      assert.strictEqual(meta.id.length, 13, "Listing ids are 13-character timestamp ids");
      const CROCKFORD = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
      let bits = 0n;
      for (const c of meta.id) bits = (bits << 5n) | BigInt(CROCKFORD.indexOf(c));
      const micros = bits >> 1n;
      const now = BigInt(Date.now()) * 1000n;
      assert.ok(micros <= now + 1000n && micros > now - 60_000_000n, "The id encodes the build time");
      assert.strictEqual(listing.toJson().listingId, meta.id);
      assert.strictEqual(meta.path, `/pub/pubky.app/marketplace/v1/listings/${meta.id}`);
      assert.strictEqual(meta.url, buildMarketUri(owner, "listing", meta.id));
      assert.strictEqual(PubkyMarketListing.fromJson(listing.toJson()).toJson().listingId, meta.id);
    });

    it("should build a review response at the subject review's id", () => {
      const body = seed("review-response");
      const owner = body.ownerPubky;
      const { review_response, meta } = buildReviewResponse(owner, body);
      assert.strictEqual(meta.id, body.reviewId);
      assert.strictEqual(meta.path, `/pub/pubky.app/marketplace/v1/review_responses/${body.reviewId}`);
      assert.strictEqual(meta.url, buildMarketUri(owner, "review_response", body.reviewId));
      assert.strictEqual(plain(review_response.toJson()), plain(body));
    });

    it("should reject a review response whose reviewId does not match its review", () => {
      const body = seed("review-response");
      body.reviewId = "0000000000000000000000000Z";
      assert.throws(() => buildReviewResponse(body.ownerPubky, body));
    });
  });

  describe("buildMarketUri", () => {
    const ID = "0032SSN7Q4EVG";
    const base = `pubky://${OTTO}/pub/pubky.app/marketplace/v1`;
    const cases = [
      ["shop", undefined, `${base}/shop.json`],
      ["listing", ID, `${base}/listings/${ID}`],
      ["drop", "spring-drop-01", `${base}/drops/spring-drop-01`],
      ["review", ID, `${base}/reviews/${ID}`],
      ["review_response", ID, `${base}/review_responses/${ID}`],
      ["watchlist", undefined, `pubky://${OTTO}/priv/pubky.app/marketplace/v1/watchlist.json`],
      ["order_receipt", "r-1", `pubky://${OTTO}/priv/pubky.app/marketplace/v1/receipts/r-1`],
      ["attestor_profile", undefined, `${base}/attestor/profile`],
      ["attestor_annotation", "ref-1", `${base}/attestor/annotations/ref-1`],
    ];
    for (const [kind, id, expected] of cases) {
      it(`should build ${kind} URIs`, () => {
        assert.strictEqual(buildMarketUri(OTTO, kind, id), expected);
      });
    }

    it("should concatenate like the pubky-app-specs builders", () => {
      assert.strictEqual(buildMarketUri(OTTO, "shop", "ignored"), `${base}/shop.json`);
      assert.strictEqual(buildMarketUri(OTTO, "listing", ""), `${base}/listings/`);
      assert.strictEqual(
        buildMarketUri("not-a-pubky", "listing", ID),
        `pubky://not-a-pubky/pub/pubky.app/marketplace/v1/listings/${ID}`
      );
      assert.throws(() => buildMarketUri(OTTO, "listing"), /require an id/);
      assert.throws(() => buildMarketUri(OTTO, "posts", ID), /unknown Market URI kind/);
    });
  });

  describe("parseMarketUri", () => {
    it("should parse every Market resource and nothing else", () => {
      const ID = "0032SSN7Q4EVG";
      const cases = [
        [buildMarketUri(OTTO, "shop"), "marketplace/v1/shop.json", undefined],
        [buildMarketUri(OTTO, "listing", ID), "marketplace/v1/listings", ID],
        [buildMarketUri(OTTO, "drop", "d-1"), "marketplace/v1/drops", "d-1"],
        [buildMarketUri(OTTO, "review", ID), "marketplace/v1/reviews", ID],
        [buildMarketUri(OTTO, "review_response", ID), "marketplace/v1/review_responses", ID],
        [buildMarketUri(OTTO, "attestor_profile"), "marketplace/v1/attestor/profile", undefined],
        [buildMarketUri(OTTO, "attestor_annotation", "ref-1"), "marketplace/v1/attestor/annotations", "ref-1"],
        [`pubky://${OTTO}/pub/pubky.app/posts/${ID}`, "unknown", undefined],
      ];
      for (const [uri, resource, id] of cases) {
        const parsed = parseMarketUri(uri);
        assert.strictEqual(parsed.user_id, OTTO, uri);
        assert.strictEqual(parsed.resource, resource, uri);
        assert.strictEqual(parsed.resource_id, id, uri);
      }
      assert.throws(() => parseMarketUri(`pubky://${OTTO}/pub/other.app/marketplace/v1/shop.json`));
      assert.throws(() => parseMarketUri("not a url"));
    });
  });

  describe("Seed vectors from pubky-app-specs", () => {
    const vectors = readJson("../test-vectors/market-records-v1.json");
    const classes = {
      shop: PubkyMarketShop,
      listing: PubkyMarketListing,
      drop: PubkyMarketDrop,
      review: PubkyMarketReview,
      review_response: PubkyMarketReviewResponse,
      watchlist: PubkyMarketWatchlist,
      order_receipt: PubkyMarketOrderReceipt,
    };
    const okJson = (value) => (value.startsWith("ok:") ? value.slice(3) : null);

    for (const vector of vectors.records.filter((v) => v.case === "seed")) {
      it(`should read the ${vector.seed} seed as pubky-app-specs does`, () => {
        const input = JSON.parse(vector.input);
        if (vector.kind === "attestor_profile") {
          const record = verifyAttestorProfile(vector.uri, vector.input);
          assert.strictEqual(plain(record), plain(JSON.parse(okJson(vector.expected.validated))));
          return;
        }
        if (vector.kind === "attestor_annotation") {
          const record = verifyAttestorAnnotation(vector.uri, vector.input);
          assert.strictEqual(plain(record), plain(JSON.parse(okJson(vector.expected.validated))));
          return;
        }
        const record = classes[vector.kind].fromJson(input).toJson();
        assert.strictEqual(plain(record), plain(JSON.parse(okJson(vector.expected.validated))));

        const verified = vector.expected.verified && okJson(vector.expected.verified);
        if (verified && vector.kind === "review") {
          assert.strictEqual(plain(verifyPurchaseAttestation(input)), plain(JSON.parse(verified)));
          assert.strictEqual(itemCountKey(input), JSON.parse(okJson(vector.expected["item count key"])));
          assert.strictEqual(sellerRatingKey(input), JSON.parse(okJson(vector.expected["seller rating key"])));
        }
        if (verified && vector.kind === "order_receipt") {
          assert.strictEqual(plain(verifyOrderReceiptAttestation(input)), plain(JSON.parse(verified)));
        }
        const edition = vector.expected["edition verified"] && okJson(vector.expected["edition verified"]);
        if (edition) {
          assert.strictEqual(plain(verifyDropEditionAttestation(input)), plain(JSON.parse(edition)));
        }
      });
    }

    it("should reject every forged signature and proof", () => {
      const verifiers = {
        verified: { review: verifyPurchaseAttestation, order_receipt: verifyOrderReceiptAttestation },
        "edition verified": { order_receipt: verifyDropEditionAttestation },
      };
      const forged = vectors.records.filter((v) => v.forgedObservation);
      assert.strictEqual(forged.length, 28);
      for (const vector of forged) {
        if (vector.kind === "attestor_profile") {
          assert.throws(() => verifyAttestorProfile(vector.uri, vector.input), vector.case);
        } else if (vector.kind === "attestor_annotation") {
          assert.throws(() => verifyAttestorAnnotation(vector.uri, vector.input), vector.case);
        } else {
          const verify = verifiers[vector.forgedObservation][vector.kind];
          assert.throws(() => verify(JSON.parse(vector.input)), vector.case);
        }
      }
    });

    it("should match every price decade and weight vector", () => {
      for (const vector of vectors.price) {
        if ("currency" in vector) {
          if (!Number.isSafeInteger(vector.unitPriceMinor) || vector.exponent < 0) continue;
          const observed = priceDecade(vector.currency, vector.exponent, vector.unitPriceMinor);
          assert.strictEqual(observed ?? null, vector.expected, JSON.stringify(vector));
        } else {
          let observed;
          try {
            observed = `ok:${priceWeight(vector.priceDecade ?? undefined)}`;
          } catch {
            observed = "err";
          }
          assert.strictEqual(observed, vector.expected, JSON.stringify(vector));
        }
      }
    });
  });
});
