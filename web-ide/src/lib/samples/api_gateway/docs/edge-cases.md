# API Gateway

The model shows the happy path: one diner, one order, three clean forwards. Production is where the gateway earns its keep — and where its sharp edges show.

## Edge cases & failure modes

- **The gateway *is* the blast radius.** Every Nomnom request rides through `Gateway`. If it falls over or chokes on CPU, menus, ordering, *and* courier tracking all go dark at once — even though the backends are healthy. The single front door is also a single point of failure.
- **A slow backend leaks upward.** `placeOrder` forwards to `Ordering.place` and blocks. If `Ordering` is slow, gateway threads pile up waiting on it and starve unrelated `menu` and `track` traffic. One sick service can take the whole edge down via the shared gateway.
- **Auth at the door, trust on the inside.** Because `Menus`, `Ordering`, and `Couriers` "never face the internet," it's tempting to skip auth between gateway and backend. Anyone who lands inside the network — a misconfigured pod, a leaked credential — can then call `Ordering.place` directly. The internal boundary still needs identity.
- **Routing rot.** The `#[http]` routes (`POST /orders`, `GET /orders/{id}/courier`) are a contract the phone app hard-codes. Change a path or a field in `Reply` and old app versions in the wild break silently. Versioning lives here, and the model doesn't show it.
- **Coarse rate limits punish neighbours.** "Rate-limit it once, centrally" is one knob. One abusive diner hammering `track` can trip a global limit that throttles everyone else's `placeOrder`.

## Resilience

Run the gateway as several stateless replicas behind a load balancer so losing one instance is a non-event. Give each downstream its own **timeout** and **bulkhead** (a bounded connection pool) so a slow `Ordering` can't starve `Menus`. Wrap forwards in a **circuit breaker** that fails fast and sheds load when a backend is unhealthy, and pair it with **retry** (with jittered backoff) for transient blips — but only on idempotent reads like `menu` and `track`, never a naive retry of `POST /orders`. Apply rate limits *per diner and per route*, not one global bucket. This deliberately small model omits all of it — no timeouts, no health checks, no auth between gateway and backends, no API versioning — so the routing idea stays legible.

## Pairs well with

- **Circuit breaker** and **retry** — the standard pair for hardening each forward to a backend.
- **Idempotent receiver** — so `Ordering.place` can be safely retried without double-charging a diner.
- **Backends for Frontends** — the same edge idea, sharpened per client device.
- **Sidecar / Ambassador** — push mTLS and tracing into a proxy so the gateway and backends stay thin.
