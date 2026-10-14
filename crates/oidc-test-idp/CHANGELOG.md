# Changelog

All notable changes to `oidc-test-idp` are documented here. The git tag
`oidc-test-idp/v{version}` releases the crate; this crate is versioned
separately from the other crates of the repository. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow semver.
Release headings are plain `## X.Y.Z` — the release pipeline greps that exact
form to decide whether a version ships.

## Unreleased

## 0.7.0

### Changed

- First registry release of `oidc-test-idp` as its own library, tagged `oidc-test-idp/v0.7.0`. The library is imported at 0.7.0: the code is the one shipped by the workspace release `v1.2.1`; what changes is the versioning: the single workspace version and the single `vX.Y.Z` tag are gone, and this crate now carries its own version line.
- Container image: `ghcr.io/botresources/br-oidc-test-idp:0.7.0`, published from the `oidc-test-idp/v0.7.0` tag to the existing package. The earlier tags of the package (`0.1.0` to `0.6.0`, `1.0.0` to `1.2.1`) are images of earlier releases of the harness line and are untouched. Moving from `:1.2.1`, change the tag: the number goes down because the library line is `0.7`.
- Dependency pin: `{ git = "https://github.com/BotResources/br-e2e-harness", package = "oidc-test-idp", tag = "oidc-test-idp/v0.7.0", version = "0.7.0" }`.
