# br-e2e-harness

> [!IMPORTANT]
> **This repository is maintained for BotResources and its authorized clients.**
> It is published under Apache-2.0 and made available read-only for visibility
> and dependency consumption. The Apache-2.0 license governs your rights to
> use, modify, and fork the code; the rest of this notice describes our
> operational stance, not a legal restriction.
>
> **We do not accept external pull requests, issues, or support requests.**
> Issues and Discussions are disabled. PRs from accounts that are not on the
> internal contributor allowlist will be closed without review. The GitHub
> fork button is disabled — you may still fork under Apache-2.0, but we
> provide no support outside the BR commercial relationship.
>
> - Clients with a commercial relationship: contact your BR account manager.
> - Security reports: see [SECURITY.md](SECURITY.md) (private email channel).
> - This is not a community-supported project. No support is provided through
>   GitHub.

Test fixtures for end-to-end testing of services built on the
[BotResources](https://botresources.ai) platform.

> [!WARNING]
> **EVERYTHING IN THIS REPOSITORY IS A TEST FIXTURE.** These components
> deliberately do things no production system should ever do — such as signing
> any token on request. They implement **no authentication** on their admin
> surfaces, by design. **Never deploy any of them outside an isolated,
> throwaway test network** (a CI job, a `kind` cluster, a local Docker Compose
> stack).

## Why this repo exists

The BR platform doctrine is *real* end-to-end tests: real transport, real
services, no mocks inside the system under test, and no test backdoor in any
production binary. Test accommodations belong in the harness — you substitute
a *dependency* with a controllable equivalent; you never weaken production
code.

Some dependencies cannot be the real thing in a test stack (you do not run a
real corporate IdP in CI). This repo hosts the controllable equivalents,
shared by every repo that needs them.

## Catalog

| Fixture | Image | Replaces | Docs | Changelog |
|---|---|---|---|---|
| `oidc-test-idp` | `ghcr.io/botresources/br-oidc-test-idp` | A real OIDC identity provider (Entra, Google, …) | [README](crates/oidc-test-idp/README.md) | [CHANGELOG](crates/oidc-test-idp/CHANGELOG.md) |

Planned: `identity-test` — a minimal, contract-conformant identity service
(Passport + claim declaration) so that platform services can be e2e-tested
against *an* identity without depending on any project's private one.

## Distribution

Each fixture is a workspace crate, versioned and tagged independently
(`<crate>-vX.Y.Z`), and consumed as a **container image**: on merge to `main`,
CD publishes `ghcr.io/botresources/br-<crate>:<version>` (multi-arch,
amd64 + arm64) when that version's image does not exist yet.

## Release process

1. In your PR, bump the affected crate's `Cargo.toml` version and add a
   matching `## [X.Y.Z] — YYYY-MM-DD` section to its `CHANGELOG.md`.
2. CI gates the PR (fmt, clippy, tests, deny, machete, semver-checks,
   changelog entry).
3. On merge to `main`: `release-tags` creates the `<crate>-vX.Y.Z` tag and
   GitHub Release; `cd` builds and publishes the missing image version.

## Dev

```bash
cargo build  --workspace
cargo test   --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt    --all
```

MSRV: **1.88** (edition 2024). License: Apache-2.0.
