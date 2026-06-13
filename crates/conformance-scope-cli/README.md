# conformance-scope-cli

A **language-agnostic CLI** (`conformance-scope`) that drives the BotResources
**scope-declaration wire handshake** against any service — in any language,
stateless or stateful — with **no Rust to write**. It is a thin wrapper over the
[`conformance-scope`] battery: parse arguments, build the expected declaration
and acceptor behavior, call the lib runner, format the report, set the exit code.
All protocol logic lives in the lib; the binary reimplements none of it.

> ⚠️ **TEST TOOLING.** It plays the Identity side of the handshake on a test
> network. It is not a production component.

## Usage

The binary is self-documenting. For the authoritative, always-current surface,
run `--help` at any level:

```sh
conformance-scope --help           # subcommands + global flags
conformance-scope run --help       # targets, expected declaration, scenarios, output
conformance-scope manifest --help  # the multi-service manifest mode
```

Driving it from a script, CI, or an AI agent? Read `--help` first — it is the
source of truth for flags and defaults; this README can lag behind the binary.

## Install

### Prebuilt binary (recommended)

Each unified `v{version}` release publishes standalone binaries as GitHub
Release assets — no Docker, no Rust toolchain needed to run. Pick the asset for
your platform from the release page:

| Platform | Asset |
| --- | --- |
| Linux x86_64 | `conformance-scope-<version>-x86_64-unknown-linux-musl.tar.gz` |
| macOS Apple Silicon | `conformance-scope-<version>-aarch64-apple-darwin.tar.gz` |

The Linux build is a static musl binary (runs on any glibc/musl distro). Each
asset ships a matching `.sha256`:

```sh
version=0.3.0
target=x86_64-unknown-linux-musl
asset="conformance-scope-${version}-${target}.tar.gz"

curl -fLO "https://github.com/BotResources/br-e2e-harness/releases/download/v${version}/${asset}"
curl -fLO "https://github.com/BotResources/br-e2e-harness/releases/download/v${version}/${asset}.sha256"

sha256sum -c "${asset}.sha256"
tar -xzf "${asset}"
chmod +x conformance-scope
./conformance-scope --help
```

On macOS use `shasum -a 256 -c "${asset}.sha256"` to verify.

### From source

```sh
cargo build -p conformance-scope-cli --release
# binary at target/release/conformance-scope
```

### Quick examples

Attach to a live service's NATS and `/readyz`:

```sh
conformance-scope run --attach \
  --nats nats://localhost:4222 \
  --readyz http://localhost:8080/readyz \
  --service-key example-service \
  --scopes example:read \
  --platform-only true
```

Spawn a throwaway `nats-server` and drive a subject binary through the full
battery:

```sh
conformance-scope run --spawn ./my-subject \
  --service-key example-service \
  --scopes example:read
```

## Two modes

### Attach (default) — zero host dependencies

Connect to an already-running service's NATS and `/readyz`, play the acceptor,
capture the declare, validate it by deserializing into the real `br-core-scope`
types, assert the content matches what you expect, then drive (or withhold)
acceptance and confirm the readiness gating. It never spawns `nats-server` and
never builds anything.

```sh
conformance-scope run \
  --nats nats://127.0.0.1:4222 \
  --readyz http://127.0.0.1:8080/readyz \
  --service-key example-service \
  --scopes example:read,example:admin
```

The service owns its handshake JetStream stream; the CLI binds a consumer to the
**pre-existing** stream (default name from the wire contract; override with
`--stream`) and fails loud if it is absent. Attach runs `s1, s2` and the
`declaration-content` assertion by default — the lifecycle-controlling scenarios
(s3/s4/s6) cannot run against an already-booted service.

### Spawn — convenience, needs `nats-server`

Stand up a throwaway `nats-server`, launch a subject binary with the env
contract, and run the full `s1..s6` battery plus the content assertion. Requires
`nats-server` on `PATH`.

```sh
conformance-scope run \
  --spawn ./my-subject \
  --service-key example-service \
  --scopes example:read,example:admin
```

The subject binary is configured via environment variables: `SERVICE_KEY`,
`SCOPE_KEYS` (CSV), `PLATFORM_ONLY`, `SCOPE_DECLARATION_ENABLED`, plus the NATS /
HTTP / stream wiring the CLI sets per scenario.

## Expected declaration

The assertion is `--service-key` + `--scopes` (CSV) + `--platform-only`.
`platform_only` is per scope in the domain, so `--platform-only` accepts either a
single bool applied to every scope, or a per-scope `key=bool` CSV:

```sh
--platform-only false
--platform-only example:read=false,example:admin=true
```

A wrong `--scopes` makes the `declaration-content` check fail with an
expected-vs-observed diff and a non-zero exit:

```text
service: example-service
  [FAIL] declaration-content
      expected: service_key="example-service", scopes=[example:read]
      observed: service_key="example-service", scopes=[example:read,example:admin]
      scope set mismatch:
        expected: example:read(platform_only=false)
        observed: example:read(platform_only=false), example:admin(platform_only=false)

NON-CONFORMANT: 2 passed, 1 failed, 0 skipped
```

## Acceptor behavior

`--accept` (default) plays the acceptor as accept. `--reject [REASON]` plays it
as reject, exercising the rejection path; the optional reason code is one of
`scope_owned_by_another_service` (default), `duplicate_scope_in_declaration`,
`scope_prefix_mismatch`. In the full spawn battery the rejection scenario (s4)
always exercises a rejection regardless of the global flag — `--reject` only
customizes the reason it surfaces.

In **attach** mode the rejection scenario (s4) is not in the default set and
only exercises a rejection when you pass `--reject`. If you select it without
`--reject` (`--scenarios s4` in accept mode), s4 is reported `skipped` — with a
detail noting it requires `--reject` — never run, never failed. Run it with
`conformance-scope run --reject ... --scenarios s4 ...`, attaching while the
service is still awaiting acceptance so the declare is observable.

## Scenarios, timeout, output

- `--scenarios <CSV>`, e.g. `--scenarios s1,s2` or
  `--scenarios declaration-content`. Defaults: attach ⇒ `s1,s2` + content;
  spawn ⇒ `s1..s6` + content.
- `--timeout <DUR>` per step, e.g. `10s` (default), `500ms`.
- `--format <human|json|junit>` (default `human`), `--output <PATH>` to write to
  a file instead of stdout. Each check reports its id, expected, observed, and a
  wire/file excerpt on failure.

## Many services — manifest

`conformance-scope manifest <FILE>` runs a YAML manifest of services and
aggregates one report. Each service is either `attach` or `spawn`:

```yaml
services:
  - service_key: example-service
    scopes:
      - key: example:read
        platform_only: false
      - key: example:admin
        platform_only: true
    attach:
      nats: nats://127.0.0.1:4222
      readyz: http://127.0.0.1:8080/readyz
      # stream: IDENTITY   # optional, defaults to the wire-contract stream
  - service_key: other-service
    scopes:
      - key: other:read
    scenarios: s1,s2
    reject: scope_owned_by_another_service   # optional; mirrors `run --reject [REASON]`
    timeout: 30s                             # optional; per-service step timeout (default 10s)
    spawn:
      path: ./other-subject
```

Each service takes the same per-service options as `run` — `scenarios`,
`reject` (a reason code), `timeout` — and follows the same scenario/acceptor
semantics (e.g. in `attach`, selecting `s4` requires `reject`, else it is
`skipped`).

`--format` / `--output` apply to the aggregate report.

## Exit codes

- `0` — fully conformant (no failed checks).
- `1` — at least one check failed.
- `2` — a usage / connection / I/O error (bad arguments, NATS unreachable,
  missing stream, unwritable output).

## Releasing

The CLI binaries ride the **unified `v{version}` release**, alongside the rest of
the workspace — there is no separate CLI tag. To cut a release:

1. Bump the workspace `version` in the root `Cargo.toml`.
2. Add a `## [X.Y.Z] — YYYY-MM-DD` section to the root `CHANGELOG.md` (the
   release notes are extracted from this section).
3. Merge to `main`. `release-tags` creates the `v{version}` tag and Release on
   the merge; `release-cli` triggers on the same merge, builds the matrix, and
   uploads the binaries.

The `release-cli` workflow builds the matrix, packages each binary as
`conformance-scope-<version>-<target>.tar.gz` + `.sha256`, and uploads them to
the unified `v{version}` GitHub Release (notes from the root `CHANGELOG.md`). To
re-cut without a new merge, run it via `workflow_dispatch`.

### Adding a target

The build matrix lives in `.github/workflows/release-cli.yml` as rows of
`{ target, runner, builder }`. Adding a platform is one entry:

- Another musl/Linux arch — `builder: zigbuild` on `ubuntu-latest` (e.g.
  `aarch64-unknown-linux-musl`); it cross-compiles statically and is asserted
  free of a `PT_INTERP` segment.
- Another macOS arch — `builder: native` on a macOS runner (e.g.
  `x86_64-apple-darwin` on `macos-13`).

`zigbuild` rows get the static assertion; `native` rows do not. No other step
changes.

## License

Apache-2.0. MSRV **1.88** (edition 2024).

[`conformance-scope`]: ../conformance-scope/README.md
