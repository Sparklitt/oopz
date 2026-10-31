# 0004: Crates for decrypting WhatsApp crypt15 backups

Status: accepted (2026-10-23)

oops decrypts a person's own end-to-end encrypted WhatsApp backup with the 64-digit key they chose. It never rolls
its own cryptography: only the crates below.

**The format** (as documented by the open-source `wa-crypt-tools`, which we use only to cross-check on synthetic
files): a one-byte header length, an optional `0x01` feature flag, a protobuf header holding a 16-byte IV, then the
zlib-compressed SQLite database encrypted with AES-256-GCM (16-byte nonce), a 16-byte tag, and on the main database a
16-byte MD5 checksum of everything before it. The 64-digit key is 32 bytes in hex; the encryption key is
HKDF-SHA256 of it with a zero salt and the info `"backup encryption"`.

**Decision.** Only these crates, all RustCrypto or long-established:

| Need | Crate |
|---|---|
| AES-256-GCM with a 16-byte nonce | `aes-gcm` 0.11 (`AesGcm<Aes256, U16>`) |
| Key derivation | `hkdf` 0.13 + `sha2` 0.11 |
| zlib | `flate2` 1 (not cryptography) |
| The protobuf header | `prost` 0.14, derive only, no code generation |
| Hex key parsing | `hex` 0.4 |
| Wiping the key from memory | `zeroize` 1 |
| Reading the key without echo | `rpassword` 7 |

The MD5 footer is not checked when reading: AES-GCM's tag already authenticates every byte we use. Writing a backup
(the merge) has its own record, 0006.

## Consequences

- A wrong key, a truncated file or an unknown header fails loudly with a clear message that never includes the key.
- The key is read from the terminal without echo, held in a `Zeroizing` buffer, and never accepted as an argument or
  environment variable (both leak into shell history and process lists).
- `cargo audit` covers these crates with the rest.
