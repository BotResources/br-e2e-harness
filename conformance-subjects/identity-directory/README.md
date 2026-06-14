# identity-directory — Px/Cx directory-wire anchor

A minimal **Go** program that emits the BotResources **identity Published
Language** (directory) wire — the read-only KV projection identity publishes and
generic services consume: `identity/_meta`, `identity/users/{uuid}`,
`identity/groups/{uuid}`.

Unlike the `scope-service` / `identity-passport` subjects (live black-box
services driven over NATS/HTTP), this anchor has **no live handshake to drive**:
the directory PL is a one-way KV projection, and a *live* projection is exercised
by the consumer-kit conformance (the Cx batteries) separately. This anchor's sole
job is to **freeze the on-wire bytes independently of the Rust lib** — it prints
the canonical directory snapshot to stdout, and the Rust conformance runner
deserialises every emitted value **through `br-core-directory`** (the oracle). If
the lib drifts (a renamed/retyped core field, changed serde, a broken flatten),
that deserialisation against this frozen Go wire fails — that is how the lib is
guarded.

> This is a **test fixture** — a wire anchor, not a service. It opens no socket
> and binds no infra.

The on-wire contract this anchor freezes is documented in
[`../../docs/conformance/directory-wire-v1.md`](../../docs/conformance/directory-wire-v1.md).
That document is authoritative; this README only describes how to run the binary.

## What it emits

`identity-directory` prints, to stdout, one canonical JSON document: the full
directory KV snapshot. The envelope is a thin Go transport (`{key, value}`
entries) so the runner can pair each KV key with its stored value; the **`value`
of each entry is the frozen wire** — exactly the bytes that would be stored at
that KV key, deserialisable through `br-core-directory`.

```json
{
  "meta":   { "key": "identity/_meta",                 "value": { "version": 1, "entities": ["users", "groups"] } },
  "users":  [ { "key": "identity/users/<uuid>",  "value": { "email": …, "first_name": …, "last_name": …, "x_custom": … } }, … ],
  "groups": [ { "key": "identity/groups/<uuid>", "value": { "name": …, "member_ids": […], "x_custom": … } }, … ],
  "snapshot_version": 1
}
```

The snapshot covers the **core + the generic extension mechanism**, nothing
project-specific:

- `_meta` declaring both published entities (`["users", "groups"]`).
- a user **with** the neutral extension `x_custom` (proving an extra key
  round-trips flat alongside the core) and a **core-only** user with `null`
  names (proving the names are emitted, never omitted).
- a group **with** the neutral extension and a **core-only** group with an empty
  `member_ids` array.

It deliberately does **not** emit `organization_id`, orgs, memberships or any
tenancy field: tenancy is a project extension, not the conformance socle (epic
#54 — `organization_id` is an extension, not core).

## The frozen directory wire (values)

```json
identity/_meta                 → { "version": 1, "entities": ["users", "groups"] }
identity/users/{uuid}          → { "email": "…", "first_name": "…"|null, "last_name": "…"|null, <extensions flat> }
identity/groups/{uuid}         → { "name": "…", "member_ids": ["<uuid>", …], <extensions flat> }
```

Core keys are snake_case, no `rename_all`, no `skip` — `first_name`/`last_name`
are emitted as `null` when absent (matching the Rust `Option<String>` with no
`skip_serializing_if`). Project extensions ride **flat** at the top level
(matching the Rust `#[serde(flatten)]`), never nested under an `extensions` key.

## Build & run

```sh
# build
go build -o identity-directory .     # or: make build

# emit the canonical wire snapshot
./identity-directory                 # prints the JSON document above to stdout
```

```sh
make test       # go vet + go test (the golden-shape + KV-key + flatten tests)
```

The offline golden tests (KV-key prefixes, the user/group/meta golden shapes,
the flat-extension and null-names invariants) run with plain `go test` — no
infra. Pairing the Go-frozen wire against the live `br-core-directory` types is
the Rust conformance runner's job (WU9): it builds this binary, runs it once,
and deserialises every `value` through the lib.

## Why table

| Thing | Why it is the way it is |
|---|---|
| Emits to stdout, opens no socket | The directory PL is a KV projection, not a live handshake; WU8 freezes the *bytes*, so a print-the-canonical-wire program is the faithful anchor. A live projection is the consumer-kit (Cx) conformance, a separate concern. |
| `{key, value}` transport envelope around each entry | Lets the runner pair a KV key with its stored value in one document; only the `value` is the frozen wire — the envelope is harness transport, never deserialised through the lib. |
| `first_name`/`last_name` emitted as `null`, not omitted | Byte-matches the Rust `Option<String>` core fields, which have no `skip_serializing_if` — a core key is always present on the wire. |
| Neutral extension `x_custom` rides flat alongside core | Byte-matches the Rust `#[serde(flatten)] extensions` — an extra key sits at the top level and lands in the lib's `extensions` map. A neutral key (not `organization_id`) keeps the socle tenancy-agnostic. |
| `member_ids` always a JSON array (empty when no members) | Byte-matches `Vec<Uuid>`; membership is derivable from `member_ids`, so an empty group is `[]`, never absent. |
| No `organization_id` / orgs / tenancy | Tenancy is a project **extension**, not the conformance core (epic #54). The anchor freezes only the core + the generic extension mechanism. |
| No NATS / no KV bind | The anchor never auto-provisions and never touches infra — it is a pure, deterministic wire emitter; the runner owns any KV setup the Cx batteries need. |
