# scope-service — G3 conformance subject

A minimal **Go** service that performs the BotResources **scope-declaration wire
handshake** on boot, exactly as a real non-identity scope-owning service does.

It is the **test SUBJECT** for conformance group **G3**: a separate Rust runner
drives it as a **black box** — it sets env vars, brings up a JetStream-enabled
NATS plus a fake acceptor, and asserts on the subject's NATS traffic and its
`/readyz` / `/livez` HTTP endpoints.

> This is a **test fixture**. It implements no authentication on its HTTP
> surface and is meant only for an isolated, throwaway test network.

The on-wire contract this subject implements is frozen in
[`../../docs/conformance/scope-wire-v1.md`](../../docs/conformance/scope-wire-v1.md).
That document is authoritative; this README only describes how to run the binary.

## What it does

On boot, when enabled:

1. Builds a `DeclareServiceScopes` from env, wraps it in an `IntegrationCommand`
   with a fresh `correlation_id` (UUID), and publishes it (JetStream) to
   `integration.cmd.identity.service_scope.declare.v1` (captured by the fixed
   `INTEGRATION_CMD` stream).
2. Awaits a confirmation on
   `integration.evt.identity.service_scope.{accepted,rejected}.v1` via an
   **ephemeral** pull consumer on the fixed `INTEGRATION_EVT` stream
   (`DeliverPolicy=New`, `AckPolicy=None`, filtered to the two event subjects),
   matching on `metadata.correlation_id == ours`.
3. **Re-publishes the same `correlation_id`** every `WAIT_TIMEOUT` until a
   confirmation for it arrives.
4. `/readyz` is **503** until an `accepted` arrives (then **200**); on a
   `rejected` it stays **503** and stops re-publishing. `/livez` is always
   **200**.

Disabled mode (`SCOPE_DECLARATION_ENABLED=false`): publishes nothing, `/readyz`
is **200** immediately.

**The subject does NOT create the JetStream streams** (the platform never
auto-provisions). The fixed `INTEGRATION_CMD` and `INTEGRATION_EVT` streams must
already exist: `INTEGRATION_CMD` must capture the declare subject
(`integration.cmd.>`) and `INTEGRATION_EVT` both event subjects
(`integration.evt.>`). If either is missing, the awaiter fails and `/readyz`
stays 503.

## Configuration (env only)

| Variable | Required | Default | Meaning |
|---|---|---|---|
| `NATS_URL` | no | `nats://127.0.0.1:4222` | JetStream-enabled NATS URL |
| `HTTP_ADDR` | no | `:8080` | bind addr for `/readyz` + `/livez` |
| `SERVICE_KEY` | **yes** | — | the declaring service key (`manifest.key`), e.g. `notifier` |
| `SCOPE_KEYS` | no | _(empty)_ | comma-separated `service:capability` keys; empty ⇒ empty scope set |
| `LABEL_KEY` | no | _(empty)_ | `label_key` applied to the manifest and every scope |
| `DESCRIPTION_KEY` | no | _(empty)_ | `description_key` applied to the manifest and every scope |
| `PLATFORM_ONLY` | no | `false` | `platform_only` applied to every scope |
| `WAIT_TIMEOUT` | no | `10s` | re-publish interval (Go duration, e.g. `1s`, `500ms`) |
| `SCOPE_DECLARATION_ENABLED` | no | `true` | `false` ⇒ disabled mode (no publish, ready immediately) |

`LABEL_KEY` / `DESCRIPTION_KEY` / `PLATFORM_ONLY` are applied uniformly to all
scopes — the subject's purpose is to exercise the **wire envelope**, not to vary
per-scope metadata. To drive per-scope variation the runner can run multiple
instances.

## Build & run

```sh
# build
go build -o scope-service .          # or: make build

# run against a local JetStream NATS with pre-created INTEGRATION_CMD / INTEGRATION_EVT streams
NATS_URL=nats://127.0.0.1:4222 \
HTTP_ADDR=127.0.0.1:8080 \
SERVICE_KEY=notifier \
SCOPE_KEYS=notifier:read,notifier:admin \
LABEL_KEY=label.notifier \
DESCRIPTION_KEY=desc.notifier \
WAIT_TIMEOUT=1s \
./scope-service
```

```sh
make test       # go vet + go test (incl. the golden-shape test)
```

## Local smoke (with `nats-server` + `nats` CLI)

If `nats-server` and the `nats` CLI are on PATH, you can reproduce the smoke
test the build was verified with:

```sh
# 1. start NATS with JetStream
nats-server -js -sd /tmp/scope-js -p 4242 &

# 2. create the fixed streams (subject never provisions them)
nats -s nats://127.0.0.1:4242 stream add INTEGRATION_CMD \
  --subjects "integration.cmd.>" --storage file --replicas 1 --retention limits \
  --discard old --max-msgs=-1 --max-bytes=-1 --max-age=0 --max-msg-size=-1 \
  --dupe-window=2m --no-allow-rollup --deny-delete --deny-purge \
  --max-msgs-per-subject=-1 --max-consumers=-1
nats -s nats://127.0.0.1:4242 stream add INTEGRATION_EVT \
  --subjects "integration.evt.>" --storage file --replicas 1 --retention limits \
  --discard old --max-msgs=-1 --max-bytes=-1 --max-age=0 --max-msg-size=-1 \
  --dupe-window=2m --no-allow-rollup --deny-delete --deny-purge \
  --max-msgs-per-subject=-1 --max-consumers=-1

# 3. run the subject (no acceptor)
NATS_URL=nats://127.0.0.1:4242 HTTP_ADDR=127.0.0.1:8090 \
SERVICE_KEY=notifier SCOPE_KEYS=notifier:read,notifier:admin \
LABEL_KEY=label.notifier DESCRIPTION_KEY=desc.notifier WAIT_TIMEOUT=1s \
./scope-service &

# 4. /readyz is 503 (no acceptor); /livez is 200
curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:8090/readyz   # 503
curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:8090/livez    # 200

# 5. read back the published declare command and check the subject + shape
nats -s nats://127.0.0.1:4242 stream get INTEGRATION_CMD \
  --last-for=integration.cmd.identity.service_scope.declare.v1

# 6. accept it: publish an event echoing the command's correlation_id
#    (grab CID from step 5's metadata.correlation_id), then /readyz → 200
nats -s nats://127.0.0.1:4242 pub integration.evt.identity.service_scope.accepted.v1 \
  '{"event_id":"00000000-0000-7000-8000-0000000000aa","event_type":"service_scope.accepted","version":1,"occurred_at":"2026-01-01T00:00:00Z","metadata":{"actor_id":"00000000-0000-0000-0000-0000000000ab","correlation_id":"<CID>"},"payload":{"service":"notifier"}}'
curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:8090/readyz   # 200
```

The full S1–S6 e2e (declare-on-boot / readiness-gating / timeout-then-accept /
reject / duplicate-confirmation / disabled) is the Rust conformance runner's job;
it brings its own fake acceptor against the frozen wire spec.

## Why table

| Thing | Why it is the way it is |
|---|---|
| Subject does not create the stream | Mirrors the platform's fail-loud, never-auto-provision doctrine; the awaiter does `get`/bind, not create. The runner/harness owns stream setup. |
| `declaringActorID` is a v5 UUID under a fixed namespace | Byte-reproduces `br-util-scope-declaration::declaring_actor`; verified equal to the Rust output in `wire_test.go`. The acceptor never validates it, but the command must look real. |
| `causation_id` omitted (not null) | The real `EventMetadata` skips it when `None`; a boot-time self-initiated declare has no causing message. |
| Confirmation outcome decided by NATS subject, not body | Matches the declarer's `resolve_match`: accepted-subject ⇒ ready, rejected-subject ⇒ decode reason. Body is only read on the rejected path. |
| Uniform `LABEL_KEY`/`DESCRIPTION_KEY`/`PLATFORM_ONLY` across scopes | The subject tests the envelope, not per-scope metadata; keeps the env contract flat for the black-box runner. |
