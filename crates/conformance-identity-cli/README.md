# conformance-identity-cli

A **language-agnostic CLI** (`conformance-identity`) that runs the BotResources
**scope-acceptance** battery (G2, A1..A7) against an Identity acceptor binary or a
running acceptor in any language, with **no Rust to write**. It is a thin wrapper over
the [`conformance-identity`] battery: parse arguments, call `run_spawn` or `run_attach`,
format the report, set the exit code. All protocol logic stays in the battery crate.

> **TEST TOOLING.** It publishes `DeclareServiceScopes` commands and judges the
> `ServiceScopes` confirmations. Spawn mode stands up a throwaway `nats-server`; attach mode
> publishes to the broker you point it at. It is not a production component.

## Usage

The binary is self-documenting; `--help` is the source of truth for flags and defaults:

```sh
conformance-identity --help
conformance-identity run --help
```

| Flag | Meaning |
| --- | --- |
| `--spawn <PATH>` | Spawn this acceptor binary (a path, or a bare name on `PATH`) against a throwaway `nats-server`. |
| `--anchor` | Spawn the Go reference acceptor `identity-acceptor` instead (looked up on `PATH`, then the cache directory, then downloaded from the `conformance-identity/v{version}` release). |
| `--attach --nats <URL> --readyz <URL>` | Drive an already-running acceptor. The deployment owns the `INTEGRATION_CMD` and `INTEGRATION_EVT` streams; the CLI creates none. |
| `--scenarios a1,a2,...` | Default: `a1..a7`. |
| `--timeout 10s` | Wait for the acceptor's `/readyz` and for each confirmation. |
| `--subject-label <LABEL>` | Acceptor name in the report. |
| `--format human\|json\|junit`, `--output <PATH>` | Report format, and a file instead of stdout. |

Exactly one of `--spawn`, `--anchor`, `--attach` is required.

**Exit codes:** `0` conformant, `1` non-conformant (a check failed or was skipped), `2`
the run could not complete (bad flag, unknown scenario, binary not found or not
executable, anchor unavailable, `fabric-nats` not beside the executable, `nats-server`
not on `PATH`, acceptor never ready, streams absent in attach mode, report could not be
written).

## The acceptor under test

In spawn mode the CLI sets this env on the acceptor process:

| Var | Meaning |
| --- | --- |
| `NATS_URL` | the throwaway broker URL (command durable provisioned) |
| `HTTP_ADDR` | the loopback address to bind (`127.0.0.1:<port>`) |
| `SCOPE_ACCEPTANCE_ENABLED` | `true` |

The acceptor exposes `GET /readyz` and `GET /livez`, consumes the `DeclareServiceScopes`
command and answers on the `ServiceScopes` event subjects. The A1..A7 table is in the
[`conformance-identity`] README.

## Run in CI

Download the published release binary at a pinned version, build the acceptor, and run
the battery against it. The archive holds `conformance-identity` and `fabric-nats`; keep
them in the same directory. The runner needs `nats-server` and nothing else (no Go, no
Rust to run the battery).

```yaml
- name: install nats-server
  run: |
    set -euo pipefail
    v=v2.14.1
    curl -fsSL "https://github.com/nats-io/nats-server/releases/download/${v}/nats-server-${v}-linux-amd64.tar.gz" -o nats.tar.gz
    tar -xzf nats.tar.gz
    sudo install "nats-server-${v}-linux-amd64/nats-server" /usr/local/bin/nats-server

- name: download the identity conformance CLI
  run: |
    set -euo pipefail
    version=0.1.1
    asset="conformance-identity-${version}-x86_64-unknown-linux-musl.tar.gz"
    base="https://github.com/BotResources/br-e2e-harness/releases/download/conformance-identity/v${version}"
    curl -fsSLO "${base}/${asset}"
    curl -fsSLO "${base}/${asset}.sha256"
    sha256sum -c "${asset}.sha256"
    mkdir conformance-bin
    tar -xzf "${asset}" -C conformance-bin

- name: build the acceptor
  run: cargo build --release --locked

- name: scope-acceptance conformance battery
  run: ./conformance-bin/conformance-identity run --spawn target/release/my-identity --format junit --output identity-conformance.xml
```

A non-zero exit code fails the job. On macOS use `shasum -a 256 -c` and the
`conformance-identity-<version>-aarch64-apple-darwin.tar.gz` asset. To judge a service
that is already running, use `run --attach --nats <URL> --readyz <URL>` instead of
`--spawn`.

## Install

Each `conformance-identity/v{version}` release carries:

| Platform | Asset |
| --- | --- |
| Linux x86_64 | `conformance-identity-<version>-x86_64-unknown-linux-musl.tar.gz` |
| macOS Apple Silicon | `conformance-identity-<version>-aarch64-apple-darwin.tar.gz` |

The Linux build is static musl. Each asset ships a matching `.sha256`. From source:
`cargo build -p conformance-identity-cli --release` and
`cargo build -p br-test-harness --bin fabric-nats --release` (binaries at
`target/release/conformance-identity` and `target/release/fabric-nats`).

## License

Apache-2.0. MSRV **1.88** (edition 2024).

[`conformance-identity`]: ../conformance-identity/README.md
