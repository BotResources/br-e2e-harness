# Changelog

All notable changes to `conformance-nats-fabric-cli` are documented here. This crate
is not a registered library: it is versioned with `conformance-nats-fabric` (its
version always equals that crate's), and its release binaries ride the git tag
`conformance-nats-fabric/v{version}`. Format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow semver.
Release headings are plain `## X.Y.Z` - the release pipeline greps that exact
form to decide whether a version ships.

## Unreleased

## 0.1.1

### Added

- First release of the CLI, with `conformance-nats-fabric` 0.1.1 under the tag `conformance-nats-fabric/v0.1.1`: the static binaries `conformance-nats-fabric-0.1.1-<target>.tar.gz` are attached to that release. `conformance-nats-fabric run` runs the battery in spawn mode (throwaway `nats-server`) or, with `--nats URL`, in attach mode; `conformance-nats-fabric list` prints the check ids. The report is `human`, `json` or `junit`; the exit code is 0 (conformant), 1 (a check failed), 2 (usage error, unknown check, repeated check, empty selection, missing anchor binary, no server). The credentials of the `--nats` URL appear in neither the report nor stderr: a panic hook prints the location of a panic and never its message, and the URL is cut at its last `@`. `--timeout` rejects zero and overflowing values with exit code 2. A killed run (SIGINT, SIGTERM, SIGKILL) is not cleaned in attach mode; a timed-out check is.
- The report and the `list` text are written through a helper that returns the error: a stdout that cannot be written (closed pipe, full disk) exits 2 with one `error: writing to stdout: ...` line, never a panic. The panic hook is installed only around the run and restored after it; a panic that escapes the run leaves the hook in place and unwinds normally (exit 101, no double panic, no abort), and the `[[bin]]` sets `doc = false` so its docs do not collide with the library's.
- Stderr follows the same rule: the final `error:` line and the panic hook message are written with an ignored result, so a closed stderr keeps exit code 2 (it was a panic, exit 101) and the hook cannot abort the process.
- It depends on `conformance-nats-fabric` through a workspace path dependency, not a tag pin: the CLI is released from the same commit as the library it drives, and no consumer depends on the CLI as a crate.
