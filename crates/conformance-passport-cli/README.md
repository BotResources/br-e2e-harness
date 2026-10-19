# conformance-passport-cli

A **language-agnostic CLI** (`conformance-passport`) that runs the BotResources
**bearer/PAT to Passport** battery (G1, `GET /internal/passport`) against a service
binary in any language, with **no Rust to write**. It is a thin wrapper over the
[`conformance-passport`] battery: parse arguments, call `run_spawn`, format the
report, set the exit code. All protocol logic stays in the battery crate.

> **TEST TOOLING.** It stands up a throwaway `nats-server`, seeds committed sealed
> wire vectors into its `PUBLISHED_LANGUAGE` bucket and destroys that bucket (P8). It is
> not a production component, and it never touches a live service.

## Usage

The binary is self-documenting; `--help` is the source of truth for flags and defaults:

```sh
conformance-passport --help
conformance-passport run --help
```

| Flag | Meaning |
| --- | --- |
| `--spawn <PATH>` | Spawn this service binary (a path, or a bare name on `PATH`) as the subject. |
| `--anchor` | Drive the Go reference subject `identity-passport` instead (looked up on `PATH`, then the cache directory, then downloaded from the `conformance-passport/v{version}` release). Exactly one of `--spawn` / `--anchor`. |
| `--scenarios p1,p2,...` | Default: `p1..p10`. `p8` is destructive and always runs last. `g4` is an offline lib test and is not selectable. |
| `--timeout 10s` | How long to wait for the subject's `/readyz`. |
| `--subject-label <LABEL>` | Subject name in the report (default: the binary's file name). |
| `--format human\|json\|junit`, `--output <PATH>` | Report format, and a file instead of stdout. |

**Exit codes:** `0` conformant, `1` non-conformant (a check failed or was skipped), `2`
the run could not complete (bad flag, unknown scenario, binary not found or not
executable, anchor unavailable, `nats-server` not on `PATH`, subject never ready).

**Spawn only, no attach mode.** The battery owns the broker: it provisions the
`PUBLISHED_LANGUAGE` bucket, seeds the vectors, and P8 deletes the bucket under the
subject. That cannot run against a service attached to a live NATS.

## The service under test

The service binary must honor this env contract (set by the CLI on the child process):

| Var | Meaning |
| --- | --- |
| `NATS_URL` | the throwaway broker URL (bucket `PUBLISHED_LANGUAGE` already provisioned) |
| `PORT` | the loopback port to bind (`http://127.0.0.1:$PORT`) |
| `BEARER_SEAL_KEY` | base64-std of the 32-byte key the vectors are sealed under |

It exposes `GET /internal/passport`, `/readyz` and `/livez`; it only ever serves, it
never seals. The contract and the P1..P10 table are in the [`conformance-passport`]
README.

## Run in CI

Download the published release binary at a pinned version, build the service, and run
the battery against it. The CLI boots the service binary itself; the runner needs
`nats-server` and nothing else (no Go, no Rust to run the battery).

```yaml
- name: install nats-server
  run: |
    set -euo pipefail
    v=v2.14.1
    curl -fsSL "https://github.com/nats-io/nats-server/releases/download/${v}/nats-server-${v}-linux-amd64.tar.gz" -o nats.tar.gz
    tar -xzf nats.tar.gz
    sudo install "nats-server-${v}-linux-amd64/nats-server" /usr/local/bin/nats-server

- name: download the passport conformance CLI
  run: |
    set -euo pipefail
    version=0.1.1
    asset="conformance-passport-${version}-x86_64-unknown-linux-musl.tar.gz"
    base="https://github.com/BotResources/br-e2e-harness/releases/download/conformance-passport/v${version}"
    curl -fsSLO "${base}/${asset}"
    curl -fsSLO "${base}/${asset}.sha256"
    sha256sum -c "${asset}.sha256"
    tar -xzf "${asset}"

- name: build the service
  run: cargo build --release --locked

- name: passport conformance battery
  run: ./conformance-passport run --spawn target/release/my-service --format junit --output passport-conformance.xml
```

A non-zero exit code fails the job. On macOS use `shasum -a 256 -c` and the
`conformance-passport-<version>-aarch64-apple-darwin.tar.gz` asset.

## Install

Each `conformance-passport/v{version}` release carries:

| Platform | Asset |
| --- | --- |
| Linux x86_64 | `conformance-passport-<version>-x86_64-unknown-linux-musl.tar.gz` |
| macOS Apple Silicon | `conformance-passport-<version>-aarch64-apple-darwin.tar.gz` |

The Linux build is static musl. Each asset ships a matching `.sha256`. From source:
`cargo build -p conformance-passport-cli --release` (binary at
`target/release/conformance-passport`).

## License

Apache-2.0. MSRV **1.88** (edition 2024).

[`conformance-passport`]: ../conformance-passport/README.md
