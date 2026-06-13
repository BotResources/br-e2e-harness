# Changelog

All notable changes to `conformance-scope-cli` are documented here. Format
follows [Keep a Changelog](https://keepachangelog.com/); versions follow semver.

## [0.1.0] — 2026-06-13

Requires `conformance-scope >= 0.2.0` (the battery + runner this binary wraps).

### Added

- Initial release: the `conformance-scope` CLI, a language-agnostic wrapper over
  the `conformance-scope` S1–S6 battery. All protocol logic lives in the lib;
  the binary parses arguments, builds the run config, calls the lib runner,
  formats the report, and sets the exit code.
- `conformance-scope run` — drive a single service in **attach** mode (default,
  zero host deps: `--nats` + `--readyz` against a live service, `--stream` for
  the pre-existing handshake stream) or **spawn** mode (`--spawn <PATH>`, needs
  `nats-server` on PATH). Expected declaration via `--service-key`, `--scopes`
  (CSV), `--platform-only` (a single bool or a per-scope `key=bool` CSV).
  Acceptor behavior via `--accept` (default) / `--reject [REASON]`. Scenario
  selection `--scenarios`, `--timeout`, `--format`, `--output`.
- `conformance-scope manifest <FILE>` — run a YAML manifest of many services
  (each `attach` or `spawn`) and aggregate one report.
- Report formats: `human`, `json`, `junit` — each check shows its id, expected,
  observed, and a wire/file excerpt on failure; a wrong-scopes failure reads as a
  clear expected-vs-observed diff.
- Exit codes: `0` fully conformant, `1` at least one check failed, `2` a
  usage / connection / I/O error.
