# VOID — Agent handoff packet

**Start:** attach this complete ZIP to your implementation agent and paste `KICKOFF.md`. The agent should copy the packet into `docs/void-handoff/` in `Beyon-Digital/VOID` and start W00. No earlier chat is needed.

`VOID_IMPLEMENTATION_HANDOFF.html` is the self-contained readable initial-handoff snapshot. After implementation begins, current status lives in the JSON ledgers and regenerated Markdown views. `HANDOFF.md` and `CONTRACTS.md` hold normative instructions; `tracking/*.json` are canonical execution ledgers. `WORK_PACKAGES.md`, `TEST_MATRIX.md` and `FEATURE_MAP.md` are generated views.

| File / directory | Purpose |
|---|---|
| `KICKOFF.md` | Main implementation assignment. Start here; do not send every phase prompt at once. |
| `HANDOFF.md` | Accepted direction, boundaries, repo migration, scope, sequencing and delivery rules. |
| `CONTRACTS.md` | Native messages, state ownership, timebase, revisions, recovery, AI, plugins and exports. |
| `WORK_PACKAGES.md` | 30 dependency-ordered work packages and concrete done conditions. |
| `TEST_MATRIX.md` | 100 implementation acceptance scenarios; all initially not run. |
| `FEATURE_MAP.md` | All 116 source requirements and 22 stock outcome groups mapped to tasks. |
| `tracking/` | Task, test, feature, dependency, infrastructure, platform and baseline JSON. |
| `schemas/` + `examples/` | Four initial semantic schemas and illustrative validation inputs—not actual implemented APIs. |
| `prompts/` | Master, six phase-specific assignments, and verify/resume prompt. |
| `references/` | Original plans and source registers preserved offline. New handoff wins on architectural conflicts. |
| `PROGRESS.md` | Agent-maintained execution/resume checkpoint. |
| `SOURCES.md` | Evidence provenance and limits of current verification. |
| `tools/validate_packet.py` | Standard-library integrity/traceability validator. It does not test the application. |
| `tools/render_views.py` | Regenerate work/test/feature Markdown views from canonical JSON. |
| `MANIFEST.sha256` | Delivery hashes. Changes during implementation intentionally invalidate original delivery hashes. |

## Commands for the packet itself

From the packet root: `python3 tools/validate_packet.py`; regenerate views using `python3 tools/render_views.py`. Validate the four JSON schemas/examples additionally with a Draft 2020-12 validator. Application commands described in the handoff must be created by the implementation agent; they are not claimed to exist in the repository today.

## Scope guard

The full roadmap remains in scope. First prove native playback/plugin/save/reopen/render, then complete an offline song. Do not mistake F1 for full Logic parity. No licences are purchased, repository licence changed, cloud resources provisioned or releases published by this packet.
