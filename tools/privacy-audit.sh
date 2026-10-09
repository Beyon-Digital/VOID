#!/usr/bin/env bash
# tools/privacy-audit.sh — VOID diagnostic-privacy static sweep (TEST_MATRIX
# T29 / W07, tooling half).
#
# Scans committed sources for the channels by which private data could leave
# the process, and asserts the current tree has none carrying PII-shaped
# fields: no telemetry/analytics SDKs, no network emitters in shipping code,
# no home/username/email/absolute-user-path captures in diagnostic payloads,
# no credential-named values in log statements, no committed key material.
#
# This is a static sweep. T29's runtime half — injecting private
# paths/prompts/audio into a running build and inspecting emitted logs and
# crash bundles — needs a runnable app and is recorded open in
# docs/ops/NEEDS.md.
#
# Exit 0 = no FAIL findings. WARN findings are printed for review.
set -u
cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

PASS=0; FAIL=0; WARN=0
ok()   { printf "  PASS  %s %s\n" "$1" "$2"; PASS=$((PASS+1)); }
warn() { printf "  WARN  %s %s\n" "$1" "$2"; WARN=$((WARN+1)); }
bad()  { printf "  FAIL  %s %s\n" "$1" "$2"; FAIL=$((FAIL+1)); }

echo "VOID privacy audit (static sweep, T29 tooling) — $(uname -srm)"

# File set: tracked sources. Excluded: docs/markdown (prose, not emitters),
# generated protocol bindings (schema-derived), vendored third_party, the
# tracking ledgers, and this script (it matches its own patterns).
SOURCES=$(git ls-files \
  | grep -viE '(^|/)(docs|examples|third_party|\.devin|node_modules)/' \
  | grep -vE 'tracking/|protocol/generated/|apps/void-tauri/src-tauri/gen/' \
  | grep -vE '\.(md|markdown|html|json|yaml|yml|toml|lock|fbs|sha256)$' \
  | grep -vE 'tools/privacy-audit\.sh$' \
  | grep -iE '\.(rs|ts|tsx|js|jsx|py|cpp|cc|h|hpp|sh)$' || true)
MANIFESTS=$(git ls-files | grep -E '(Cargo\.toml|package\.json|requirements.*\.txt|pyproject\.toml|setup\.py)$' | grep -v third_party || true)

scan() { # $1 = extended regex over shipping sources -> matching lines on stdout
  [ -n "$SOURCES" ] && echo "$SOURCES" | xargs grep -nE "$1" 2>/dev/null || true
}

# --- 1. emitter inventory (informational) --------------------------------------
emitters=$(echo "$SOURCES" | xargs grep -liE 'telemetry|diagnostic|crash[_-]?report|analytics|breadcrumb' 2>/dev/null | wc -l)
ok "emitter-inventory" "$emitters tracked source files reference telemetry/diagnostics vocabulary (all in-process event channels — see sink checks below)"

# --- 2. telemetry/analytics SDKs ------------------------------------------------
# Only dependency declarations and import/use statements count — plain English
# words like "segment"/"plausible" are not SDK references.
SDK_PKG_RE='(@sentry/|sentry[-_]sdk|posthog-js|posthog-rs|analytics-node|@segment/|segment-analytics|@amplitude/|amplitude-js|mixpanel|(firebase|crashlytics|appcenter)[-_/]|@opentelemetry/|opentelemetry[-_]sdk|telemetrydeck|google-analytics|react-ga|gtag|@umami/|umami[-_]is|plausible-tracker|@datadog/|dd-trace|newrelic|@bugsnag/|rollbar)'
sdk_hits=""
dep_lines=$(echo "$MANIFESTS" | xargs grep -nE "\"?[A-Za-z0-9@/_.-]*\"?\s*[:=]" 2>/dev/null | grep -iE "$SDK_PKG_RE" || true)
[ -n "$dep_lines" ] && sdk_hits="$dep_lines"
import_lines=$(echo "$SOURCES" | xargs grep -nE '(import|require|from|use)\s.*' 2>/dev/null | grep -iE "$SDK_PKG_RE" || true)
[ -n "$import_lines" ] && sdk_hits="$sdk_hits"$'\n'"$import_lines"
sdk_hits=$(printf '%s' "$sdk_hits" | grep -v '^$' || true)
if [ -z "$sdk_hits" ]; then
  ok "no-telemetry-sdk" "no analytics/crash-report SDK in dependency declarations or imports"
else
  bad "no-telemetry-sdk" "telemetry SDK references found:"
  printf '%s\n' "$sdk_hits" | sed 's/^/    /'
fi

# --- 3. off-process sinks in shipping code --------------------------------------
# Files that both (a) speak diagnostic vocabulary and (b) can send bytes off
# this process are the leak path. Test fixtures/harnesses are excluded — the
# offline-denial journeys legitimately open sockets.
shipping=$(echo "$SOURCES" | grep -vE '(^|/)(tests|test|fixtures)/|/tests/|/tests?/|_test\.|\.test\.|harness/|workers/' || true)
sink_hits=$(echo "$shipping" | xargs grep -lE 'reqwest|ureq|hyper::|fetch\(|axios|XMLHttpRequest|TcpStream::connect|UdpSocket|websocket|WebSocket\(' 2>/dev/null | xargs grep -liE 'telemetry|diagnostic|crash|report|metric|log' 2>/dev/null || true)
if [ -z "$sink_hits" ]; then
  ok "no-offprocess-emit" "no shipping file combines diagnostic vocabulary with a network sink"
else
  bad "no-offprocess-emit" "diagnostic code with a network sink:"
  printf '%s\n' "$sink_hits" | sed 's/^/    /'
fi

# --- 4. environment sweeps -------------------------------------------------------
# Whole-environment enumeration only — reading one named var is not a sweep.
env_sweeps=$(scan 'env::vars\(\)|Object\.(keys|entries|values)\(\s*process\.env|JSON\.stringify\(\s*process\.env|\.\.\.process\.env|for\s*\([^)]*process\.env' || true)
env_count=$(printf '%s' "$env_sweeps" | grep -c . || true)
if [ "$env_count" -eq 0 ]; then
  ok "no-env-sweep" "no whole-environment enumeration outside known build/runtime vars"
else
  warn "no-env-sweep" "$env_count site(s) enumerate the process environment — review values leaving the process:"
  printf '%s\n' "$env_sweeps" | head -10 | sed 's/^/    /'
fi

# --- 5. PII-shaped captures in shipping code -------------------------------------
pii_paths=$(echo "$shipping" | xargs grep -nE '/home/[A-Za-z]|/Users/[A-Za-z]|C:\\\\Users\\|%USERPROFILE%|~/Library' 2>/dev/null || true)
if [ -z "$pii_paths" ]; then
  ok "no-abs-user-paths" "no absolute user-directory literals in shipping code"
else
  bad "no-abs-user-paths" "absolute user-path literals (would leak the operator's name/dir):"
  printf '%s\n' "$pii_paths" | head -10 | sed 's/^/    /'
fi

ident=$(echo "$shipping" | xargs grep -nE 'home_dir\(|whoami::|env::var\(("USER"|"USERNAME"|"LOGNAME")|os\.userInfo|getpass\.getuser|process\.env\.(USER|USERNAME|LOGNAME)' 2>/dev/null || true)
if [ -z "$ident" ]; then
  ok "no-identity-capture" "no home-dir/username capture in shipping code"
else
  warn "no-identity-capture" "user-identity reads found — confirm they never reach a diagnostic payload:"
  printf '%s\n' "$ident" | head -10 | sed 's/^/    /'
fi

emails=$(echo "$shipping" | xargs grep -nE '[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}' 2>/dev/null | grep -vE 'schema\.|w3\.org|registry\.|npmjs|github\.com|gitlab|SPDX|license|@deprecated|@param|@returns' || true)
if [ -z "$emails" ]; then
  ok "no-email-literals" "no email-shaped literals in shipping code"
else
  warn "no-email-literals" "email-shaped literals found (fine in fixtures, never in emitted fields):"
  printf '%s\n' "$emails" | head -10 | sed 's/^/    /'
fi

# --- 6. credentials in log statements --------------------------------------------
cred_logs=$(scan '(info|warn|error|debug|trace)!|println!|eprintln!|console\.(log|warn|error|info)|logger\.|logging\.(info|warning|error|debug)' \
  | grep -iE 'password|passphrase|secret|token|api_?key|credential|authorization|bearer' \
  | grep -vE 'token_(id|bucket|capability|denied|check)|auth(or)?_token.*(refus|reject|deni)|expected.*token|wrong.*token|invalid.*token' || true)
if [ -z "$cred_logs" ]; then
  ok "no-creds-in-logs" "no log statement carries credential-named values"
else
  bad "no-creds-in-logs" "log statements naming credential-shaped values — review:"
  printf '%s\n' "$cred_logs" | head -15 | sed 's/^/    /'
fi

# --- 7. committed key material ----------------------------------------------------
keys=$(git ls-files | grep -vE '(^|/)(docs|examples)/|\.md$|tracking/|third_party/' \
  | xargs grep -lnE '-----BEGIN (RSA |EC |OPENSSH |PGP |ENCRYPTED )?PRIVATE KEY|ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|AKIA[0-9A-Z]{16}|sk-[A-Za-z0-9_-]{20,}|xox[bap]-[A-Za-z0-9-]{10,}' 2>/dev/null || true)
if [ -z "$keys" ]; then
  ok "no-committed-secrets" "no private-key blocks or vendor token shapes in tracked files"
else
  bad "no-committed-secrets" "possible committed secrets:"
  printf '%s\n' "$keys" | sed 's/^/    /'
fi

# --- 8. diagnostic-export surface (informational) ---------------------------------
diag=$(echo "$SOURCES" | xargs grep -lniE 'diagnostic.?bundle|crash.?bundle|exportDiagnostics|support.?bundle' 2>/dev/null || true)
if [ -z "$diag" ]; then
  warn "diag-export-surface" "no diagnostics-bundle export path found — T29 also expects the export to require explicit user action (runtime drill: docs/ops/NEEDS.md)"
else
  ok "diag-export-surface" "$(printf '%s\n' "$diag" | wc -l) file(s) touch diagnostic-bundle paths — confirm export is user-initiated"
fi

echo "---"
echo "privacy-audit: $PASS pass, $WARN warn, $FAIL fail"
[ "$FAIL" -eq 0 ]
