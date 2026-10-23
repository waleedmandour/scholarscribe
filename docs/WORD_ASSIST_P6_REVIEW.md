# P6 — Windows/macOS desk review (never compiled on either platform here)

Status: desk review + low-risk fixes, per the owner's Gate-1 item P6. Every
statement below carries an evidence tag. **Nothing in this document or its
commits has been compiled or run on Windows or macOS** — the sandbox is Linux.
The Windows-only test lands in CI's `windows-latest` legs (D1 pending).

## 1. CA trust / uninstall / sideload commands for hostile paths

The Gate-1 instruction text embedded the commands as string literals. P6
replaced them with **pure builder functions** (commit `149deaf`), so the
displayed commands are now *generated* with per-shell quoting that is
unit-tested on the owner's named inputs:

| Builder | Shell it targets | Output shape |
|---|---|---|
| `ca_trust_command_windows` | **cmd.exe** | `certutil -user -addstore Root "<dir>\word-assist-ca.der"` |
| (PowerShell variant inline in the registration text) | **PowerShell** | `certutil -user -addstore Root '<dir>\word-assist-ca.der'` (single-quoted) |
| `ca_trust_command_macos` | **zsh/bash** | `security add-trusted-cert -r trustRoot '<dir>/word-assist-ca.der'` |
| `ca_revoke_command_windows` | **cmd.exe** | `certutil -user -delstore Root "<CA CN>"` |
| `ca_revoke_command_macos` | **zsh/bash** | `security remove-trusted-cert 'word-assist-ca.der' && security delete-certificate -c "<CA CN>"` |
| `sideload_copy_command_macos` (attached to the manifest-export result) | **zsh/bash** | `mkdir -p $HOME/Library/Containers/com.microsoft.Word/Data/Documents/wef && cp '<manifest>' $HOME/.../wef/manifest.xml` |

Quoting helpers: `quote_windows_cmd` (double quotes), `quote_powershell`
(single quotes, inner `'` doubled), `quote_posix` (single quotes, `'\''`
re-entry). Tests (`assist_commands::tests::p6_*`, VERIFIED passing locally on
Linux, 177/177):

- spaces: `C:\Users\John Doe\...` and `Application Support` — quoting holds.
- apostrophe: `/Users/O'Brien/...` — POSIX round-trips exactly
  (`'\''` re-entry; test strips the quoting and compares); PowerShell doubles
  it (`O''Brien`).
- non-ASCII: `C:\Users\مستخدم\...` and `/Users/José/...` — verbatim UTF-8
  passthrough, no escaping damage.
- long paths: 40 × ~38-char segments (>1200 chars) — no truncation, still
  quoted.
- no-elevation shape asserted on every command: starts with
  `certutil -user`, contains no `-d ` (admin domain), no `sudo`, no
  `-enterprise`/`-gpo`.

### Elevation verification against vendor documentation

- **Windows `certutil -user`** — VERIFIED (Microsoft Learn,
  https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/certutil,
  fetched 2026-10-24): "-addstore … Options: [-f] [-Enterprise] [-user]
  [-GroupPolicy] [-dc DCName]" and, for the shared store options, "**The
  -user option accesses a user store instead of a machine store.**" The
  machine/enterprise/group-policy variants are the privileged ones; `-user`
  scopes to the caller's own profile. That the per-user write needs no
  elevation is the documented store scoping (INFERRED as "no elevation" from
  the documented user-vs-machine split; the instruction text also never
  suggests an admin prompt).
- **macOS `security add-trusted-cert`** — VERIFIED (man page text via
  https://www.unix.com/man-page/osx/1/security/, fetched 2026-10-24):
  "Add certificate (in DER or PEM format) from certFile to **per-user or
  local Admin Trust Settings**. When modifying **per-user Trust Settings,
  user authentication is required via an authentication dialog**. When
  modifying admin Trust Settings, the process must be running as root, or
  admin authentication is required. Options: **-d Add to admin cert store;
  default is user.** -r resultType … default is trustRoot." Our command
  omits `-d` and passes `-r trustRoot` → per-user trust with the OS auth
  dialog — exactly what the app's instruction text says ("macOS shows an
  authentication dialog… that prompt is the OS, not ScholarScribe").
  `remove-trusted-cert` follows the same user-default domain on the same
  page. **No `-d`, no `sudo` anywhere.**
- The app itself never executes any of these (settled ruling: no trust-store
  writes) — VERIFIED: the builders only return strings; no `Command::new`
  call sites reference certutil/security (grep evidence).

## 2. cfg audit — every platform branch in `src-tauri/src`

Grep (VERIFIED): `cfg(windows)`, `cfg(target_os`, `cfg(target_arch` have **zero**
matches in `src/`. The complete set of platform branches:

| Site | Branch | Reviewed verdict |
|---|---|---|
| `src/assist/server.rs:1154` | `#[cfg(unix)]` | TLS leaf key read from the 0600 file fallback; guarded by `path.exists()`; audit event records the path. OK. |
| `src/assist/server.rs:1175` | `#[cfg(not(unix))]` | `Ok(None)` — Windows: no file-fallback read. OK (refusal happens at generation, below). |
| `src/assist/server.rs:1257` | `#[cfg(unix)]` | write 0600 fallback + audit event. OK. |
| `src/assist/server.rs:1273-1282` | `#[cfg(not(unix))]` | **Refusal with user-facing message** (see §3). OK. |
| `src/assist/settings.rs:509` | `#[cfg(unix)]` | `write_private` sets 0600 via `PermissionsExt`. OK. |
| `src/assist/settings.rs:519` | `#[cfg(not(unix))]` | `Err("private-file fallback is refused on this platform (keychain required)")`. OK — now tested (§3). |

No `cfg(target_os = "macos"|"windows")` special-casing exists; portability
rides on `std` + the `keyring` facade. **Finding: none.**

## 3. Windows keychain-failure path (file fallback refused)

Flow (VERIFIED, `src/assist/server.rs:1250-1282`): on generation, the leaf
key goes to the keychain via `keyring::Entry::set_password`; if that fails,
unix writes the 0600 file fallback **with an audit event**, while Windows hits
`#[cfg(not(unix))]` and returns the error:

> "the OS keychain (Windows Credential Manager) is required to store the Word
> Assist TLS key; the unencrypted file fallback is refused on Windows"

(`server.rs:1277-1281`). This propagates out of `ensure_certificate_chain` →
`start_with_limits` → the enable path, i.e. it is **user-visible as the
enable-failure message**, not a silent log. VERIFIED by reading the error path.

**Test through a seam (commit `149deaf`):** `settings::tests::
p6_write_private_refused_on_windows` (`#[cfg(not(unix))]`) drives the shared
`write_private` seam and asserts the refusal message contains "refused" and
"keychain" and that **no file is created**. It compiles only on Windows and
will therefore run on the CI `windows-latest` legs (D1 pending — labelled
NEEDS HUMAN VERIFICATION until a CI run proves it). The unix mirror
`p6_write_private_uses_0600_on_unix` passes locally (VERIFIED).

## 4. File locking / temp dirs, CRLF, path separators, case-insensitivity

- **File locking when tests delete temp dirs:** no file-locking API exists in
  the codebase (no `flock`/`fs2`/`file_guard`/`LockFile` matches). The audit
  log is in-memory only (`src/audit.rs:4`); persistence writes JSON files
  without advisory locks. Tests remove their temp dirs at the end via
  `remove_dir_all`; nothing holds open handles into them afterwards (each
  write opens/closes). VERIFIED by grep + reading the two writer sites.
  **Finding: none.**
- **`\r\n` in expected strings:** the Rust tests do not compare multi-line
  file-derived content that could differ per checkout: the two XML readers
  parse from strings (XML parser normalises line ends); the frozen eval/golden
  files are byte-frozen with `.gitattributes eol=lf` coverage from commit
  `3328ea6` (H9, verified across autocrlf modes at Gate 1). The new P6 tests
  use `\n` only inside PEM/base64 assertions which are generated, not
  checked-out files. **Finding: none** (Windows CI legs re-prove under D1).
- **Path separators:** zip entry names are compared against the crate's
  normalized `/` form (`entry.name() == "word/document.xml"`) — correct on all
  platforms; filesystem paths are built with `std::path::Path::join` — no
  hardcoded `/` outside URLs. VERIFIED by grep (`join("/")` → no matches).
  **Finding: none.**
- **Case-insensitive filesystems:** the app writes exactly three files into
  the assist data dir — `word-assist-ca.der`, `word-assist-cert.der`,
  `word-assist-key.p8` (`server.rs:1047` + `CA_CERT_FILE`/`LEAF_CERT_FILE`
  consts) — plus the settings JSON; no two names differ only by case, so
  APFS (insensitive) and ext4 (sensitive) behave identically. Temp-dir test
  prefixes are distinct per suite. **Finding: none.**
- **Instruction strings are display-only:** the builders' outputs are never
  executed by the app (settled ruling; grep evidence in §1).

## 5. Runner labels (re-verified)

`windows-latest`, `macos-latest` (arm64), `macos-15-intel` (x86_64),
`ubuntu-latest` — labels already verified against GitHub's runner-images docs
on 2026-10-23 per the workflow header comment (A1); the P5 commit did not
change any `runs-on:` label. NEEDS HUMAN VERIFICATION remains for the
labels' continued availability on GitHub's side (they can retire images;
re-check before Stage C).

## 6. What was changed by P6 (commits)

- `149deaf` — builders + hostile-path tests + write_private seam tests +
  CA_COMMON_NAME single source (label: compiled + tested locally on Linux).
- No cfg-branch behaviour changes were needed: every reviewed branch matched
  the hardened rulings (the two Windows refusals are intentional).
