import { describe, expect, it } from "vitest";
import { CODE_LEN, nonce, normalizeCode, proof, verify } from "./pairing";

const CODE = "k7qpx2rm3n4b5c6d7e8f9g2h";

describe("pairing", () => {
  it("matches the shared test vector", async () => {
    // Same values as crates/core/src/pairing.rs.
    expect(await proof(CODE, "extension", "0011aabb", "ccdd2233", 47613)).toBe(
      "6d4d1d32b7a25926213559f1ee1913a0a9c037d2873c8e859579e7a864423bcb",
    );
    expect(await proof(CODE, "app", "0011aabb", "ccdd2233", 47613)).toBe(
      "2872337fe1d08fc0ac3d73657f6c17354c7ec62a3e38b3b102f88fa4698eeb9d",
    );
  });

  it("verify rejects the wrong code, side, port, nonce or garbage", async () => {
    const p = await proof(CODE, "app", "aa", "bb", 47613);
    expect(await verify(CODE, "app", "aa", "bb", 47613, p)).toBe(true);
    expect(await verify("x".repeat(CODE_LEN), "app", "aa", "bb", 47613, p)).toBe(false);
    expect(await verify(CODE, "extension", "aa", "bb", 47613, p)).toBe(false);
    // Bound to the dialled port: a relayed handshake fails.
    expect(await verify(CODE, "app", "aa", "bb", 47614, p)).toBe(false);
    expect(await verify(CODE, "app", "aa", "bc", 47613, p)).toBe(false);
    expect(await verify(CODE, "app", "aa", "bb", 47613, "not hex")).toBe(false);
    expect(await verify(CODE, "app", "aa", "bb", 47613, undefined)).toBe(false);
  });

  it("normalizes typed codes", () => {
    expect(normalizeCode("K7QP-X2RM-3N4B-5C6D-7E8F-9G2H ")).toBe(CODE);
    expect(normalizeCode("too-short")).toBeNull();
    expect(normalizeCode("0".repeat(CODE_LEN))).toBeNull();
  });

  it("nonces are random hex", () => {
    expect(nonce()).toMatch(/^[0-9a-f]{32}$/);
    expect(nonce()).not.toBe(nonce());
  });
});
