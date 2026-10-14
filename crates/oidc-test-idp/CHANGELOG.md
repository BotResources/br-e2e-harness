# Changelog

All notable changes to `oidc-test-idp` are documented here. The git tag
`oidc-test-idp/v{version}` releases the crate; this crate is versioned
separately from the other crates of the repository. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow semver.
Release headings are plain `## X.Y.Z` — the release pipeline greps that exact
form to decide whether a version ships.

## Unreleased

## 0.1.0

### Changed

- First release of `oidc-test-idp` as its own library, tagged `oidc-test-idp/v0.1.0`. The code is the one shipped by the workspace release `v1.2.1`; what changes is the versioning: the single workspace version and the single `vX.Y.Z` tag are gone, and this crate now carries its own version line.
- Container image: `ghcr.io/botresources/br-oidc-test-idp:0.1.0`, published from the `oidc-test-idp/v0.1.0` tag. Moving from `:1.2.1`, change the tag: the number goes down because the line restarts at `0.1`.
- Dependency pin: `{ git = "https://github.com/BotResources/br-e2e-harness", package = "oidc-test-idp", tag = "oidc-test-idp/v0.1.0", version = "0.1.0" }`.
