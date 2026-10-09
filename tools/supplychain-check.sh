#!/usr/bin/env bash
# tools/supplychain-check.sh — VOID supply-chain audit (TEST_MATRIX T04 / W01).
#
# Verifies, from committed sources only:
#   1. flatc pin matches tools/protocol-gen.sh (REQUIRED_FLATC).
#   2. Cargo dependency sources: no git deps outside the allowlist
#      (SUPPLYCHAIN_GIT_ALLOWLIST, space-separated URLs, default empty),
#      no non-registry sources, no wildcard version requirements.
#   3. Lockfile freshness: `cargo metadata --locked` (Cargo.lock covers every
#      manifest) and `pnpm install --frozen-lockfile --lockfile-only`
#      (pnpm-lock.yaml covers every package.json).
#   4. SQLite linked build: every rusqlite dependency must use the `bundled`
#      feature; reports the vendored amalgamation's SQLITE_VERSION from the
#      resolved libsqlite3-sys source (NOT the host's sqlite CLI — a system
#      binary says nothing about the bundled library, per T04's expectation).
#
# Exit 0 = all checks pass. Warnings are printed but do not fail the run.
set -u
cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

PASS=0; FAIL=0; WARN=0
ok()   { printf "  PASS  %s %s\n" "$1" "$2"; PASS=$((PASS+1)); }
warn() { printf "  WARN  %s %s\n" "$1" "$2"; WARN=$((WARN+1)); }
bad()  { printf "  FAIL  %s %s\n" "$1" "$2"; FAIL=$((FAIL+1)); }

echo "VOID supply-chain check — $(uname -srm)"

# --- 1. flatc pin -------------------------------------------------------------
want_flatc=$(grep -m1 '^REQUIRED_FLATC=' tools/protocol-gen.sh | cut -d'"' -f2)
if [ -z "$want_flatc" ]; then
  bad "flatc-pin" "could not read REQUIRED_FLATC from tools/protocol-gen.sh"
elif command -v flatc >/dev/null; then
  got_flatc=$(flatc --version | awk '{print $3}')
  if [ "$got_flatc" = "$want_flatc" ]; then
    ok "flatc-pin" "$got_flatc (protocol-gen.sh REQUIRED_FLATC)"
  else
    bad "flatc-pin" "want $want_flatc, got $got_flatc"
  fi
else
  bad "flatc-pin" "flatc not on PATH (want $want_flatc)"
fi

# --- 2. cargo dependency source audit -----------------------------------------
# Reads the resolved graph; does not invent a registry hit where a lockfile
# answer exists (cargo metadata honours --locked before consulting the index).
if ! command -v cargo >/dev/null; then
  bad "cargo-metadata" "cargo not on PATH"
elif ! command -v python3 >/dev/null; then
  bad "cargo-metadata" "python3 not on PATH (needed to parse cargo metadata)"
else
  meta_file=$(mktemp)
  trap 'rm -f "$meta_file"' EXIT
  ok_meta=0
  for flags in "--locked --offline" "--locked"; do
    if cargo metadata --format-version 1 $flags >"$meta_file" 2>/dev/null; then
      ok_meta=1; break
    fi
  done
  if [ "$ok_meta" -eq 0 ]; then
    bad "cargo-metadata" "cargo metadata --locked failed (lockfile out of date?)"
  else
    META_FILE="$meta_file" python3 - <<'PY'
import json, os, sys
meta = json.load(open(os.environ["META_FILE"]))
allowlist = set(os.environ.get("SUPPLYCHAIN_GIT_ALLOWLIST", "").split())
git_deps, other_deps, n_reg, n_path = [], [], 0, 0
for p in meta["packages"]:
    src = p.get("source")
    if src is None:
        n_path += 1
    elif "crates.io" in src or "index.crates.io" in src:
        n_reg += 1
    elif src.startswith("git+"):
        url = src[4:].split("?")[0].split("#")[0]
        if url in allowlist:
            n_reg += 1
        else:
            git_deps.append(f"{p['name']} {p['version']} <- {src}")
    else:
        other_deps.append(f"{p['name']} {p['version']} <- {src}")
print(f"resolved: {n_reg} registry, {n_path} path/workspace")
for line in git_deps:
    print(f"GIT_DEP {line}")
for line in other_deps:
    print(f"OTHER_DEP {line}")
sys.exit(2 if git_deps or other_deps else 0)
PY
    rc=$?
    if [ $rc -eq 0 ]; then
      ok "cargo-dep-sources" "all resolved deps are crates.io or workspace paths"
    else
      bad "cargo-dep-sources" "non-registry deps found (see GIT_DEP/OTHER_DEP lines above; allow via SUPPLYCHAIN_GIT_ALLOWLIST)"
    fi
  fi
fi

# --- 2b. wildcard / floating version requirements in manifests -----------------
wildcards=$(grep -rn --include=Cargo.toml -E 'version\s*=\s*"\*"' . 2>/dev/null | grep -v '^\./target/' || true)
if [ -z "$wildcards" ]; then
  ok "no-wildcard-versions" "no version=\"*\" in any Cargo.toml"
else
  bad "no-wildcard-versions" "wildcard version requirements found:"
  printf '%s\n' "$wildcards" | sed 's/^/    /'
fi

# --- 3. lockfile freshness ------------------------------------------------------
if git ls-files --error-unmatch Cargo.lock >/dev/null 2>&1; then
  ok "cargo-lock-committed" "Cargo.lock is tracked"
else
  bad "cargo-lock-committed" "Cargo.lock is not committed"
fi
# --locked above already proves Cargo.lock covers the workspace manifests.
if command -v pnpm >/dev/null; then
  if pnpm install --frozen-lockfile --lockfile-only >/dev/null 2>&1; then
    ok "pnpm-lock-fresh" "pnpm-lock.yaml satisfies every package.json"
  else
    bad "pnpm-lock-fresh" "pnpm install --frozen-lockfile --lockfile-only failed"
  fi
else
  warn "pnpm-lock-fresh" "pnpm not on PATH — lockfile freshness unverified"
fi

# --- 4. SQLite linked build -----------------------------------------------------
sqlite_users=$(grep -rln --include=Cargo.toml 'rusqlite' crates/ apps/ 2>/dev/null || true)
missing_bundled=""
for f in $sqlite_users; do
  # A rusqlite dep line without the bundled feature links the host libsqlite3.
  if ! grep -E 'rusqlite.*bundled' "$f" >/dev/null; then
    missing_bundled="$missing_bundled $f"
  fi
done
if [ -z "$sqlite_users" ]; then
  warn "sqlite-linkage" "no crate depends on rusqlite"
elif [ -n "$missing_bundled" ]; then
  bad "sqlite-linkage" "rusqlite without bundled feature in:$missing_bundled (would link host libsqlite3 — unqualified)"
else
  ok "sqlite-linkage" "all rusqlite deps use features=[bundled]: $(echo $sqlite_users | tr '\n' ' ')"
fi

if [ -f Cargo.lock ] && command -v python3 >/dev/null; then
  python3 - <<'PY'
import re, glob, os
lock = open("Cargo.lock").read()
pkgs = re.findall(r'name = "([^"]+)"\nversion = "([^"]+)"', lock)
for want in ("libsqlite3-sys", "rusqlite"):
    vs = sorted({v for n, v in pkgs if n == want})
    if vs:
        print(f"LOCK {want} {','.join(vs)}")
    else:
        print(f"ABSENT {want}")
PY
fi
# Report the vendored amalgamation version actually compiled — only for the
# libsqlite3-sys versions resolved in Cargo.lock, never the host sqlite3 binary.
sqlite_ver=""
locked_sqlite_sys=$(python3 -c '
import re
lock = open("Cargo.lock").read()
print(" ".join(sorted({v for n, v in re.findall(r"name = \"([^\"]+)\"\nversion = \"([^\"]+)\"", lock) if n == "libsqlite3-sys"})))
' 2>/dev/null)
for dirv in $locked_sqlite_sys; do
  for hdr in "${CARGO_HOME:-$HOME/.cargo}"/registry/src/*/libsqlite3-sys-"$dirv"/sqlite3/sqlite3.h; do
    [ -f "$hdr" ] || continue
    hv=$(grep -m1 '#define SQLITE_VERSION ' "$hdr" | cut -d'"' -f2)
    sqlite_ver="$sqlite_ver libsqlite3-sys-$dirv→sqlite-$hv"
  done
done
if [ -n "$sqlite_ver" ]; then
  ok "sqlite-bundled-version" "$sqlite_ver (vendored amalgamation, not system sqlite)"
elif [ -n "$locked_sqlite_sys" ]; then
  warn "sqlite-bundled-version" "libsqlite3-sys $locked_sqlite_sys resolved but vendored sqlite3.h not unpacked in CARGO_HOME — run a build once; recorded evidence: docs/ops/SUPPLYCHAIN.md"
else
  warn "sqlite-bundled-version" "libsqlite3-sys absent from Cargo.lock"
fi

echo "---"
echo "supplychain-check: $PASS pass, $WARN warn, $FAIL fail"
[ "$FAIL" -eq 0 ]
