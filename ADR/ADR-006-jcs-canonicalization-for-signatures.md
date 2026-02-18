# ADR-006: JCS (RFC 8785) Canonicalization for Signatures

**Status:** Accepted
**Date:** 2026-02-18

## Context

JSON serialization is not deterministic — key order, whitespace, and number formatting vary across implementations. Signing a JSON document requires a canonical form so that:

1. The same logical document always produces the same bytes
2. Signatures can be verified across different JSON libraries and languages
3. `signed_fields` can select which top-level fields are included in the signature

## Decision

Use **JCS (JSON Canonicalization Scheme, RFC 8785)** for all signature operations:

1. **Canonicalization**: Before hashing, the payload is serialized per RFC 8785 rules:
   - Object keys sorted lexicographically
   - No whitespace
   - Numbers in shortest form (no trailing zeros, no positive exponent sign)
   - Strings with minimal escaping
2. **Signing**: Ed25519 over SHA-256 hash of the JCS-canonical bytes.
3. **signed_fields**: The `signature.signed_fields` array lists which top-level keys are included. Only those fields (in sorted order) form the canonical payload. `signed_fields` must cover all non-signature top-level fields.
4. **key_id**: Enables key rotation without breaking existing signatures. Verification requires matching `key_id` to the correct public key.
5. **Verification fails** on mismatch of `canonicalization`, `key_id`, or `signed_fields` between the signature block and the verifier's expectations.

**Signature block:**
```json
{
  "algorithm": "ed25519",
  "key_id": "k-2026-02",
  "signer": "admin@co.com",
  "canonicalization": "JCS-RFC8785",
  "signed_fields": ["version", "name", "role", ...],
  "created_at": "2026-02-18T12:00:00Z",
  "digest": "sha256:...",
  "value": "base64-encoded-ed25519-signature"
}
```

## Consequences

- Cross-language signature verification (Rust, Python, JavaScript all have JCS libs)
- `signed_fields` enables partial signatures (sign identity without signing ephemeral state)
- Key rotation via `key_id` without re-signing all existing personas
- Dependency on JCS implementation (or hand-rolled canonical serializer)
- Slightly more complex than signing raw bytes, but much more robust
