# Changelog

All notable changes to `conformance-scope-cli` are documented here. This crate
is not a registered library: it is versioned with `conformance-scope` (its
version always equals that crate's), and its release binaries ride the git tag
`conformance-scope/v{version}`. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow semver.
Release headings are plain `## X.Y.Z` — the release pipeline greps that exact
form to decide whether a version ships.

## Unreleased

### Changed

- The manifest YAML is read with `serde-saphyr` (pure Rust) instead of `serde_yaml_ng`, which pulled in the unmaintained `unsafe-libyaml` (RUSTSEC-2024-0449). The manifest format is unchanged.

