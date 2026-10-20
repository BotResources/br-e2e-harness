# conformance-nats-fabric-cli

A **language-agnostic CLI** (`conformance-nats-fabric`) for the BotResources **NATS Fabric** conformance battery: the v1 subject grammar and the `PUBLISHED_LANGUAGE` KV, checked against a real `nats-server` and an independent Go anchor. It is a thin wrapper over [`conformance-nats-fabric`]: parse arguments, call the battery runner, format the report, set the exit code. All protocol logic lives in the battery; the binary reimplements none of it.

> **TEST TOOLING.** It starts throwaway `nats-server` processes or writes run-namespaced keys on the server you attach to. It is not a production component.

The battery tests the `br-util-nats-fabric` linked into this binary against NATS. It boots no service, so there is no service binary to point it at.

## Usage

`--help` is the source of truth for flags and defaults:

```sh
conformance-nats-fabric --help
conformance-nats-fabric run --help
conformance-nats-fabric list
```

- `run` (spawn mode): throwaway `nats-server` processes, every check. Needs `nats-server` on `PATH`.
- `run --nats nats://host:4222` (attach mode): an existing server whose `INTEGRATION_CMD`, `INTEGRATION_EVT` and `PUBLISHED_LANGUAGE` already exist. Nothing is provisioned; what the run wrote is removed, also when a check times out; the two spawn-only checks are not run by default, so a default attach verdict covers 10 of the 12 checks and prints no `skipped`; they are reported `skipped` only when selected with `--checks`. A killed run (SIGINT, SIGTERM, SIGKILL) is not cleaned: run-namespaced keys or the widened durable can remain on the server.
- `--checks CSV`, `--timeout 60s` (per check, above zero), `--format human|json|junit`, `--output PATH`.
- Exit code: `0` conformant, `1` a check failed, `2` usage error, unknown check, repeated check, empty selection, missing anchor binary, unreachable server or a stdout that cannot be written (closed pipe, full disk); a stderr that cannot be written never changes the code. `--timeout 0s` or a value too large is a usage error. The credentials of `--nats` appear in neither the report nor stderr, panics included.

The Go anchor (`nats-fabric`) is found on `PATH`, then in the cache directory, then downloaded and verified from the `conformance-nats-fabric/v{version}` release.

## Run in CI

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

Add `--nats "$NATS_URL"` to check the server your pipeline provisioned instead of throwaway ones; attach mode needs no local `nats-server`, so the install step can go. On macOS use `shasum -a 256 -c` and the asset `aarch64-apple-darwin`.

## Releasing

The CLI is not a library of its own: its version always equals that of `conformance-nats-fabric`, and its binaries (`conformance-nats-fabric-<version>-<target>.tar.gz` + `.sha256`) are attached to the `conformance-nats-fabric/v{version}` release by the `release-cli-nats-fabric` workflow.

## License

Apache-2.0. MSRV **1.88** (edition 2024).

[`conformance-nats-fabric`]: ../conformance-nats-fabric/README.md
