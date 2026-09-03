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
| `oidc-test-idp` | `ghcr.io/botresources/br-oidc-test-idp` | A real OIDC identity provider (Entra, Google, …) | [README](crates/oidc-test-idp/README.md) | [CHANGELOG](CHANGELOG.md) |

### Library fixtures (dev-dependency crates)

| Fixture | Consumed as | Gives you | Docs | Changelog |
|---|---|---|---|---|
| `br-test-harness` | `[dev-dependencies]` git + tag | Real PG + NATS/KV + spawned binary + Passport/GraphQL/WS/SSE clients (re-exports `oidc-test-idp` in-process) | [README](crates/br-test-harness/README.md) | [CHANGELOG](CHANGELOG.md) |
| `conformance-scope` | `[dev-dependencies]` git + tag | Black-box S1–S6 scope-declaration conformance battery; drive it against your own service binary via `start_with_binary` | [README](crates/conformance-scope/README.md) | [CHANGELOG](CHANGELOG.md) |
| `conformance-identity` | `[dev-dependencies]` git + tag | Black-box G2 battery: the scope-*acceptance* side, driving an Identity registry against the frozen Go declaring subject | [README](crates/conformance-identity/README.md) | [CHANGELOG](CHANGELOG.md) |
| `conformance-passport` | `[dev-dependencies]` git + tag | Black-box G1 battery: sealed bearer → `Passport` on `GET /internal/passport`, seeded from the Go anchor's committed wire vectors | [README](crates/conformance-passport/README.md) | [CHANGELOG](CHANGELOG.md) |
| `conformance-directory` | `[dev-dependencies]` git + tag | Identity published-language batteries: **Px** publisher, **Cx** consumer (real NATS + Postgres), plus the offline wire-deser gate on `br-core-directory` | [README](crates/conformance-directory/README.md) | [CHANGELOG](CHANGELOG.md) |
| `conformance-nats-fabric` | `[dev-dependencies]` git + tag | Black-box battery for the v1 integration subject grammar and the `PUBLISHED_LANGUAGE` KV, anchored against an independent Go renderer | [README](crates/conformance-nats-fabric/README.md) | [CHANGELOG](CHANGELOG.md) |

Planned: `identity-test` — a minimal, contract-conformant identity service
(Passport + claim declaration) so that platform services can be e2e-tested
against *an* identity without depending on any project's private one.

### CLI tools (release binaries)

| Tool | Consumed as | Gives you | Docs | Changelog |
|---|---|---|---|---|
| `conformance-scope-cli` (bin `conformance-scope`) | GitHub Release binary (asset of release `v{version}`) | Run the scope-declaration conformance battery against any service, in any language, with no Rust to write | [README](crates/conformance-scope-cli/README.md) | [CHANGELOG](CHANGELOG.md) |

## Distribution

Every fixture is a workspace crate sharing **one workspace version**; the whole
repository releases as a single git tag `v{version}`. How a fixture is *consumed*
depends on its kind:

- **Service fixtures** ship as a **container image**: on merge to `main`, CD
  publishes `ghcr.io/botresources/br-<crate>:<version>` (multi-arch, amd64 +
  arm64) when that version's image does not exist yet.
- **Library fixtures** (`br-test-harness`, `conformance-scope`) are consumed as a
  `git` + `tag` **dev-dependency** pinned to `v{version}` — there is no image.
  The tag is the release.
- **CLI tools** (`conformance-scope-cli`) ship as **GitHub Release binaries**
  (static musl-linux + macOS) uploaded to the unified `v{version}` release — no
  image; downloaded and run directly.

## Release process

1. In your PR, bump the workspace `version` in the root `Cargo.toml`, add a
   matching `## X.Y.Z - YYYY-MM-DD` section to the root `CHANGELOG.md` — plain
   heading, ASCII hyphen, no brackets: `check-changelog.sh` and `release-tags`
   both accept `## {version}` followed by a space or the end of the line — and
   move every README pin of this repo to
   `tag = "v{version}"`.
2. CI gates the PR (fmt, clippy, tests, deny, machete, semver-checks, changelog
   entry, README pins).
3. On merge to `main`, three idempotent, order-independent workflows ship the
   one version: `release-tags` creates the single `v{version}` tag and GitHub
   Release (notes from the root `CHANGELOG.md`); `cd` builds and publishes any
   missing service-fixture image; `release-cli` builds the
   `conformance-scope-cli` binaries (static musl-linux + macOS) and uploads them
   to the `v{version}` release. A library fixture's tag is itself the release.

All three trigger on `push: main` and self-gate on whether their artifact for
`v{version}` already exists, because a tag pushed with the default `GITHUB_TOKEN`
does not trigger a second workflow — so the CLI binaries cannot be driven off the
tag event and ride the merge event instead.

## Why

| Thing | Why it is the way it is |
|---|---|
| `[profile.dev.package.{rsa,num-bigint-dig}] opt-level = 3` in the root `Cargo.toml` | Pure-Rust RSA key generation is unbearably slow unoptimized; the OIDC test IdP's pre-generated key pool would stall every test suite otherwise, so the crypto stack stays optimized even in dev/test profiles. |
| A cross-service wire is frozen by a **Go anchor**, and the Rust side deserializes through the **real lib types** | Freezing the wire in Rust freezes it against the very code that could drift with it. The anchor is an independent implementation, so a renamed field or a changed serde in `br-core-*` fails against it. Where the wire is a *credential* the runner cannot re-derive (the sealed bearer), the anchor emits **committed vectors** checked byte-for-byte by its own Go test, so no consumer needs `go` on `PATH` to run the battery. |
| Raw `async_nats` lives **here** and nowhere else | Service code reaches NATS only through `br-util-nats-fabric`. But a test fixture must do what the fabric refuses — provision an adversarial durable, purge a stream, publish malformed bytes, narrow a stream to inject a delivery outage — so the raw client is confined to `br-test-harness`, and a suite that needs one of those asks the harness instead of opening its own connection. |

## Dev

```bash
cargo build  --workspace
cargo test   --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt    --all
```

MSRV: **1.88** (edition 2024). License: Apache-2.0.
