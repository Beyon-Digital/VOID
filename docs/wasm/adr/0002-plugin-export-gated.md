# ADR 0002 — Native plugin export stays gated

- Status: **accepted (gated)** — 2026-10-09, Lane X / W28
- Feature: HOST-04 (export a patch/module as a native plugin — VST3 /
  CLAP / AU)

## Context

Exporting a module as a native plugin means emitting **arbitrary
signed binaries** built against third-party SDKs (Steinberg VST3,
CLAP, Apple AU). Each target has its own SDK distribution terms, and
producing signed binaries requires a signer policy and credentials.
Neither the SDK rights review nor the signer infrastructure exists on
this lane today — shipping a stub or a "supported" flag would be the
fake-pass anti-pattern this work package forbids.

## Decision

`GatedIntegration::PluginExport` ships **Gated** in the gate ledger.
Any request → `IntegrationGated` until `try_qualify` records real
evidence (per-format SDK id+version, rights/licence reference,
enabling commit).

## Consequences

- `tests/wasm/t98` proves the gate is closed AND that the open path
  is evidence-only (`gates_open_only_with_evidence`).
- When unblocked, export generates packages; the generated artifact
  is a NEW native artifact (outside the wasm declarative spec) — that
  boundary must be designed against the signer policy at that time,
  not assumed now.
