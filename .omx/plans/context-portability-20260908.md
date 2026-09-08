# Context migration cleanup plan — 2026-09-08

1. Preserve all existing runtime, source-reference, experiment and intermediate files locally; delete nothing.
2. Keep .omx/plans as tracked historical plans; replace untracked scratch notepad with a short durable pointer after backing it up inside ignored state.
3. Add an authoritative docs/handoff entry, asset manifest and environment/bootstrap notes. Point AGENTS.md and README to it.
4. Exclude .omx state, source plates, reference comparisons, backups and experiments by allowlist. Do not republish third-party image files or scenes that pack them.
5. Through Blender MCP, inspect the last complete v08 artifact and package ONLY its generated exterior scene if dependencies are self-contained and no reference images/scripts/external libraries are included. Keep the source and active scene unchanged. Validate the packaged scene against v08 and record hashes.
6. Preserve a bounded set of v08 machine-readable reports and source link/hash metadata; mark historical evidence and unresolved v09 distinctly.
7. Run existing Python regression tests before/after edits, validate ignore rules, document links, snapshot checksums and a clean Git snapshot. Check Rust formatting/static/test status if practical without new downloads.
8. Review scoped staged changes and required Git identity before any commit. Do not sweep unrelated files or force-push. Confirm what actually reaches the remote; do not claim clone portability for unpushed work.
