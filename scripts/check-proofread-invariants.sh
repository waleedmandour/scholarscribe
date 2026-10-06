#!/usr/bin/env bash
# scripts/check-proofread-invariants.sh
# Phase 1+2b+3 invariant checks for the Proofread feature (v2.3.0).
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"
FAIL=0
log() { echo "[check-proofread-invariants] $*"; }
fail() { echo "[check-proofread-invariants] FAIL: $*" >&2; FAIL=1; }
ok() { echo "[check-proofread-invariants] ok: $*"; }

log "H1: checking Tier 1 files for networking references..."
TIER1_FILES=("src-tauri/src/grammar.rs" "src-tauri/src/grammar_commands.rs" "src-tauri/src/text_masking.rs" "src/components/Proofread.svelte")
for f in "${TIER1_FILES[@]}"; do
  if [ ! -f "$f" ]; then fail "expected file not found: $f"; continue; fi
  if rg -n 'reqwest|hyper::|ureq|curl::|isahc|surf::|attohttpc|tokio::net|std::net::|http::Client|fetch\(' "$f"; then
    fail "$f references a networking crate or function (H1 violation)"
  fi
done
# tier2.rs is ALLOWED to reference reqwest (reuses Ollama client at 127.0.0.1:11434).
ok "H1: Tier 1 files checked"

log "H2: checking Tier 1 files for detector-module coupling..."
FORBIDDEN_PATTERNS=(
  'use crate::style[^_]' 'use crate::risk_profiler' 'use crate::style_fingerprint'
  'use crate::voice_consistency' 'use crate::appeal_letter'
  'crate::style::StyleProfile' 'crate::risk_profiler::RiskProfile'
  'crate::style_fingerprint::StyleFingerprint' 'crate::voice_consistency::ConsistencyReport'
)
FORBIDDEN_FRONTEND=('RiskProfiler' 'StyleAnalysis' 'StyleFingerprint' 'VoiceConsistency' 'DetectorLiteracy' 'AppealLetter')
for f in "src-tauri/src/grammar.rs" "src-tauri/src/grammar_commands.rs" "src-tauri/src/text_masking.rs" "src-tauri/src/tier2.rs"; do
  for pat in "${FORBIDDEN_PATTERNS[@]}"; do
    if rg -n "$pat" "$f"; then fail "$f references forbidden symbol: $pat (H2 violation)"; fi
  done
done
for f in "src/components/Proofread.svelte"; do
  for pat in "${FORBIDDEN_FRONTEND[@]}"; do
    if rg -nw "$pat" "$f"; then fail "$f imports forbidden component: $pat (H2 violation)"; fi
  done
done
ok "H2: detector-coupling checked"

log "CSP: checking tauri.conf.json CSP is unchanged..."
EXPECTED_CSP='default-src '\''self'\''; img-src '\''self'\'' data:; style-src '\''self'\'' '\''unsafe-inline'\''; script-src '\''self'\'' '\''unsafe-inline'\''; connect-src '\''self'\'' http://127.0.0.1:11434 http://localhost:11434'
ACTUAL_CSP=$(python3 -c "
import json
with open('src-tauri/tauri.conf.json') as f:
    conf = json.load(f)
print(conf.get('app', {}).get('security', {}).get('csp', ''))
")
if [ "$ACTUAL_CSP" != "$EXPECTED_CSP" ]; then
  fail "tauri.conf.json CSP changed from baseline."
  fail "  expected: $EXPECTED_CSP"
  fail "  actual:   $ACTUAL_CSP"
else
  ok "CSP: unchanged from baseline"
fi

log "H5: checking ledger types for free-text fields..."
ALLOWED_STRING_FIELDS_REGEX='engine_name|engine_version|generator'
VIOLATIONS=""
CURRENT_STRUCT=""
in_target=0
while IFS= read -r line; do
  if echo "$line" | grep -qE '^pub struct (LedgerFinding|LedgerDecision|LedgerSummary) '; then
    CURRENT_STRUCT=$(echo "$line" | grep -oE '(LedgerFinding|LedgerDecision|LedgerSummary)'); in_target=1; continue
  fi
  if [ "$in_target" -eq 1 ] && echo "$line" | grep -qE '^\}'; then in_target=0; CURRENT_STRUCT=""; continue; fi
  if [ "$in_target" -eq 1 ]; then
    field=$(echo "$line" | grep -oE 'pub [a-z_]+: String,' | sed -E 's/pub ([a-z_]+): String,/\1/' || true)
    if [ -n "$field" ]; then
      if ! echo "$field" | grep -qE "^($ALLOWED_STRING_FIELDS_REGEX)$"; then
        VIOLATIONS="$VIOLATIONS\n  - $CURRENT_STRUCT.$field"
      fi
    fi
  fi
done < src-tauri/src/grammar.rs
if [ -n "$VIOLATIONS" ]; then
  fail "ledger structs contain free-text String fields (H5 violation):"
  echo -e "$VIOLATIONS" >&2
else
  ok "H5: ledger structs contain no free-text String fields"
fi

echo
if [ "$FAIL" -eq 0 ]; then log "ALL INVARIANTS HOLD"; exit 0; else log "INVARIANT VIOLATIONS DETECTED"; exit 1; fi
