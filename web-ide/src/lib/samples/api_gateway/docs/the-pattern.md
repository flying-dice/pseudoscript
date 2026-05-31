# API Gateway

When every client request enters through one front door, the messy work of the edge — authentication, rate limiting, routing — has exactly one place to live.

## The problem

Nomnom is a food-delivery app. A hungry **Diner** opens the phone app, browses a restaurant's menu, places an order, and watches a courier creep across the map. Behind that single screen sit three very different services: **Menus**, **Ordering**, and **Couriers**.

The naive design lets the app call each service directly. Now every one of those three services has to authenticate the diner, enforce rate limits, and sit exposed on the public internet. The auth logic gets copy-pasted three ways and drifts. A new service means a new public endpoint to secure. The mobile team has to know the address and contract of every backend, and a refactor on the server side ripples straight out to phones in the wild that nobody can force-update.

## The pattern

Nomnom puts one **Gateway** container in front of everything. The `Diner`'s `order(restaurant)` call goes to `Nomnom::Gateway.placeOrder(restaurant)` — and *only* to the gateway. The app never holds an address for a backend.

Walk the routes on `Gateway`:

- `menu(id)` is exposed as `GET /restaurants/{id}/menu` and forwards to `Menus.show(id)`.
- `placeOrder(restaurant)` is `POST /orders` and forwards to `Ordering.place(restaurant)`.
- `track(id)` is `GET /orders/{id}/courier` and forwards to `Couriers.locate(id)`.

Each backing service — `Menus`, `Ordering`, `Couriers` — is a plain container that returns a `Reply { status, body }` and is reachable *only* through the gateway. They never face the internet.

The guarantee is captured in the `SingleFrontDoor` feature: when a diner's order reaches the gateway, the gateway "authenticates and rate-limits it once, centrally," then routes it to ordering — and the app never touches a backing service directly. Auth and rate limits live in one place, the public surface is one host, and the backends are free to evolve their own contracts as long as the gateway keeps translating.

## When to use it

Reach for an API gateway when many clients talk to many backend services and the cross-cutting edge concerns — authN/authZ, rate limiting, TLS termination, request logging — would otherwise be duplicated per service. It shines when you want internal services off the public internet, and when client and service teams need to evolve independently behind a stable façade.

## When to avoid it

A single service with one client doesn't need a gateway; it adds a hop and a deploy for nothing. Avoid pushing business logic into the gateway — once it starts *deciding* rather than *routing*, it becomes a distributed monolith's chokepoint.

## Trade-offs

The gateway is a single point of failure and a latency tax on every call, so it must be replicated and kept thin. It's also a shared deploy: a careless route change can break every client at once. The payoff is real, though — one place to secure, one place to observe, and backends that never have to think about the open internet.
