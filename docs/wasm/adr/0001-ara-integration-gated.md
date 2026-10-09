# ADR 0001 — ARA integration stays gated

- Status: **accepted (gated)** — 2026-10-09, Lane X / W28
- Feature: HOST-03 (ARA / Audio Random Access editor integration)
- Decision owner: W28 lane; integrator may revisit with evidence.

## Context

T98 asks for "advanced synthesis and SDK gates — every enabled
advanced instrument/save patch and ARA or plugin-export integration
against selected SDK and rights; blocked rights/features remain open."

ARA (Celemony) is an SDK-gated integration: using it requires the ARA
SDK distribution/development agreement. We do NOT hold that licence
in this repository or org today, and we cannot lawfully/effectively
verify an ARA integration without it.

## Decision

`GatedIntegration::Ara` ships **Gated** in the void-wasm gate ledger
(`gates.rs`). Requests return `IntegrationGated` — a typed refusal,
not a stub claiming support. The only path to `Qualified` is
`try_qualify` with non-empty `sdk` + `rights_ref` + `commit` evidence.

## Consequences

- No ARA-related claims in feature lists, UI, or docs-as-done.
- The gate record is where future qualification lands; when the org
  holds the SDK rights, `try_qualify` records the version + licence
  reference + the commit that enabled it.
- WASM modules are unaffected: ARA would ride the same restricted
  host ABI if ever qualified — no special-casing added now.
