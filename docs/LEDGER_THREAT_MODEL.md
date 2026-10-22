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
