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

Each `conformance-scope/v{version}` release publishes standalone binaries as
GitHub Release assets — no Docker, no Rust toolchain needed to run. Pick the asset for
your platform from the release page:

| Platform | Asset |
| --- | --- |
| Linux x86_64 | `conformance-scope-<version>-x86_64-unknown-linux-musl.tar.gz` |
| macOS Apple Silicon | `conformance-scope-<version>-aarch64-apple-darwin.tar.gz` |

The Linux build is a static musl binary (runs on any glibc/musl distro). Each
asset ships a matching `.sha256`:

```sh
version=0.1.1
target=x86_64-unknown-linux-musl
asset="conformance-scope-${version}-${target}.tar.gz"

curl -fLO "https://github.com/BotResources/br-e2e-harness/releases/download/conformance-scope/v${version}/${asset}"
curl -fLO "https://github.com/BotResources/br-e2e-harness/releases/download/conformance-scope/v${version}/${asset}.sha256"

sha256sum -c "${asset}.sha256"
tar -xzf "${asset}"
chmod +x conformance-scope
./conformance-scope --help
```

On macOS use `shasum -a 256 -c "${asset}.sha256"` to verify.

### From source

```sh
cargo install --path crates/conformance-scope-cli --locked
```

This installs the `conformance-scope` binary into `~/.cargo/bin`.

### Quick examples

Attach to a live service's NATS and `/readyz`:

```sh
conformance-scope run --attach \
  --nats nats://localhost:4222 \
  --readyz http://localhost:8080/readyz \
  --service-key example_service \
  --scopes example_service:read \
  --platform-only true
```

Spawn a throwaway `nats-server` and drive a subject binary through the full
battery:

```sh
conformance-scope run --spawn ./my-subject \
  --service-key example_service \
  --scopes example_service:read
```

## Run in CI

A service runs the battery in its own CI, against its own booted binary, with
the released `conformance-scope` binary pinned to a version. Nothing is compiled
and no Rust toolchain is needed. The CI mode is **attach**: the service is the
one under test, and the CLI plays the Identity side of the handshake against the
service's NATS and `/readyz`.

Before the CLI runs, the job must hold: a `nats-server` with JetStream, the two
fixed streams `INTEGRATION_CMD` (`integration.cmd.>`) and `INTEGRATION_EVT`
(`integration.evt.>`) already created (attach mode never creates them; a missing
stream exits `2`), and the service booted on that NATS, awaiting acceptance of
its declaration. The streams must be fresh: a leftover declare from an earlier
run makes the battery read a dead correlation id, so the recipe starts
`nats-server` on a new store directory (`-sd "$(mktemp -d)"`) at every run.

```yaml
- name: scope-declaration conformance
  env:
    CONFORMANCE_SCOPE_VERSION: "0.1.1"
    NATS_SERVER_VERSION: "v2.14.1"
    NATSCLI_VERSION: "0.3.1"
  run: |
    set -euo pipefail
    tools="$RUNNER_TEMP/conformance-tools"
    mkdir -p "$tools"

    target=x86_64-unknown-linux-musl
    asset="conformance-scope-${CONFORMANCE_SCOPE_VERSION}-${target}.tar.gz"
    base="https://github.com/BotResources/br-e2e-harness/releases/download/conformance-scope/v${CONFORMANCE_SCOPE_VERSION}"
    (cd "$tools" && curl -fsSLO "${base}/${asset}" && curl -fsSLO "${base}/${asset}.sha256" && sha256sum -c "${asset}.sha256" && tar -xzf "${asset}")

    nats_server="nats-server-${NATS_SERVER_VERSION}-linux-amd64"
    curl -fsSL "https://github.com/nats-io/nats-server/releases/download/${NATS_SERVER_VERSION}/${nats_server}.tar.gz" | tar -xz -C "$tools"
    natscli="nats-${NATSCLI_VERSION}-linux-amd64"
    curl -fsSL -o "$tools/natscli.zip" "https://github.com/nats-io/natscli/releases/download/v${NATSCLI_VERSION}/${natscli}.zip"
    unzip -q -o "$tools/natscli.zip" -d "$tools"

    "$tools/$nats_server/nats-server" -js -sd "$(mktemp -d)" -a 127.0.0.1 -p 4222 -m 8222 &
    until curl -fs http://127.0.0.1:8222/healthz >/dev/null; do sleep 0.2; done
    nats="$tools/$natscli/nats --server nats://127.0.0.1:4222"
    $nats stream add INTEGRATION_CMD --subjects 'integration.cmd.>' --defaults
    $nats stream add INTEGRATION_EVT --subjects 'integration.evt.>' --defaults

    NATS_URL=nats://127.0.0.1:4222 HTTP_ADDR=127.0.0.1:8080 ./target/release/my-service &
    until curl -fs http://127.0.0.1:8080/livez >/dev/null; do sleep 0.2; done

    "$tools/conformance-scope" run \
      --nats nats://127.0.0.1:4222 \
      --readyz http://127.0.0.1:8080/readyz \
      --service-key my_service \
      --scopes my_service:read,my_service:admin \
      --platform-only false \
      --format junit --output conformance-scope.xml
```

The step fails the job on a non-zero exit code: `1` when the service is not
conformant (a failed or skipped check), `2` when the run itself
could not happen (NATS unreachable, a missing stream, a bad argument). The
service's own environment variables are the service's; the CLI needs only its
NATS URL and its `/readyz` URL. On macOS use the `aarch64-apple-darwin` asset and
`shasum -a 256 -c`. Bump `CONFORMANCE_SCOPE_VERSION` deliberately, as for any
pinned tool; the `.sha256` check ties the download to the published asset. Poll the
service's `/livez`, never its `/readyz`: `/readyz` stays `503` until the CLI accepts
the declaration, so waiting on it would deadlock.

`--spawn <service-binary>` is not the CI mode: it stands up its own throwaway
`nats-server` and needs the `fabric-nats` binary and the provisioning manifest of
the source tree beside the executable, which the released archive does not hold.

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
  --service-key example_service \
  --scopes example_service:read,example_service:admin
```

Both fixed-name handshake streams, `INTEGRATION_CMD` and `INTEGRATION_EVT`, must
already exist. Attach checks them before anything else, never creates them, and
exits `2` when either is missing. Attach runs `s1, s2` and the `declaration-content` assertion by default.
s3, s4 and s6 depend on how the service was booted, so they are opt-in through
`--scenarios` and hold only under their prerequisite:

| Scenario | Prerequisite in attach mode |
|---|---|
| s3 | the service is still unconfirmed and re-publishes its declare within `--timeout`: raise `--timeout` above the service's own wait timeout |
| s4 | `--reject`, and the service still awaiting acceptance |
| s6 | the service was booted with scope declaration disabled |

Without the prerequisite s3, s4 (with `--reject`) and s6 fail and their detail names it;
s4 without `--reject` is reported `skipped`. A scenario is never refused for that
reason: the CLI cannot know how the service was booted.

### Spawn — convenience, needs `nats-server` and `fabric-nats`

Stand up a throwaway `nats-server`, launch a subject binary with the env
contract, and run the full `s1..s6` battery plus the content assertion. Requires
`nats-server` on `PATH`, the `fabric-nats` binary (the `fabric-nats` bin of the
`br-test-harness` package, built outside the tests) in the directory of the
`conformance-scope` executable, and the manifest
`crates/conformance-scope/tests/fixtures/scope_declaration.toml` of the source tree
the executable was built from. The released archive holds only `conformance-scope`,
so a released binary cannot run `--spawn` unless `fabric-nats` is placed beside it
and the build tree is present; a missing prerequisite exits `2` with an error that
names it. Attach mode needs none of them.

```sh
conformance-scope run \
  --spawn ./my-subject \
  --service-key example_service \
  --scopes example_service:read,example_service:admin
```

The subject binary is configured via environment variables: `SERVICE_KEY`,
`SCOPE_KEYS` (CSV), `PLATFORM_ONLY`, `SCOPE_DECLARATION_ENABLED`, plus the NATS /
HTTP wiring the CLI sets per scenario.

## Expected declaration

The assertion is `--service-key` + `--scopes` (CSV) + `--platform-only`.
The order of the scopes in `--scopes` does not matter: the declaration is compared as a set.
`platform_only` is per scope in the domain, so `--platform-only` accepts either a
single bool applied to every scope, or a per-scope `key=bool` CSV:

```sh
--platform-only false
--platform-only example_service:read=false,example_service:admin=true
```

The assertion is checked before anything is contacted. A `--scopes` key that no
declaration can hold (not `<service>:<capability>` in the allowed charset, not owned by
`--service-key`, or listed twice), a `--platform-only` key that names no listed scope or
appears twice, and a malformed `--service-key` exit `2` naming the key; they are never
reported as a failed check of the service.

A wrong `--scopes` makes the `declaration-content` check fail with an
expected-vs-observed diff and a non-zero exit:

```text
service: example_service
  [FAIL] declaration-content
      expected: service_key="example_service", scopes=[example_service:read]
      observed: service_key="example_service", scopes=[example_service:read,example_service:admin]
      scope set mismatch:
        expected: example_service:read(platform_only=false)
        observed: example_service:admin(platform_only=false), example_service:read(platform_only=false)

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
- `--timeout <DUR>` per step, e.g. `10s` (default), `500ms`, `2m`. The battery refuses a value of zero or above 86400s (24 hours) before any I/O, and the CLI exits `2` naming the value; so does a number that does not fit a `u64` of the chosen unit.
- `--format <human|json|junit>` (default `human`), `--output <PATH>` to write to
  a file instead of stdout. Each check reports its id, expected, observed, and a
  wire/file excerpt on failure.

## Many services — manifest

`conformance-scope manifest <FILE>` runs a YAML manifest of services and
aggregates one report. Each service is either `attach` or `spawn`:

```yaml
services:
  - service_key: example_service
    scopes:
      - key: example_service:read
        platform_only: false
      - key: example_service:admin
        platform_only: true
    attach:
      nats: nats://127.0.0.1:4222
      readyz: http://127.0.0.1:8080/readyz
  - service_key: other_service
    scopes:
      - key: other_service:read
    scenarios: s1,s2
    reject: scope_owned_by_another_service
    timeout: 30s
    spawn:
      path: ./other-subject
```

`reject` and `timeout` are optional (`timeout` defaults to `10s`). Each service takes the
same per-service options as `run` — `scenarios`, `reject` (a reason code), `timeout` — and follows the same scenario/acceptor
semantics (e.g. in `attach`, selecting `s4` requires `reject`, else it is
`skipped`).

A key the manifest does not know (a typo such as `platform_onyl`) is refused with exit `2`
naming it; the manifest never ignores a field.

The whole manifest is validated before the first service runs: an empty `services` list,
two `attach` entries on one broker (`attach` assumes a fresh handshake stream, so a broker
serves one attach entry) and any service whose timeout, scope keys, scenarios, `reject`
code, `/readyz` URL or `spawn` path cannot be run exit `2` naming the service and the
value, with no service run and no report written.

`--format` / `--output` apply to the aggregate report.

## Exit codes

- `0` — conformant (at least one check ran, none failed, none skipped).
- `1` — not conformant (a check failed or was skipped).
- `2` — a usage / connection / I/O / prerequisite error (bad arguments, NATS
  unreachable, missing stream, unwritable output, a missing `nats-server`,
  `fabric-nats`, an unreadable manifest, a `--spawn` target that is not a file, a
  subject that exited at boot on each of five attempts, or `--spawn` combined with `--nats` or `--readyz`, which
  would be ignored: the spawn battery runs against its own throwaway broker).

## Releasing

The CLI is not a library of its own: its version always equals the version of
`conformance-scope`, and its binaries are attached to the `conformance-scope/v{version}`
release. To cut a release:

1. Bump `version` in both `crates/conformance-scope/Cargo.toml` and
   `crates/conformance-scope-cli/Cargo.toml` to the same value, and add a
   `## X.Y.Z` section to both `CHANGELOG.md` files.
2. Merge to `main`. `release-tags` tags `conformance-scope/v{version}`, creates
   the Release (notes from `crates/conformance-scope/CHANGELOG.md`) and starts
   `release-cli` with that tag.

The `release-cli` workflow reads the version from the tag, builds the matrix at
the tagged commit, packages each binary as
`conformance-scope-<version>-<target>.tar.gz` + `.sha256`, and attaches them to
that release, next to the four `scope-service-<os>-<arch>.tar.gz` assets (+ `.sha256`) of the
Go subject, which `conformance-scope` downloads when its tests find no `scope-service` on `PATH`.
To re-cut without a new merge, run it via `workflow_dispatch` with
the tag.

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
