// Pairing with the GetLocked app. Mirrors crates/core/src/pairing.rs; the
// test vector in pairing.test.ts is checked on both sides.
//
// Both sides know a per-install pairing code. On connect, each proves it
// knows the code with an HMAC-SHA256 over both nonces and the port the
// extension dialled, so a program squatting a lower port can neither pass
// as the app nor relay a real handshake through to it.

const ALPHABET = "abcdefghjkmnpqrstuvwxyz23456789";
export const CODE_LEN = 24;

export type Side = "app" | "extension";
const LABELS: Record<Side, string> = { app: "getlocked-app", extension: "getlocked-ext" };

/** Lowercases and drops dashes and spaces. `null` if it isn't a full code. */
export function normalizeCode(input: string): string | null {
  const code = input.toLowerCase().replace(/[^a-z0-9]/g, "");
  return code.length === CODE_LEN && [...code].every((c) => ALPHABET.includes(c)) ? code : null;
}

const toHex = (bytes: Uint8Array) => [...bytes].map((b) => b.toString(16).padStart(2, "0")).join("");

/** A random hex nonce for one handshake. */
export function nonce(): string {
  return toHex(crypto.getRandomValues(new Uint8Array(16)));
}

function key(code: string): Promise<CryptoKey> {
  return crypto.subtle.importKey("raw", new TextEncoder().encode(code), { name: "HMAC", hash: "SHA-256" }, false, [
    "sign",
    "verify",
  ]);
}

const message = (side: Side, appNonce: string, extNonce: string, port: number) =>
  new TextEncoder().encode(`${LABELS[side]}|${appNonce}|${extNonce}|${port}`);

/** The hex proof `side` sends for this handshake. */
export async function proof(code: string, side: Side, appNonce: string, extNonce: string, port: number): Promise<string> {
  const sig = await crypto.subtle.sign("HMAC", await key(code), message(side, appNonce, extNonce, port));
  return toHex(new Uint8Array(sig));
}

/** Checks a proof (WebCrypto's verify is constant-time). */
export async function verify(
  code: string,
  side: Side,
  appNonce: string,
  extNonce: string,
  port: number,
  proofHex: unknown,
): Promise<boolean> {
  if (typeof proofHex !== "string" || !/^[0-9a-f]{64}$/i.test(proofHex)) return false;
  const sig = new Uint8Array(proofHex.match(/../g)!.map((h) => parseInt(h, 16)));
  return crypto.subtle.verify("HMAC", await key(code), sig, message(side, appNonce, extNonce, port));
}
