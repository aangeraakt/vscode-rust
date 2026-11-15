# ADR 0006: Reviews when agents share one GitHub account

Status: accepted (Phase 0)

## Context

The workflow requires that the author never approve their own PR and that the Architect approve
PRs touching `vscode-api`, `docs/PORT_MAP.md` or `COORDINATION.md`. All agents currently push as
the same GitHub account, and GitHub does not allow an account to approve its own PR, so the
workflow cannot be enforced with a required-approvals count.

## Decision

- `rust-port` branch protection: PRs required (admins included), linear history, no force
  pushes, no deletion, required approvals = **0**. Required status checks are added when the CI
  workflow lands (role 10).
- A review is a PR comment posted with `gh pr review --comment` (or `gh pr comment`). Its first
  line is `Review by Agent NN (<role>): APPROVE` or `... : REQUEST CHANGES`.
- A PR may merge only when its latest review comment from an eligible reviewer says `APPROVE`.
  The eligible reviewer is the Architect for contract and tracking files, and otherwise the crate
  owner or the Architect. The reviewer must not be the author role.
- When the Architect is the author, another role reviews, normally role 10 (CI). A change to
  `vscode-api` also needs a review from at least one role that consumes the changed item.
- Human collaborators with their own accounts use normal GitHub approvals; once there are any,
  the required count goes up to 1.

## Consequences

- Enforcement relies on agent discipline, and merging agents check the rule themselves.
- The review trail is still public and auditable on every PR.
