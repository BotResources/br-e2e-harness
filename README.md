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

Every crate is its own library: it carries its own version, its own
`CHANGELOG.md`, and releases under its own git tag `{crate}/v{version}`. A crate
at `0.0.0` has not shipped on its own line yet; until it does, it is still
consumed at the legacy workspace tag `v1.2.1` (`tag = "v1.2.1"`).

The repo holds two kinds of fixture. **Service fixtures** are deployable
controllable equivalents of a real dependency, consumed as a container image.
**Library fixtures** are dev-dependency crates of reusable test plumbing,
consumed as a `git` + `tag` Cargo dependency (not an image).

### Service fixtures (container images)

| Fixture | Version | Image | Replaces | Docs | Changelog |
|---|---|---|---|---|---|
| `oidc-test-idp` | 0.1.0 | `ghcr.io/botresources/br-oidc-test-idp:0.1.0` | A real OIDC identity provider (Entra, Google, ...) | [README](crates/oidc-test-idp/README.md) | [CHANGELOG](crates/oidc-test-idp/CHANGELOG.md) |

`oidc-test-idp` is also a library (`br-test-harness` re-exports it in-process).
Install pin:

```toml
oidc-test-idp = { git = "https://github.com/BotResources/br-e2e-harness", package = "oidc-test-idp", tag = "oidc-test-idp/v0.1.0", version = "0.1.0" }
```

### Library fixtures (dev-dependency crates)

| Fixture | Version | Gives you | Docs | Changelog |
|---|---|---|---|---|
| `br-test-harness` | 0.0.0 | Real PG + NATS/KV + spawned binary + Passport/GraphQL/WS/SSE clients (re-exports `oidc-test-idp` in-process) | [README](crates/br-test-harness/README.md) | [CHANGELOG](crates/br-test-harness/CHANGELOG.md) |
| `conformance-scope` | 0.0.0 | Black-box S1-S6 scope-declaration conformance battery; drive it against your own service binary via `start_with_binary` | [README](crates/conformance-scope/README.md) | [CHANGELOG](crates/conformance-scope/CHANGELOG.md) |
| `conformance-identity` | 0.0.0 | Black-box G2 battery: the scope-*acceptance* side, driving an Identity registry against the frozen Go declaring subject | [README](crates/conformance-identity/README.md) | [CHANGELOG](crates/conformance-identity/CHANGELOG.md) |
| `conformance-passport` | 0.0.0 | Black-box G1 battery: sealed bearer -> `Passport` on `GET /internal/passport`, seeded from the Go anchor's committed wire vectors | [README](crates/conformance-passport/README.md) | [CHANGELOG](crates/conformance-passport/CHANGELOG.md) |
| `conformance-directory` | 0.0.0 | Identity published-language batteries: **Px** publisher, **Cx** consumer (real NATS + Postgres), plus the offline wire-deser gate on `br-core-directory` | [README](crates/conformance-directory/README.md) | [CHANGELOG](crates/conformance-directory/CHANGELOG.md) |
| `conformance-nats-fabric` | 0.0.0 | Black-box battery for the v1 integration subject grammar and the `PUBLISHED_LANGUAGE` KV, anchored against an independent Go renderer | [README](crates/conformance-nats-fabric/README.md) | [CHANGELOG](crates/conformance-nats-fabric/CHANGELOG.md) |

Planned: `identity-test` - a minimal, contract-conformant identity service
(Passport + claim declaration) so that platform services can be e2e-tested
against *an* identity without depending on any project's private one.

### CLI tools (release binaries)

| Tool | Version | Consumed as | Gives you | Docs | Changelog |
|---|---|---|---|---|---|
| `conformance-scope-cli` (bin `conformance-scope`) | 0.0.0, always `conformance-scope`'s | GitHub Release binary (asset of the release `conformance-scope/v{version}`) | Run the scope-declaration conformance battery against any service, in any language, with no Rust to write | [README](crates/conformance-scope-cli/README.md) | [CHANGELOG](crates/conformance-scope-cli/CHANGELOG.md) |

`conformance-scope-cli` is not a library of its own: it is versioned with
`conformance-scope`, and its binaries ride that crate's release.

## Distribution

Seven crates are registered libraries, named like the crate: `oidc-test-idp`,
`br-test-harness`, `conformance-directory`, `conformance-identity`,
`conformance-nats-fabric`, `conformance-passport`, `conformance-scope`. Each
releases under its own tag `{crate}/v{version}`. How a fixture is *consumed*
depends on its kind:

- **Library fixtures** are consumed as a `git` + `tag` **dev-dependency** pinned
  per crate - `{ git = "https://github.com/BotResources/br-e2e-harness", package = "<crate>", tag = "<crate>/v{version}", version = "{version}" }`.
  The `version` beside the tag is required (`cargo-deny` rejects a tag-only git
  dependency as a wildcard). The tag is the release.
- **Service fixtures** ship as a **container image** built from the crate's tag:
  `ghcr.io/botresources/br-<crate>:<version>` (multi-arch, amd64 + arm64), the
  version being the one in the `oidc-test-idp/v{version}` tag.
- **CLI tools** (`conformance-scope-cli`) ship as **GitHub Release binaries**
  (static musl-linux + macOS) attached to the `conformance-scope/v{version}`
  release, the version being the one in that tag.

A crate that names a sibling's types in its public API pins that sibling by the
same git url and the same tag string, and the workspace root maps the url to the
local paths with `[patch]` once such a pin exists; two different tags give two copies of the type.
Crates that depend on a sibling therefore move together, and a crate releases
only after the siblings it pins have released.

`v1.2.1` is the legacy workspace tag. A crate at `0.0.0` is still consumed at it.

## Release process

1. In your PR, bump `version` in the crate's own `Cargo.toml`, add a plain
   `## X.Y.Z` section to that crate's `CHANGELOG.md` (no brackets: the release
   pipeline greps that exact heading), and move the crate's install pin in this
   README to the new tag. `conformance-scope-cli` moves with `conformance-scope`.
2. CI gates the PR: fmt, clippy, tests, deny, machete, semver-checks (the
   `oidc-test-idp` and `br-test-harness` libraries; the conformance batteries are
   executable specs and are exempt), changelog entry, README pins, sibling pins,
   and the registry gate (the version must be sealed and not yet implemented).
3. On merge to `main`, `release-tags` detects each tracked crate whose version
   has no `{crate}/v{version}` tag, applies the release-line law, tags, creates
   the GitHub release and records the API listing and dependencies. It then
   starts `cd` (the `oidc-test-idp` image) or `release-cli` (the
   `conformance-scope` binaries) with the new tag.

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
