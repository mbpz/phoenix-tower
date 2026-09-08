# Project working agreements

## Mandatory Git identity

User directive recorded on 2026-09-07: **Only use `ttbb <apples398@163.com>` for commits in this repository.**

- Both the author and committer must have name `ttbb` and email `apples398@163.com`.
- Keep repository-local `user.name=ttbb` and `user.email=apples398@163.com`; never fall back to a different global identity.
- Before every commit, amend, rebase, or history rewrite, check `git var GIT_AUTHOR_IDENT` and `git var GIT_COMMITTER_IDENT`. Environment variables and explicit author flags can override repository config; correct any mismatch before proceeding.
- After committing, verify both identities with `git show -s --format='%an <%ae> | %cn <%ce>' HEAD`.
- Do not use any other identity for new or rewritten commits here.
- Identity correction does not authorize force-pushing shared history or changing identity in unrelated repositories.

## Cross-machine handoff

- At the beginning of project work, read [docs/handoff/README.md](docs/handoff/README.md).
  It is the durable entry point for current status, verified artifacts, limitations and next steps.
- `.omx/plans/*.md` are historical plans, not live execution instructions. Recheck current
  Git/files/tool state before treating a recorded blocker or completion claim as current.
- Do not restore `.omx/state` as a live session on another machine. Do not commit logs,
  private reference plates, image comparisons, credentials or unreviewed `.blend` files.
- The reviewed scene-only snapshot under `docs/handoff/yellow-crane/` is an explicit
  exception for portable generated whitebox geometry, not permission to publish reference imagery.
- When handing off a new milestone, update the handoff document and selected artifact
  hashes together. Keep estimated geometry distinct from accepted game assets.
