# Content rights model (W19)

Two artifacts keep stock content honest:

- `content/inventory.json` — authored **stock descriptors**. Every entry
  carries an id, the STOCK-nn family row it serves (FEATURE_MAP.md's
  "Named stock-tool coverage" table), a musical-outcome label, a
  `source` (`original` | `licensed` | `commissioned`) and rights fields.
  These are inventory truth, not implementation claims — `status` is
  `needs_license`, `needs_dsp`, `descriptor_only` or `ready`, and only
  listening + numerical evidence moves an entry to `ready`.
- `docs/content-rights/ledger.json` — the **license ledger**. Every
  non-original entry's `rights.agreement_ref` points at a ledger row
  recording licensor, grant scope and status (`required` until the
  agreement is executed and its sha256 is recorded). `original` entries
  must use `license_id: "VOID-ORIG"` and carry no `agreement_ref`.

Validation: `crates/void-content` (`inventory.rs`) parses and validates
both files — `tests/content` runs the check in CI so a missing ledger
row, revoked licence or malformed entry fails the suite. The ledger
schema is described in `license-ledger.schema.json`.

Rules:

- Never copy competitor assets, binaries or preset signatures. Entries
  describe *outcome coverage* (`coverage_of` names what the original
  comparison inventory listed — coverage, not cloning).
- `ready` requires T72 evidence (listening + numerical fixtures) per the
  W19 "done only when" clause; until then entries stay honestly marked.
- User-captured content (Auto Sampler, imports) is `distribution:
  user_only` — VOID never redistributes it.
