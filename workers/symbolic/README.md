# void-symbolic-worker

Symbolic-completion argv worker (workers/PROTOCOL.md v1, W13).

Reads a `job/1.0.0` spec on stdin (`kind: "symbolic"`, parameters = scoped
region context), trains a seeded interval-Markov + Krumhansl key-estimate
model on the seed notes, and writes `proposals.json` — up to
`maxProposals` ranked continuations, each a list of note events plus a
model-derived score and a measured-facts rationale.

Deterministic: same spec + same `seed` ⇒ byte-identical `proposals.json`.
No network, no writes outside `--staging`, labels/lyrics are inert data.

Build: `cargo build --manifest-path workers/symbolic/Cargo.toml`
