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

The repo holds two kinds of fixture. **Service fixtures** are deployable
controllable equivalents of a real dependency, consumed as a container image.
**Library fixtures** are dev-dependency crates of reusable test plumbing,
consumed as a `git` + `tag` Cargo dependency (not an image).

### Service fixtures (container images)

| Fixture | Image | Replaces | Docs | Changelog |
|---|---|---|---|---|
| `oidc-test-idp` | `ghcr.io/botresources/br-oidc-test-idp` | A real OIDC identity provider (Entra, Google, …) | [README](crates/oidc-test-idp/README.md) | [CHANGELOG](crates/oidc-test-idp/CHANGELOG.md) |

### Library fixtures (dev-dependency crates)

| Fixture | Consumed as | Gives you | Docs | Changelog |
|---|---|---|---|---|
| `br-test-harness` | `[dev-dependencies]` git + tag | Real PG + NATS/KV + spawned binary + Passport/GraphQL/WS/SSE clients (re-exports `oidc-test-idp` in-process) | [README](crates/br-test-harness/README.md) | [CHANGELOG](crates/br-test-harness/CHANGELOG.md) |
| `conformance-scope` | `[dev-dependencies]` git + tag | Black-box S1–S6 scope-declaration conformance battery; drive it against your own service binary via `start_with_binary` | [README](crates/conformance-scope/README.md) | [CHANGELOG](crates/conformance-scope/CHANGELOG.md) |

Planned: `identity-test` — a minimal, contract-conformant identity service
(Passport + claim declaration) so that platform services can be e2e-tested
against *an* identity without depending on any project's private one.

### CLI tools (release binaries)

| Tool | Consumed as | Gives you | Docs | Changelog |
|---|---|---|---|---|
| `conformance-scope-cli` (bin `conformance-scope`) | GitHub Release binary (`conformance-scope-cli-vX.Y.Z`) | Run the scope-declaration conformance battery against any service, in any language, with no Rust to write | [README](crates/conformance-scope-cli/README.md) | [CHANGELOG](crates/conformance-scope-cli/CHANGELOG.md) |

## Distribution

Every fixture is a workspace crate, versioned and tagged independently
(`<crate>-vX.Y.Z`). How it is *consumed* depends on its kind:

- **Service fixtures** ship as a **container image**: on merge to `main`, CD
  publishes `ghcr.io/botresources/br-<crate>:<version>` (multi-arch, amd64 +
  arm64) when that version's image does not exist yet.
- **Library fixtures** (`br-test-harness`, `conformance-scope`) are consumed as a
  `git` + `tag` **dev-dependency** — there is no image. The tag is the release.
- **CLI tools** (`conformance-scope-cli`) ship as **GitHub Release binaries**
  (static musl-linux + macOS) built on an explicit `conformance-scope-cli-vX.Y.Z`
  tag — no image; downloaded and run directly.

## Release process

1. In your PR, bump the affected crate's `Cargo.toml` version and add a
   matching `## [X.Y.Z] — YYYY-MM-DD` section to its `CHANGELOG.md`.
2. CI gates the PR (fmt, clippy, tests, deny, machete, semver-checks,
   changelog entry).
3. On merge to `main`: `release-tags` creates the `<crate>-vX.Y.Z` tag and
   GitHub Release. For a service fixture, `cd` then builds and publishes the
   missing image version; a library fixture's tag is itself the release.

`conformance-scope-cli` is the exception: it is **excluded from auto-tagging** and
released by an explicit human-pushed `conformance-scope-cli-vX.Y.Z` tag, which
triggers `release-cli` to build and upload its binaries — see its
[README](crates/conformance-scope-cli/README.md#releasing).

## Why

| Thing | Why it is the way it is |
|---|---|
| `[profile.dev.package.{rsa,num-bigint-dig}] opt-level = 3` in the root `Cargo.toml` | Pure-Rust RSA key generation is unbearably slow unoptimized; the OIDC test IdP's pre-generated key pool would stall every test suite otherwise, so the crypto stack stays optimized even in dev/test profiles. |

## Dev

```bash
cargo build  --workspace
cargo test   --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt    --all
```

MSRV: **1.88** (edition 2024). License: Apache-2.0.
