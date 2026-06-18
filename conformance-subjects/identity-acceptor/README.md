# identity-acceptor — G2 conformance subject

A minimal **Go** service that plays the BotResources **Identity scope-acceptance**
side of the scope-declaration wire handshake, exactly as the real Identity registry
does: it consumes declare commands, decides each against an in-memory scope
registry, and replies with `accepted` or `rejected`.

It is the **test SUBJECT** for conformance group **G2** — the mirror of G3 with the
role inverted. A separate Rust runner ([`conformance-identity`]) drives it as a
**black box**: it brings up a JetStream-enabled NATS, plays the **declaring** side
(publishing real declare commands), and asserts the subject's replies against the
real `judge_declaration` oracle.

> This is a **test fixture**. It implements no authentication on its HTTP surface
> and is meant only for an isolated, throwaway test network.

The on-wire contract this subject implements is frozen in
[`../../docs/conformance/scope-wire-v1.md`](../../docs/conformance/scope-wire-v1.md).
That document is authoritative; this README only describes how to run the binary.

## What it does

On boot, when enabled:

1. Binds a **durable** pull consumer (`DeliverPolicy=All`, `AckPolicy=Explicit`,
   filtered to `integration.cmd.identity.service_scope.declare.v1`) on the fixed
   `INTEGRATION_CMD` stream, then sets `/readyz` **200**.
2. For each declare command, decodes it and **judges** it against an in-memory
   `scope_key → owning_service` registry that **persists across declarations**,
   applying the same rules and precedence as the real
   `judge_declaration` / `ScopeRegistry`:
   - manifest key validity, then each scope key's validity → `invalid_scope_key`;
   - scope prefix ownership (key prefixed by the declaring service) →
     `scope_prefix_mismatch`;
   - intra-declaration duplicate → `duplicate_scope_in_declaration`;
   - cross-service ownership of an already-registered key →
     `scope_owned_by_another_service`;
   - a re-declaration by the same owner is an **idempotent accept**.
3. On accept: registers all scopes and publishes
   `integration.evt.identity.service_scope.accepted.v1` (`{"service":"<key>"}`),
   **echoing the command's `correlation_id`**. On reject: registers nothing and
   publishes `integration.evt.identity.service_scope.rejected.v1`
   (`{"service":"<key>","reason":{…}}`), echoing the `correlation_id`.

Disabled mode (`SCOPE_ACCEPTANCE_ENABLED=false`): consumes nothing, `/readyz` is
**200** immediately. `/livez` is always **200**.

**The subject does NOT create the JetStream streams** (the platform never
auto-provisions). The fixed `INTEGRATION_CMD` and `INTEGRATION_EVT` streams must
already exist: `INTEGRATION_CMD` must capture the declare subject
(`integration.cmd.>`) and `INTEGRATION_EVT` both event subjects
(`integration.evt.>`). If `INTEGRATION_CMD` is missing, the consumer fails and
`/readyz` stays 503.

There is **no seeding via env** — the registry is black-box state. The runner seeds
an ownership context by driving a prior accepted declaration.

## Configuration (env only)

| Variable | Required | Default | Meaning |
|---|---|---|---|
| `NATS_URL` | no | `nats://127.0.0.1:4222` | JetStream-enabled NATS URL |
| `HTTP_ADDR` | no | `:8080` | bind addr for `/readyz` + `/livez` |
| `SCOPE_ACCEPTANCE_ENABLED` | no | `true` | `false` ⇒ disabled mode (consume nothing, ready immediately) |

## Build & run

```sh
# build
go build -o identity-acceptor .      # or: make build

# run against a local JetStream NATS with pre-created INTEGRATION_CMD / INTEGRATION_EVT streams
NATS_URL=nats://127.0.0.1:4222 \
HTTP_ADDR=127.0.0.1:8080 \
./identity-acceptor
```

```sh
make test       # go vet + go test (incl. the golden-shape + judge-precedence tests)
```

The full A1–A7 e2e (clean accept / cross-service claim / intra-declaration
duplicate / prefix mismatch / invalid key / idempotent re-declare / structurally
malformed key) is the Rust conformance runner's job; it brings its own declaring
side and the real `judge_declaration` oracle.

## Why table

| Thing | Why it is the way it is |
|---|---|
| Subject does not create the stream | Mirrors the platform's fail-loud, never-auto-provision doctrine; the consumer binds, it does not create. The runner/harness owns stream setup. |
| Judge precedence: validity → prefix → intra-duplicate → cross-owner | Byte-for-byte the order of the real `judge_declaration` (`command.validate()` runs first — manifest+scope key validity, then prefix, then duplicate — and only an accepted-validation reaches the registry's cross-owner check). A cross-service claim is therefore rejected as `scope_prefix_mismatch`, never reaching the cross-owner branch; that branch is kept for fidelity to `register_declaration` and is exercised only by a direct unit test. |
| In-memory registry persists across declarations | State is the point: ownership is established by an earlier accepted declaration, so the cross-owner and idempotent-re-declare cases need the prior state. |
| Confirmation echoes `correlation_id` and decides outcome by subject | Matches the declarer's matching rule: accepted-subject ⇒ ready, rejected-subject ⇒ decode reason; the reply must carry the command's `correlation_id`. |
| Reply metadata uses `actor_kind:"service"` | The acceptor acts as a service; the declarer matches on `correlation_id` only and tolerates any valid `actor_kind`. |
| `causation_id` omitted (not null) | The real `EventMetadata` skips it when `None`. |

[`conformance-identity`]: ../../crates/conformance-identity/README.md
