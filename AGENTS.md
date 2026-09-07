# Project working agreements

## Mandatory Git identity

User directive recorded on 2026-09-07: **Only use `ttbb <apples398@163.com>` for commits in this repository.**

- Both the author and committer must have name `ttbb` and email `apples398@163.com`.
- Keep repository-local `user.name=ttbb` and `user.email=apples398@163.com`; never fall back to a different global identity.
- Before every commit, amend, rebase, or history rewrite, check `git var GIT_AUTHOR_IDENT` and `git var GIT_COMMITTER_IDENT`. Environment variables and explicit author flags can override repository config; correct any mismatch before proceeding.
- After committing, verify both identities with `git show -s --format='%an <%ae> | %cn <%ce>' HEAD`.
- Do not use any other identity for new or rewritten commits here.
- Identity correction does not authorize force-pushing shared history or changing identity in unrelated repositories.
