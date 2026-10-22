# Installing Word Assist (v0.1 Preview) — verified per-OS walkthrough

Audience: the user installing Word Assist into Word. Every command below
is run BY YOU, never by ScholarScribe: the app never writes to the
certificate store, the registry, or any trust configuration. Sources for
each procedure are cited inline (all accessed 2026-10-23); the QA matrix
(`docs/QA_v0.1.md`) executes every step on a real machine before a release
claim is made.

The add-in manifest is exported by ScholarScribe (Word Assist tab →
"Export manifest file…"). It carries one stable add-in id
(`3f4c8d61-9a27-4b5e-b2c8-01d3e5f7a9b4`), so re-installing or re-exporting
never creates a duplicate add-in.

## What you are trusting, and what it can reach

ScholarScribe generates a per-install certificate for `localhost` /
`127.0.0.1` and serves the add-in over HTTPS on 127.0.0.1 (loopback only).
Trusting that certificate in YOUR user account lets Word's webview open
`https://localhost:<port>`. It grants nothing else: the certificate is
valid only for `localhost` and `127.0.0.1`, only you install it, and only
in your user account. (Hardening note, recorded in the release plan: the
certificate is currently a self-signed leaf with rcgen's default validity;
the temporary-CA model is a pending owner decision.)

## Windows (Word with Microsoft 365)

### 1. Trust the certificate (Current User store)

In the Word Assist tab, click **Copy certificate (PEM)** — or use the
`.der` file shown at "Certificate path" — save it as e.g.
`scholarscribe-wa.cer`, then in a normal Command Prompt:

```bat
certutil -user -addstore Root "C:\path\to\scholarscribe-wa.cer"
```

`-user` scopes the install to your account (Current User → Trusted Root
Certification Authorities). Alternative without a terminal: double-click
the certificate → Install Certificate → Current User → place in
"Trusted Root Certification Authorities".

**Verify:** `certutil -user -store Root | findstr /i "ScholarScribe"`
(lists the entry; the line after the name carries its hash).

**Revoke / uninstall:**
```bat
certutil -user -delstore Root "ScholarScribe Word Assist (localhost)"
```
(certutil accepts the common name, its serial number, or the SHA-1 hash
shown by the verify step as the identifier).

### 2. Sideload the manifest (trusted shared-folder catalog)

Microsoft's procedure for Windows (network-share catalog; this method is
Windows-only and intended for testing, per the Microsoft sideload page):

1. Export the manifest with the Word Assist tab button to a folder you
   will share (e.g. `C:\wa-addin`).
2. File Explorer → right-click that folder → Properties → Sharing →
   Share → share with yourself (Read/Write). Note the network path
   (`\\YOURPC\wa-addin`).
3. Word → File → Options → Trust Center → Trust Center Settings →
   Trusted Add-in Catalogs → Catalog Url = that network path →
   **Add catalog** → tick **Show in Menu** → OK, then restart Word.
4. Insert → My Add-ins → **SHARED FOLDER** tab → "ScholarScribe Word
   Assist" → Add.

**Verify:** the add-in appears under the SHARED FOLDER tab and opens a
task pane.

**Uninstall:** Insert → My Add-ins → SHARED FOLDER → right-click the
add-in → Remove; then remove the catalog in Trust Center (and optionally
unshare the folder).

### 3. Pair

Enable Word Assist in ScholarScribe, click "Generate pairing code", type
the 6-character code into the add-in pane, press Pair. Codes expire after
5 minutes and work once.

## macOS (Word for Mac — build ships "unvalidated")

### 1. Trust the certificate (login keychain)

```zsh
security add-trusted-cert -r trustRoot "/path/to/scholarscribe-wa.der"
```

Notes verified against the `security` command documentation:
- Without `-d` this modifies **per-user** trust settings; macOS shows an
  authentication dialog — that prompt is the operating system asking you,
  not ScholarScribe.
- The certificate file can be the `.der` from the Word Assist tab (saved
  to disk) — `add-trusted-cert` accepts DER or PEM.

**Verify:** `security find-certificate -c "ScholarScribe Word Assist"`
prints the keychain entry; Keychain Access shows it under "login".

**Revoke / uninstall:**
```zsh
security remove-trusted-cert "/path/to/scholarscribe-wa.der"
security delete-certificate -c "ScholarScribe Word Assist (localhost)"
```

### 2. Sideload the manifest (wef folder)

Microsoft's procedure for Office on Mac:

1. Export the manifest from the Word Assist tab to your Mac.
2. In Finder press `Cmd+Shift+G` and go to
   `/Users/<you>/Library/Containers/com.microsoft.Word/Data/Documents/wef`
   (create the `wef` folder if it does not exist).
3. Copy the manifest XML into that folder.
4. Quit and reopen Word, open a document, then Home → Add-ins — the
   add-in appears in the menu.

**Uninstall:** delete the manifest from the `wef` folder and clear the
Office cache (per Microsoft's "Clear the Office cache" page: remove the
`com.microsoft.Office.Word` container content per that page's procedure
for the add-in cache).

### 3. Pair — identical to Windows.

## Validation of the manifest itself (dev tooling)

CI runs a structural check on the manifest (required elements present,
GUID shape, port placeholder, LF-only) via
`scripts/check-addin-manifest.mjs`. Before a release, the owner should
additionally run Microsoft's own validator once, on a machine with Node:

```sh
npx office-addin-manifest validate ScholarScribe-Word-Assist-manifest.xml
```

`office-addin-manifest` is Microsoft's open-source dev tool (npm,
MIT). It is deliberately NOT added as a repo dependency (the v0.1 rule:
no new dependency without a proposal); this one-off local run is the
proposal, recorded here and in docs/RELEASE_PLAN_v0.1.md.

## Troubleshooting pointer

Certificate errors, port-in-use, keychain-denied, and pairing-lockout
symptoms are covered in the User Guide's troubleshooting section and in
docs/QA_v0.1.md's expected results.
