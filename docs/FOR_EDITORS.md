# For editors and integrity officers: what a ScholarScribe packet is, and is not

This one page explains what you may conclude from a ScholarScribe Writing
Provenance export packet (a signed `.zip` an author may send you), and what
you may not.

## What the packet is

A ScholarScribe packet is a ZIP archive the author produced with the
ScholarScribe desktop app. It contains:

- `manifest.json`: a machine-readable record of the document's tracked-change
  sessions (Word Track Changes, or Google Docs version history if the author
  imported a Google Doc), organized per author, hash-chained with SHA-256 and
  signed with an Ed25519 key. It contains hashes, timestamps, character
  counts, and author display names; it never contains document text.
- `disclosure.txt`: the author-side explanation of what the packet is.
- `citation_validation.json`: an inventory of citation-like patterns. This is
  a descriptive count, not a validity check.
- `README.txt`: verification instructions.
- `style_analysis.json` (only if the author opted in): aggregate
  style-consistency metrics. This is an uncalibrated, informational statistic
  and is explicitly not evidence. Its absence is the default.

## How to verify a packet offline

1. Open `verifier/index.html` from the ScholarScribe repository in any modern
   browser. It works fully offline.
2. Load the packet `.zip`.
3. Optionally load the original `.docx` so the verifier can bind the manifest
   to the document by hash.
4. If the author supplied their public key (a string starting with
   `ed25519-pub:`), paste it in. The verifier checks that the key's hash
   matches the fingerprint in the manifest and that the Ed25519 signature
   verifies. It also re-derives the whole per-author hash chain.

A valid signature proves: the packet's bytes have not changed since they were
produced by the holder of that signing key, and the internal hash chain is
intact.

## What the packet is NOT

- **It is not proof of authorship.** A signature proves package integrity and
  who produced the package, not who wrote the text.
- **It is not proof the timestamps are true.** Revision timestamps come from
  the document's own metadata (Word or Google). A determined actor with the
  original file could edit revision XML before analysis; the app flags only
  internal inconsistencies (for example, future-dated edits or broken
  chains).
- **It is not proof the text is human-written.** Nothing in the packet can
  distinguish human writing from AI-assisted writing that was tracked.
- **It is not a detector score.** No AI-detection score is included, and
  ScholarScribe deliberately contains no detection-evasion features.
- **Absence of a packet is never evidence against an author.** Not using
  ScholarScribe, deleting drafts, or writing in tools that keep no history
  says nothing about authenticity. Process evidence of this kind can only
  ever support an author's account; it can never be demanded as a gate.
- **The Writing Journal in the app is separate and unsigned.** If an author
  mentions the "Writing Process Journal": its snapshots are plain local JSON
  files with no signature and no chain. They are convenient recall for the
  author, not verifiable evidence.

## A note on AI-text detection scores

Independent evaluations (Liang et al. 2023, Patterns 4(7):100779;
Weber-Wulff et al. 2023, International Journal for Educational Integrity
19(1):26) find AI-text detectors are neither accurate nor reliable, with a
bias toward labelling text as human-written, and can systematically
misclassify writing by non-native English authors as AI-generated. Treat any
detector score as one weak signal; never as the sole basis for an integrity
finding. ScholarScribe's Detector Literacy tab exists to make this literature
easy to reach.
