#!/usr/bin/env bash
# VOID environment doctor — verifies the toolchain pins from
# docs/dependencies/PINS.md and reports PASS/FAIL per check.
set -u
PASS=0; FAIL=0; WARN=0
chk() { # name, status(0/1/warn), detail
  case "$2" in
    0) printf "  PASS  %s %s\n" "$1" "$3"; PASS=$((PASS+1));;
    warn) printf "  WARN  %s %s\n" "$1" "$3"; WARN=$((WARN+1));;
    *) printf "  FAIL  %s %s\n" "$1" "$3"; FAIL=$((FAIL+1));;
  esac
}

echo "VOID doctor — $(uname -srm)"

# Rust
if command -v cargo >/dev/null; then
  chk "rust" 0 "$(rustc --version | awk '{print $2}')"
else chk "rust" 1 "not installed"; fi

# flatc pin — must be exactly 25.9.23
if command -v flatc >/dev/null; then
  V=$(flatc --version | awk '{print $3}')
  [ "$V" = "25.9.23" ] && chk "flatc" 0 "$V" || chk "flatc" 1 "want 25.9.23, got $V"
else chk "flatc" 1 "not installed (build tools/protocol-gen.sh dependency)"; fi

# node + pnpm
command -v node >/dev/null && chk "node" 0 "$(node --version)" || chk "node" 1 "missing"
command -v pnpm >/dev/null && chk "pnpm" 0 "$(pnpm --version)" || chk "pnpm" 1 "missing (corepack enable)"

# cmake/ninja for native builds
command -v cmake >/dev/null && chk "cmake" 0 "$(cmake --version | head -1 | awk '{print $3}')" || chk "cmake" 1 "missing"
command -v ninja >/dev/null && chk "ninja" 0 "$(ninja --version)" || chk "ninja" 1 "missing"

# C++ toolchain
command -v c++ >/dev/null && chk "cxx" 0 "$(c++ --version | head -1 | cut -c1-40)" || chk "cxx" 1 "missing"

# platform checks
case "$(uname -s)" in
  Darwin)
    chk "platform" 0 "macOS $(sw_vers -productVersion 2>/dev/null) — engine/audio qualification possible"
    ;;
  Linux)
    chk "platform" warn "Linux — engine/audio hardware qualification is blocked by policy (run macOS lane for T03/T13-T16)"
    ;;
esac

# engine worker binary (optional at this stage)
if [ -n "${VOID_ENGINE_BIN:-}" ] && [ -x "${VOID_ENGINE_BIN}" ]; then
  chk "engine" 0 "$VOID_ENGINE_BIN"
else
  chk "engine" warn "VOID_ENGINE_BIN unset — spawn_engine unavailable until native/void-engine lands"
fi

# webkit for the Tauri shell (Linux dev)
if [ "$(uname -s)" = "Linux" ]; then
  pkg-config --exists webkit2gtk-4.1 && chk "webkit2gtk" 0 "$(pkg-config --modversion webkit2gtk-4.1)" || chk "webkit2gtk" warn "webkit2gtk-4.1 dev package missing (tauri dev needs it)"
fi

echo "---"
echo "doctor: $PASS pass, $WARN warn, $FAIL fail"
[ $FAIL -eq 0 ]
