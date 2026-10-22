# Word Assist v0.1 — Owner QA matrix (D5)

How to use: run the steps in order on **Windows** (primary platform) and,
if a Mac is available, on **macOS**. Write what actually happened in the
"Recorded" line of each step — a step passes only when the recorded result
matches the expected result. Anything that does not match is a Stage C
find: one commit per reported failure, fixed before the release.

Environment block (fill before starting):

- OS / build: ______  Word version / build: ______  License: Microsoft 365 / perpetual: ______
- ScholarScribe build: ______  Word Assist port: ______  Date: ______
- Ollama present: yes / no  Tier 1 model (optional, for step A13): ______

Legend: each step cites the evidence class — [DOCS] the behaviour is
specified in the docs, [CODE] implemented and unit/golden-tested, [SPIKE]
depends on live Word behaviour the owner's spike run has not yet confirmed.

## Part A — Windows (Word with Microsoft 365)

### A1. Install the app
- Do: install the ScholarScribe Word Assist build (release candidate installer or `cargo tauri build` output). Launch it.
- Expected: the app opens; the Word Assist tab is present in the sidebar.
- Recorded: ______

### A2. Enable Word Assist
- Do: open the Word Assist tab, read the disclosure, accept it, click Enable.
- Expected: status shows the listener running on `https://127.0.0.1:<port>`; the port defaults to 38080. [CODE]
- Recorded: ______

### A3. Trust the certificate (CA trust)
- Do: in the Word Assist tab, copy the certificate (PEM) or note the `.der` path; run the Windows command from `docs/WORD_ASSIST_INSTALL.md` §Windows step 1: `certutil -user -addstore Root "<cert file>"`.
- Expected: certutil prints the store add; the verify command lists the ScholarScribe certificate. Record the hash it prints (needed for A19). [DOCS]
- Recorded: ______

### A4. Sideload the manifest (shared-folder catalog)
- Do: click "Export manifest file…", share the chosen folder, add it as a Trusted Add-in Catalog (Trust Center → Trusted Add-in Catalogs → Show in Menu), restart Word, Insert → My Add-ins → SHARED FOLDER → "ScholarScribe Word Assist".
- Expected: the add-in opens a task pane; the capability line shows WordApi 1.6/1.7/1.8 support per this Word build. [DOCS]
- Recorded: ______

### A5. Pairing
- Do: click "Generate pairing code" in the app, type the 6-character code into the pane, Pair.
- Expected: the pane shows "Connected to ScholarScribe (dev)"; the app shows the paired state. Wrong-length codes are refused client-side with "The pairing code is 6 characters." [CODE]
- Recorded: ______

### A6. Pairing failure, lockout, cooldown (record actual behaviour)
- Do: with a fresh code, type a WRONG 6-character code 5 times, then the correct code.
- Expected (preview behaviour, recorded in the release plan): wrong codes fail each time; there is **no** failed-attempt lockout beyond the listener's global 120-requests/60-seconds limit; the correct code still pairs; codes expire after 5 minutes (wait 5+ minutes with the pane open → pairing fails until a new code is generated); each generated code burns on success. Record whether the correct code still paired after 5 wrong attempts. [CODE][SPIKE]
- Recorded: ______

### A7. Grammar underline appears
- Do: in Word, type or paste a sentence with a clear grammar error, e.g. "The results indicates a strong trend." Pause typing for ~2 seconds.
- Expected: an underline/annotation appears on the flagged span (annotation mode), or the issue appears in the pane's list (fallback mode). The default profile flags grammar only — a misspelled word alone should NOT be flagged in this preview. [CODE][SPIKE: live Word behaviour]
- Recorded: ______

### A8. Popup accept applies exactly once
- Do: click the flagged annotation, choose a suggestion in the popup.
- Expected: the replacement is applied exactly once (the word changes once; no duplicate edit, no double application), the pane logs the accept, and the app's ledger count for accepted increases by 1. [CODE][SPIKE: who applies — (b)/(j)/(k)]
- Recorded: ______

### A9. Reject / ignore
- Do: flag another issue and dismiss the popup (or click Reject); also test ignoring an underline and typing elsewhere.
- Expected: the text is unchanged; the pane's ledger records the reject where the UI exposes reject (record what the UI offered). [CODE]
- Recorded: ______

### A10. Paragraph edits while typing (rapid typing)
- Do: place the cursor in a flagged paragraph and keep typing rapidly for ~10 seconds.
- Expected: no wrong-offset edits, no duplicated annotations; the last quiet period triggers one check; a stale verdict for pre-edit text is discarded (pane log may show "stale result discarded"). [CODE: unit-tested in word-addin]
- Recorded: ______

### A11. Document with Track Changes
- Do: turn Track Changes ON, make edits, and let the pane check.
- Expected: the pane keeps checking without crashing; record exactly what happens (annotations inside tracked ranges are part of spike items (m)); do not assume — record. [SPIKE]
- Recorded: ______

### A12. Table paragraph
- Do: insert a 2×2 table with sentences in cells; wait for a check.
- Expected: record what the pane does with table paragraphs (covered or not per spike (m)); no crash, no hang. [SPIKE]
- Recorded: ______

### A13. Non-English paragraph
- Do: paste a paragraph in a non-English language (e.g. Arabic or German).
- Expected: the pane shows "Language not supported yet (<script>). You can enable it experimentally in ScholarScribe → Word Assist." and does not flag. Enable the language experimentally in the app, re-check, and record what changes. [CODE]
- Recorded: ______

### A14. 80-page manuscript (latency and no hang)
- Do: open a large manuscript (~80 pages). Note pane responsiveness and app CPU/memory; time a full Re-check.
- Expected: Word stays responsive; the pane logs progress without freezing; record time-to-first-underlines and full-check duration. [SPIKE: real-world scale]
- Recorded: ______

### A15. Word closed mid-request
- Do: trigger a check and immediately close Word completely.
- Expected: ScholarScribe keeps running; re-enabling/re-opening Word and the pane re-pairs (or reuses the session) and checks again. Record whether the app logs errors. [CODE]
- Recorded: ______

### A16. Tier 1 rewrite (only if a model is configured)
- Do: set a model name in the Word Assist tab (Tier 1 becomes active), flag a sentence where the rewrite applies.
- Expected: a "Rewrite available" chip appears (list/annotation UI per mode); applying it replaces the sentence once; citations/numbers/URLs in the sentence are untouched by construction (validator). [CODE][SPIKE: live model behaviour]
- Recorded: ______

### A17. Firewall prompt
- Do: enable Word Assist and watch for a Windows Firewall prompt.
- Expected: **none** — the listener binds 127.0.0.1 (loopback) only. If ANY firewall prompt appears, that is a release blocker: record the exact prompt text. [CODE: bind address is 127.0.0.1]
- Recorded: ______

### A18. App disable and revoke
- Do: click Disable in the Word Assist tab; try the pane again in Word.
- Expected: the listener stops (pane shows unreachable); the pairing is wiped; re-enabling requires a fresh pairing code. [CODE]
- Recorded: ______

### A19. Uninstall cleanup
- Do: remove the add-in (My Add-ins → Remove; clear the trusted catalog), then `certutil -user -delstore Root "ScholarScribe Word Assist (localhost)"`, then verify with `certutil -user -store Root | findstr /i "ScholarScribe"`.
- Expected: the add-in is gone from Word; the findstr returns nothing. Record anything left behind. [DOCS]
- Recorded: ______

## Part B — macOS (run only if a Mac is available; otherwise these rows stay NOT VERIFIED and the Mac build ships labelled "unvalidated")

> Do not infer any Mac result from Windows. Every row records what macOS
> actually did.

### B1. Install and launch
- Expected: the app opens; Word Assist tab present. (The build is unsigned: right-click → Open on first launch is the documented Gatekeeper path.)
- Recorded: ______

### B2. Trust the certificate
- Do: `security add-trusted-cert -r trustRoot "<cert .der>"`.
- Expected: an **authentication dialog** appears (per-user trust always prompts on macOS) — that prompt is the OS, not the app. After confirming, `security find-certificate -c "ScholarScribe Word Assist"` lists it. [DOCS]
- Recorded: ______

### B3. Sideload (wef folder)
- Do: copy the exported manifest to `/Users/<you>/Library/Containers/com.microsoft.Word/Data/Documents/wef` (create the folder), restart Word, Home → Add-ins.
- Expected: the add-in appears in the menu. [DOCS]
- Recorded: ______

### B4. Pair and check
- Expected: pairing works as on Windows; grammar flags appear per this Word build's capability probe (record the caps line verbatim). [SPIKE]
- Recorded: ______

### B5. macOS keychain prompt behaviour
- Do: run with the ledger enabled and, separately, deny the keychain prompt once.
- Expected: allowing → keys live in the keychain; denying → the app falls back to a 0600 file and logs the disclosure (record the audit line). [CODE]
- Recorded: ______

### B6. Uninstall (macOS)
- Do: delete the manifest from `wef`, `security remove-trusted-cert <cert>` + `security delete-certificate -c "ScholarScribe Word Assist (localhost)"`, clear the Office cache.
- Expected: add-in gone; cert lookup empty. [DOCS]
- Recorded: ______

## Results summary (fill after both parts)

| Area | Result (pass / fail / not run) | Findings |
|---|---|---|
| Install + enable | ______ | ______ |
| Certificate trust + revoke | ______ | ______ |
| Sideload + uninstall | ______ | ______ |
| Pairing + expiry + failure behaviour | ______ | ______ |
| Grammar flags (annotation / list mode) | ______ | ______ |
| Apply exactly once / reject | ______ | ______ |
| Rapid typing / stale discard | ______ | ______ |
| Track Changes / tables / non-English | ______ | ______ |
| Large manuscript latency | ______ | ______ |
| Word closed mid-request | ______ | ______ |
| Firewall prompt (expect none) | ______ | ______ |
| Tier 1 rewrite (if configured) | ______ | ______ |
| macOS rows (if run) | ______ | ______ |

When every row above is recorded, paste this file (and the pane's audit
export) back to the agent as Stage C input.
