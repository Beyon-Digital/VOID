# VOID — Source and verification register

## Repository

Observed `main` on **2026-10-08** at `4135db035a93b57b6b9c1f3814ff6189e4aae74a`, unchanged from the earlier static audit. Connected GitHub branch read: https://api.github.com/repos/Beyon-Digital/VOID/branches/main . Pinned commit: https://github.com/Beyon-Digital/VOID/commit/4135db035a93b57b6b9c1f3814ff6189e4aae74a . Earlier source evidence is preserved in `references/PARITY_SOURCE.json`; no new runtime or other-branch audit was performed.

## Supplied research artifacts

- `references/VOID_Logic_Pro_Parity_and_Build_Plan.html`: 116 feature groups, 22 named stock-tool outcome groups, source/evidence register, original proposed phases. Its vendor baseline is historical research, not a fresh claim about Apple's current release.
- `references/VOID_Tauri_Rust_Architecture_Decision.html`: prior shell/native decision; state-ownership and isolation ambiguities are superseded by this handoff.
- `references/VOID_Integration_Stack_and_Infrastructure.html`: 41 library decisions, 20 infrastructure rows and 12 qualification-test families; proposal, not measured combined implementation.
- `references/VOID_Dependency_Catalog.json`: unchanged original catalog. `tracking/DEPENDENCIES.json` adds qualification fields; all revisions/approvals initially unresolved.

## Limited primary-source rechecks on 2026-10-08

| Source | Rechecked boundary |
|---|---|
| https://github.com/Tracktion/tracktion_engine | Engine supplies a musical data model and JUCE-module examples; Tracktion/JUCE licensing is separate. Select the supported exact pair and current required C++ standard at W01. |
| https://github.com/juce-framework/JUCE/blob/master/LICENSE.md | Exact selected JUCE terms must be reviewed; no blanket distribution clearance is inferred. |
| https://v2.tauri.app/develop/tests/webdriver/ | Desktop automation guidance must match the actual pinned Tauri/testing route; native plugin windows and audio devices need separate qualification. |
| https://v3.tauri.app/develop/calling-frontend/ | Generic events are not the low-latency/high-throughput audio transport; this page may describe a different major than the ultimately pinned application. Do not upgrade solely to follow a documentation URL. |
| https://www2.sqlite.org/wal.html | Current documentation describes the WAL-reset fix and WAL limitations. Use a supported patched build, inspect the linked runtime and avoid network/live-WAL sync. |

These checks confirm architectural constraints, not all prior maturity, licensing, model-quality or maintenance claims. The execution agent must refresh each activated dependency at its exact lock date. Prior snapshots retain their dates and original warnings. This task did not compile the combined stack or certify legal, security, audio, model or GPU behavior.

## New work in this packet

The task DAG, 100 scenarios, protocol/timebase/queue defaults, checkpoint publication protocol, schema examples and prompts are engineering specifications authored for implementation. Numeric limits are defaults/targets, not measurements or hardware requirements. Packet integrity/schema/link checks are separate from application validation.
