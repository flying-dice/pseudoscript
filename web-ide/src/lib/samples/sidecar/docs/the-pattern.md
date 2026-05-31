# Sidecar / Ambassador

What if your checkout code could make a plain local call to "pay" — and something *beside* it quietly added mTLS, retries, timeouts, and tracing on the way out?

## The problem

MeshMate runs a **Checkout** service whose job is one thing: take an order and charge it. But the moment that call crosses the network to **Payments**, a pile of operational concerns lands on it — mutual TLS so the connection is authenticated and encrypted, retries for transient blips, timeouts so a hung payment provider doesn't wedge checkout, and distributed tracing so an operator can follow the request end to end.

Bake all that into the checkout code and two bad things happen. First, the business logic — the part that should read like the domain — drowns in networking boilerplate. Second, every service that talks to anything has to re-implement the same boilerplate, and each does it slightly differently. Upgrade your TLS policy and you're now editing a dozen services in a dozen languages.

## The pattern

MeshMate co-deploys a **Sidecar** proxy right beside the checkout service — same pod, same lifecycle. Checkout makes a *plain local call*: `Checkout.pay(order)` builds a `Charge from { order, 42 }` and hands it to `Sidecar.forward(charge)`. As far as the checkout code knows, that's the whole story.

The **Sidecar** owns everything that happens on the wire. Its `forward(charge)` is where mTLS, retries, timeouts, and tracing live; once those are applied, it calls `Payments.charge(charge)` and returns the `Receipt { ok }`. The **Payments** service is reached *only* through the sidecar — checkout never holds a connection to it.

That's the whole move, and the `ProxyCrossCutting` feature states the guarantee precisely: given checkout making a plain local call, when the call leaves the pod the sidecar adds mTLS, retries, timeouts, and tracing and forwards to payments — "but the checkout code contains none of that." The cross-cutting concerns are deployed alongside the app, not compiled into it. (When the proxy specifically represents and brokers a *remote* dependency like Payments, this shape is often called the **Ambassador**.)

## When to use it

Reach for a sidecar when many services — especially in mixed languages — need the same cross-cutting behaviour: mTLS, retries, timeouts, observability, traffic policy. It's the backbone of a service mesh. It's also the clean way to add resilience to a legacy service you can't or won't modify: wrap it, don't rewrite it.

## When to avoid it

For a single service, or one with no meaningful network egress, a sidecar is operational weight for no gain. In tight latency budgets, the extra in-pod hop and the proxy's own resource footprint may not be worth it.

## Trade-offs

You double your running processes — every pod now carries a proxy to deploy, monitor, patch, and resource. The sidecar adds a network hop and can itself fail. In exchange, your application code stays pure domain logic, and operational policy lives in one uniform, language-agnostic layer you can upgrade fleet-wide without touching a line of business code.
