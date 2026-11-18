# conformance-nats-fabric

A **black-box conformance runner** for the BotResources **NATS Fabric** — the v1
integration subject grammar (`integration.{cmd,evt}.{bc}.{aggregate}.{verb,fact}.vN`)
and the **published-language** KV (`PUBLISHED_LANGUAGE`). It drives the real
`br-util-nats-fabric` against a real `nats-server` (via `FabricTestNats`) and
anchors the wire against an **independent Go renderer** in
`conformance-subjects/nats-fabric`.

> ⚠️ **TEST FIXTURE ONLY.** Add it as a **dev-dependency**, never a runtime one.
> It runs a prebuilt Go binary and stands up a real `nats-server` on an
> isolated, throwaway test network. It never compiles anything.

## lib-as-oracle / Go-as-anchor

The Go anchor **freezes the wire independently of the lib**: it renders the v1
subject grammar and the published-user KV shape by hand, with **no dependency on
`br-rust-common`**. The Rust runner then re-derives the same wire **through the
real lib types** and asserts they agree:

- **Subjects** — the runner builds `CommandCoords` / `EventCoords` from the
  anchor's segments and renders them with the lib's `command_subject` /
  `event_subject`; the result must equal the anchor's rendered subject
  **byte-for-byte**. If the lib's renderer drifts, the comparison fails.
- **Published-language values** — the runner deserializes the anchor's frozen
  user JSON **through `br_core_directory::PublishedUser`**. Deserialization
  succeeding *is* the shape check; a drifted/retyped field fails the deser.

The anti-drift mechanism is the **Go anchor's independence**, never a mirrored
copy of the lib's view in the test. A `make guard` target fails the anchor's CI
step if the **dead `identity.cmd.` / `identity.evt.` grammar** ever appears in its
source.

## The anchor binary

The tests **assume the Go anchor is already built**. The binary is named
`nats-fabric` (the folder name under `conformance-subjects/`). It is found, in
order:

1. on `PATH` (absolute entries only; a relative entry such as `.` is never
   searched);
2. in the cache directory `$XDG_CACHE_HOME/br-e2e-harness/conformance-nats-fabric-<version>`,
   or `~/.cache/...` (`~/Library/Caches/...` on macOS) when `XDG_CACHE_HOME` is
   unset; a relative `XDG_CACHE_HOME` or `HOME` is ignored;
3. downloaded from the `conformance-nats-fabric/v<version>` GitHub release of
   `BotResources/br-e2e-harness`: the asset `nats-fabric-<os>-<arch>.tar.gz` and
   its sibling `.sha256`, for `linux` and `darwin` on `amd64` and `arm64`. The
   download fails closed on a SHA-256 mismatch, a redirect to a host other than
   `github.com`, `objects.githubusercontent.com` or
   `release-assets.githubusercontent.com`, a non-https URL, or a size over
   64 MiB; it honours the system proxy variables. `tar` is resolved on the
   absolute entries of `PATH`. The binary is extracted into the cache directory
   with the executable bit set.

A missing binary is `ConformanceError::AnchorBinaryUnavailable { binary, detail }`
(the CLI exits 2 with it), a typed error naming `nats-fabric` and what to do: put it on `PATH`, or build it
outside the tests with `make -C conformance-subjects/nats-fabric build`. The
directory of the binary leads the `PATH` of the spawned child, set on the child
command only; the crate never calls `std::env::set_var` and its own code writes
nothing to the system temporary directory (the throwaway `nats-server` store of
`br-test-harness` stays that crate's own).

## What it proves

### Integration messaging

- A durable **widened** to `integration.evt.>` (not the exact coordinate subject)
  is **narrowed back** to the exact coordinate filter when the lib create-or-binds
  it (`ensure_event_durable` returns `Ok`); reading the durable's effective filter
  back proves the widening was undone — a consumer cannot silently over-subscribe.
- A **missing fixed stream** fails loud on **both** lib paths — the
  `verify_*_durable` coverage probe *and* the `ensure_*_durable` durable bind —
  with `FabricError::Consume(NoStream)`, and the stream is still absent
  afterwards: the platform never auto-provisions it.
- The anchor's rendered subjects match the lib renderers **byte-for-byte**.
- The **dead `identity.*` grammar fails loud**: a publish on `identity.cmd.*`
  lands on no fixed stream (`PublishErrorKind::NoStream`) and no `INTEGRATION_*`
  stream captures it — no fallback, no silent drop. Both checks first require
  `INTEGRATION_CMD` and `INTEGRATION_EVT` to exist: on a server without them the
  checks panic instead of passing, and a broker that cannot answer is a panic,
  never "no message".

### Published language (KV)

- `retract` **orphan-deletes** the key.
- `reconcile` **converges** drift: adds the missing, repairs the changed,
  deletes the orphaned. Every value read back is compared **whole** (email, first
  name, last name and extensions) with the desired one.
- `bootstrap` is **parallel-safe** under a concurrently running `watch`: the check
  first proves the watch is live (it keeps putting a canary key until the watch
  projects it, since the watch delivers only new entries), then runs the
  bootstrap, which projects the published seed and retracts a sink orphan, and
  panics if the watch ended before the check stopped it.
- A live slash-keyed put is **delivered by `watch` within the deadline** —
  `prefix_watch_delivers_slash_keyed_directory_puts` asserts this on real infra,
  after the same liveness proof; a watch that ended is a panic, not a silent
  `false`.
- A malformed KV value **fails closed and names the offending key** in the
  `FabricError::Decode` error.

## Running

Add it as a dev-dependency. Its public signatures name types of three sibling
libraries (`br-test-harness`: `FabricTestNats`, `BareFabricNats`, `WidenedDurable`;
`br-util-nats-fabric`: `KvKey`, `KvPrefix`, `FabricError`, `PublishErrorKind`,
`ProjectionSink`; `br-core-directory`: `PublishedUser`), so pin each of them with
the same git url and the same tag string as this crate, or the build holds two
copies of the type and fails with a type mismatch:

```toml
[dev-dependencies]
conformance-nats-fabric = { git = "https://github.com/BotResources/br-e2e-harness", package = "conformance-nats-fabric", tag = "conformance-nats-fabric/v0.1.1", version = "0.1.1" }
br-test-harness = { git = "https://github.com/BotResources/br-e2e-harness", package = "br-test-harness", tag = "br-test-harness/v0.1.2", version = "0.1.2" }
br-util-nats-fabric = { git = "https://github.com/BotResources/br-rust-common", package = "br-util-nats-fabric", tag = "br-util-nats-fabric/v0.1.0", version = "0.1.0" }
br-core-directory = { git = "https://github.com/BotResources/br-rust-common", package = "br-core-directory", tag = "br-core-directory/v0.1.0", version = "0.1.0" }
```

The real-infra tests are `#[ignore]`-gated and require a `nats-server` on `PATH`
and, for the anchor tests, the `nats-fabric` binary (see above):

```sh
cargo test -p conformance-nats-fabric --all-targets -- --ignored --test-threads=1
```

The published-language tests in `tests/conformance.rs` **provision their KV
topology by spawning the `fabric-nats` CLI** (`fabric-nats provision --manifest
tests/fixtures/published_language.toml`) rather than calling
`FabricTestNats::with_published_language()` in-process — the suite is the CLI's
real-life testbed; the CLI must be built beside the test executable. The crate
carries **no `async-nats` dependency**: it observes and mutates the fabric
exclusively through the typed `FabricTestNats` surface (`fabric_owned`,
`pl_reader` / `pl_publisher` / `pl_put_raw`, `assert_missing_stream` /
`publish_dead_subject` / `raw_message_absent` / `fixed_streams_present`).

CI runs this battery in the `infra-e2e` job (which installs `nats-server` on
`PATH`, runs the anchor's source guard and Go tests, and puts the built
`nats-fabric` binary on `PATH` before the battery), so the real-infra gate is
enforced on every PR, not just documented.

The non-infra unit tests (byte-for-byte renderer agreement, filter shape, the
whole-value comparison, the binary lookup and the download policy) run by
default:

```sh
cargo test -p conformance-nats-fabric
```

## Runner and CLI

`run_spawn(&checks, timeout)` and `run_attach(&AttachTarget { nats_url }, &checks, timeout)` run the same 12 checks as `tests/conformance.rs` and return a `ConformanceReport` (`Pass` / `Fail` / `Skipped` per `CheckId`, expected, observed). A panicking or timed-out check is a `Fail` (the timeout is per check and must be above zero; setup and cleanup are bounded separately), never a crash of the caller. `CheckId::ALL` lists the checks, `parse_checks` reads their kebab-case codes.

This battery tests the `br-util-nats-fabric` linked into the caller against a real NATS server, and against the Go anchor. It boots no service, so there is no service binary to point it at.

- **Spawn** starts throwaway `nats-server` processes (needs `nats-server` on `PATH`) and runs every check; the `PUBLISHED_LANGUAGE` bucket is provisioned in-process.
- **Attach** runs against an existing server. It provisions nothing: a check fails loud when `INTEGRATION_CMD`, `INTEGRATION_EVT` or `PUBLISHED_LANGUAGE` is absent. It retracts every published-language key and deletes the durable it wrote, also when the check body outlives its timeout: the deadline bounds the body only, and the cleanup runs afterwards under its own 30 s bound. Nothing is cleaned when the process is killed (SIGINT, SIGTERM, SIGKILL), when the target stops answering during the cleanup, or when the setup itself never finishes; run-namespaced keys or the widened durable can then remain. The two checks that need a server without the fixed streams are not in `attach_default()`, so a default attach run covers 10 of the 12 checks; passed explicitly to `run_attach`, they are reported `Skipped`.
- A missing anchor binary stops the run with `ConformanceError::AnchorBinaryUnavailable` before any check runs.

The language-agnostic wrapper is the `conformance-nats-fabric` binary of the `conformance-nats-fabric-cli` crate (`conformance-nats-fabric --help` is the source of truth): `run` (spawn, or attach with `--nats URL`; `--checks`, `--timeout`, `--format human|json|junit`, `--output`) and `list`. Exit code 0 is conformant, 1 a failed check, 2 a usage error, an unknown check, a repeated check, an empty selection, a missing anchor binary, an unreachable server or a report that cannot be written (stdout or `--output` file).

### Run in CI

Download the release binary of a pinned version, verify it and run it. The binary finds the Go anchor on `PATH`, then in its cache, then downloads and verifies the anchor asset of its own release.

Spawn mode starts a throwaway `nats-server`, which a stock runner does not have: install a pinned one first (the version this repository's CI uses).

```yaml
- name: install nats-server
  env:
    NATS_SERVER_VERSION: v2.14.1
  run: |
    set -euo pipefail
    url="https://github.com/nats-io/nats-server/releases/download/${NATS_SERVER_VERSION}/nats-server-${NATS_SERVER_VERSION}-linux-amd64.tar.gz"
    curl -fsSL "$url" -o /tmp/nats-server.tar.gz
    tar -xzf /tmp/nats-server.tar.gz -C /tmp
    sudo install "/tmp/nats-server-${NATS_SERVER_VERSION}-linux-amd64/nats-server" /usr/local/bin/nats-server
    nats-server --version

- name: NATS Fabric conformance
  env:
    CONFORMANCE_NATS_FABRIC_VERSION: 0.1.1
  run: |
    set -euo pipefail
    version="${CONFORMANCE_NATS_FABRIC_VERSION}"
    target=x86_64-unknown-linux-musl
    asset="conformance-nats-fabric-${version}-${target}.tar.gz"
    base="https://github.com/BotResources/br-e2e-harness/releases/download/conformance-nats-fabric/v${version}"
    curl -fLO "${base}/${asset}"
    curl -fLO "${base}/${asset}.sha256"
    sha256sum -c "${asset}.sha256"
    tar -xzf "${asset}"
    ./conformance-nats-fabric run --format junit --output nats-fabric-conformance.xml
```

That is spawn mode. To check the NATS server your pipeline already provisioned (fixed streams and `PUBLISHED_LANGUAGE` bucket), add `--nats "$NATS_URL"` (attach mode needs no local `nats-server`, so the install step can go).

## Why table

| Thing | Why it is the way it is |
|---|---|
| Anchor renders subjects by hand, no `br-rust-common` dep | The Go side must freeze the wire **independently** of the lib; a shared dependency would let both drift together and blind the detector. |
| Runner re-derives through the lib types | The lib is the **oracle**: byte-for-byte subject equality and `PublishedUser` deser are the drift detectors. |
| `make guard` greps for `identity.cmd.`/`identity.evt.` | The dead pre-v1 grammar must never reappear in the live-wire anchor source; the guard needs the source tree, so it runs in the `infra-e2e` CI step that builds the anchor, in addition to the runtime fail-loud probe that proves the fabric rejects the dead subject. |
| The tests never compile | A test that compiles couples the result to a toolchain and writes build output outside the cache; the anchor is built once, outside the tests, and found on `PATH`, in the cache, or as a verified release asset. |
| The watch is proven live by a canary, not a sleep | `PublishedLanguageConsumer::watch` delivers only entries put after it subscribes; a sleep proves nothing, so the check re-puts a canary key until the watch projects it. |
| The fixed streams must exist before an absence check | `raw_message_absent` reads a missing stream as "no message" and a publish on a server without streams fails with `NoStream` for every subject; both would pass while proving nothing. |
| Real-infra tests `#[ignore]`-gated | Matches the harness convention: CI installs `nats-server` on `PATH` in the `infra-e2e` job and runs the battery with `--ignored`; the default `cargo test` stays infra-free. |
| Each PL run is key-namespaced by `FabricTestNats::key_prefix()` | The shared `PUBLISHED_LANGUAGE` bucket is reused across parallel runs; the run-id prefix keeps reconcile/orphan scopes from colliding. |
| Attach mode never provisions and cleans up after itself | The platform never auto-provisions a stream or a bucket; a conformance run against a real environment must not be the exception, nor leave run-namespaced keys or a widened durable behind. The timeout bounds the check body only, so a timed-out check is still cleaned; a killed process (any signal) is not. |
