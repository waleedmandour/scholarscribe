# Word Assist ledger — threat model

Status: ruling E-4 applied (HMAC withdrawn). Owner review 2026-10-23.
Scope: `<data>/word-assist/ledger.jsonl` — the append-only record of
assistance events (counts and hashes, never text; PROVENANCE_SPEC §0.3).

## 1. Asset and adversaries

The ledger is *disclosure evidence*: it exists so the author can later
show that and how much assistance happened, without storing manuscript
text. Its value is integrity (the record has not been quietly edited),
not confidentiality (it contains no text). The realistic adversaries:

| # | Adversary | Capability |
|---|-----------|------------|
| A1 | Malware / another process running **as the same OS user** | Read and write every file the user can write; read the user's keychain entries |
| A2 | Another local user account | No write access to the profile directory by default; no access to the user's keychain |
| A3 | The file's owner (the author) | Anything at all: edit, truncate, delete, regenerate |
| A4 | Remote attacker | No local file access; nothing to attack here |

## 2. What the SHA-256 hash chain gives

Each entry's canonical serialization includes its predecessor's hash
(`ledger.rs::canonical_entry`), so any edit to entry *n* invalidates every
hash from *n* onward. `open()` re-derives the chain and **fails closed**
(enable refuses) on a broken chain. That is tamper *evidence*, valid for
anyone who holds an independently trusted chain head — which is what the
**export-time signature** provides: when the ledger is exported, the
chain head is signed with the existing provenance Ed25519 signing key
(`provenance_commands.rs::load_or_create_signing_key` path), so a
third party can check the exported snapshot against a signature whose
key never sits next to the file. The export surface itself is a Phase 4
gate item; until it exists, nothing signs at rest.

## 3. What HMAC with a locally stored key would have added: nothing

The withdrawn design kept a 32-byte per-install key in the OS keychain
(or a 0600 file next to the ledger) and chained with
`HMAC-SHA256(key, entry || prev_hmac)`:

- **Against A1 — no protection.** The key lives on the same machine, in
  the same user profile. An attacker who can rewrite `ledger.jsonl` can
  read the key from the keychain (same user) and re-chain the whole file
  after editing — or simply change the key too. The 0600 fallback file
  was *next to the ledger*, protecting against even less.
- **Against A2 — no incremental protection.** Default profile ACLs
  already deny A2 write access to the file; where A2 *can* write (shared
  dirs), they cannot reach the per-user keychain — but neither can they
  legitimately get there to be stopped, and a chain verified against a
  signed export head catches them identically.
- **Against A3 — no protection, and none is possible locally.** Stated
  plainly: **no local scheme stops the file's owner from deleting or
  rewriting entries.** The key is theirs; the machine is theirs. HMAC
  did not change this; an export-time signature over the chain head is
  the only mechanism that makes *post-hoc* regeneration detectable,
  because the old head's signature stops matching.
- **Write-only adversary scenarios** (can write the file, cannot read
  the key) are the only theoretical HMAC win, and no realistic threat
  model produces that attacker on a desktop OS.

Conclusion: HMAC named no threat that the chain + export signature does
not cover, so per ruling E-4 it was removed (`Cargo.toml`, `Cargo.lock`,
separate commit) and the chain reverted to plain SHA-256.

## 4. Honest limitations (unchanged by any MAC)

1. **Deletion is undetectable.** A missing file cannot be distinguished
   from a never-created one. The count shown in the UI is the record.
2. **Whole-file regeneration by the owner is possible** until the export
   signature exists; after it exists, regeneration breaks *prior signed
   exports*, which is the best any local design can do.
3. **The current verifier does not check this file.** `verifier/` (the
   offline HTML verifier) validates provenance export packages only; it
   has no ledger parsing or ledger signature checking today. Making the
   exported ledger snapshot verifier-checkable is part of the Phase 4
   export work.
4. Clock skew affects `ts` ordering only; the chain does not depend on
   monotonic time.

## 5. Token and pairing (hardening round H1/H2, 2026-10-23)

Scope extended by the owner's hardening authorization to cover the
listener's two other secrets: the pairing code and the paired-client
bearer token.

**Pairing code (H1).** 6 characters from a 31-symbol unambiguous
alphabet (≈ 2^29.7 — strictly stronger than 6 decimal digits' 2^19.9),
drawn from the OS CSPRNG (`getrandom`) with rejection sampling so the
byte→index mapping is exactly uniform (256 % 31 == 8; the 8 highest
byte values are redrawn; every index is hit by exactly 8 byte values —
proven by test `pairing_code_mapping_has_no_modulo_bias`, not sampled).
The code is one-time, burns on success, and carries a GLOBAL budget of
5 wrong attempts (everything arrives from loopback, so per-IP budgets
are meaningless); the 5th failure burns the code. A new code requires
the explicit user action in the Word Assist tab, gated by a cooldown
that starts at 30 s and doubles per burn, capped at 30 min. A
route-specific limiter (10 req/60 s) sits on `/v1/pair` in addition to
the global 120/60 s budget. Audit events record success, failure and
lockout — never the code value itself.

**Paired-client token (H2).** 32 CSPRNG bytes, hex-encoded on the wire.
The engine persists ONLY `SHA-256(token)` + created_at / last_used /
label — keychain first (`scholarscribe`/`word-assist-client-token-v1`),
0600 file fallback in the app data dir on unix (audited), none on
Windows (keychain only). The plaintext token exists in exactly two
places: the /v1/pair response at issue time, and the add-in's
`localStorage`. **The owner ACCEPTED plaintext-in-localStorage** (same-
user trust boundary): any process that can read Word's webview profile
already runs as the user and could equally read the app's own memory;
nothing local is escalated by it. Revocation wipes memory + keychain +
file; rotation = revoke + fresh pairing code (the token is never pushed,
so this is the exact rotation shape); disabling the feature rotates
(revokes) the token. Sliding expiry: 30 days since last use, then the
record is revoked wherever it is next seen.

**What this does NOT defend against (honest limits).** A1 (same-user
malware) can read the token hash, the add-in's localStorage token, and
can use the token against the loopback listener while the app runs —
the trust boundary is the OS user, by design (D-3/4c). The sliding
expiry bounds the window if the profile is exfiltrated, but a stolen
*live* token is valid until revoked; there is no second factor.
