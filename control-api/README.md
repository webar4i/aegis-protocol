# Aegis Control API — planned contract

Design only. No Go service, database migrations, REST endpoints, credential issuer or executable panel adapter is implemented in v0.1. The Rust echo prototype operates independently using local certificate files.

The intended implementation is a Go service with PostgreSQL and a versioned REST API. It manages identities and desired policy; Rust data-plane nodes transfer packets. A management outage must not turn a node into an unauthenticated relay. Define bounded cached-policy lifetime and fail-closed admission before implementation.

## Proposed resource model

| Resource | Required fields/operations |
| --- | --- |
| User | Opaque ID, enabled state, expiry, usage policy; create/update/disable/delete |
| Device | User ID, public-key/certificate identity, credential generation, revoke/rotate |
| Server | Opaque ID, addresses, supported transports, health and policy generation |
| Policy | Byte/rate/concurrent-session limits, address assignment, validity interval |
| Usage | Node/device ID, cumulative ingress/egress counters, monotonic report generation |
| Client configuration | Versioned endpoints/trust data/capabilities; no CA signing key |

Use separate administrator, node and user credentials. Restrict nodes to their own policy and counters; a node must not issue administrator tokens or fetch unrelated private keys. Deliver policy over mutually authenticated management connections or an equivalently reviewed channel. Device private keys should be generated on the device where possible; enrollment approves a public key/CSR. CA signing material belongs in a separate protected issuer.

## Correctness constraints

Mutations need idempotency keys and audit records that exclude credentials. Usage reporting needs stable report IDs or cumulative counters with reset generations, so retries and server restarts cannot double-charge users. Define exactly which bytes count and whether both directions count. Account expiry and device revocation require bounded propagation and termination of existing sessions, not merely hiding configurations.

Quota enforcement must operate on the data node with an explicit budget allocation when a user spans several nodes. Approximate periodic counters alone cannot guarantee a strict global quota. Limit changes must be acknowledged by node policy generation before the API reports them as applied.

The first executable adapter should synchronize explicit mappings through APIs; it must not edit the panel database or generate fake Xray inbounds. See [3x-ui research and bridge plan](../integrations/3x-ui/README.md). Define an OpenAPI document and contract tests only when resource semantics are ready to implement.
