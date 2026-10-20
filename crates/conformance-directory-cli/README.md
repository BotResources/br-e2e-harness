# conformance-directory-cli

A **language-agnostic CLI** (`conformance-directory`) that runs the BotResources
**identity Published Language** (directory KV projection) battery: the wire checks
W1..W6, the publisher checks P1..P2 and the consumer checks C1..C6, with **no Rust to
write**. It is a thin wrapper over the [`conformance-directory`] battery: parse
arguments, call `run_battery`, format the report, set the exit code. All protocol logic
stays in the battery crate.

> **TEST TOOLING.** The publisher and consumer checks stand up a throwaway
> `nats-server` and a throwaway Postgres database. It is not a production component.

**No spawn or attach mode.** The battery has no service binary to start. It deserializes
the Go anchor's frozen snapshot through the real `br-core-directory` types (W) and
replays it through the real `br-util-directory` publisher and consumer kit against real
infra (P, C). A service's CI runs it to prove the kit it pins honors the frozen wire.

## Usage

The binary is self-documenting; `--help` is the source of truth for flags and defaults:

```sh
conformance-directory --help
conformance-directory run --help
```

| Flag | Meaning |
| --- | --- |
| `--anchor-binary <PATH>` | Run this `identity-directory` anchor. Default: looked up on `PATH`, then the cache directory, then downloaded from the `conformance-directory/v{version}` release. |
| `--checks w1,p1,c3,...` | Default: `w1..w6, p1..p2, c1..c6`. They always run in that order. |
| `--subject-label <LABEL>` | Subject name in the report (default `identity-directory`). |
| `--format human\|json\|junit`, `--output <PATH>` | Report format, and a file instead of stdout. |

| Checks | Need |
| --- | --- |
| `w1..w6` | the anchor only |
| `p1..p2` | `nats-server` on `PATH`, `fabric-nats` beside the executable |
| `c1..c6` | the same, plus a Postgres whose admin role can create roles and databases, in `E2E_PG_ADMIN_URL` or `DATABASE_URL` |

**Exit codes:** `0` conformant, `1` non-conformant (a check failed or was skipped), `2`
the run could not complete (bad flag, unknown check, anchor missing or unable to run,
`fabric-nats` not beside the executable, `nats-server` not on `PATH`, no reachable
Postgres, report could not be written).

## Run in CI

Download the published release binary at a pinned version and run it. The archive holds
`conformance-directory` and `fabric-nats`; keep them in the same directory. The CLI
fetches the Go anchor itself from the same release (verified by its SHA-256) when it is
not on `PATH`. The runner needs `nats-server` and a Postgres.

```yaml
services:
  postgres:
    image: postgres:16
    env:
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: postgres
    ports: ["5432:5432"]

steps:
  - name: install nats-server
    run: |
      set -euo pipefail
      v=v2.14.1
      curl -fsSL "https://github.com/nats-io/nats-server/releases/download/${v}/nats-server-${v}-linux-amd64.tar.gz" -o nats.tar.gz
      tar -xzf nats.tar.gz
      sudo install "nats-server-${v}-linux-amd64/nats-server" /usr/local/bin/nats-server

  - name: download the directory conformance CLI
    run: |
      set -euo pipefail
      version=0.1.1
      asset="conformance-directory-${version}-x86_64-unknown-linux-musl.tar.gz"
      base="https://github.com/BotResources/br-e2e-harness/releases/download/conformance-directory/v${version}"
      curl -fsSLO "${base}/${asset}"
      curl -fsSLO "${base}/${asset}.sha256"
      sha256sum -c "${asset}.sha256"
      mkdir conformance-bin
      tar -xzf "${asset}" -C conformance-bin

  - name: directory conformance battery
    env:
      E2E_PG_ADMIN_URL: postgresql://postgres:postgres@localhost:5432/postgres
    run: ./conformance-bin/conformance-directory run --format junit --output directory-conformance.xml
```

A non-zero exit code fails the job. On macOS use `shasum -a 256 -c` and the
`conformance-directory-<version>-aarch64-apple-darwin.tar.gz` asset. A service that only
publishes (no consumer) can run `--checks w1,w2,w3,w4,w5,w6,p1,p2` and needs no Postgres.

## Install

Each `conformance-directory/v{version}` release carries:

| Platform | Asset |
| --- | --- |
| Linux x86_64 | `conformance-directory-<version>-x86_64-unknown-linux-musl.tar.gz` |
| macOS Apple Silicon | `conformance-directory-<version>-aarch64-apple-darwin.tar.gz` |

The Linux build is static musl. Each asset ships a matching `.sha256`. From source:
`cargo build -p conformance-directory-cli --release` and
`cargo build -p br-test-harness --bin fabric-nats --release` (binaries at
`target/release/conformance-directory` and `target/release/fabric-nats`).

## License

Apache-2.0. MSRV **1.88** (edition 2024).

[`conformance-directory`]: ../conformance-directory/README.md
