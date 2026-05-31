# The Pattern

ACME Bank runs a thirty-year-old mainframe core. Nobody wants to rewrite it in a single heroic weekend — that is how banks end up on the front page. **Strangler Fig** shows how ACME replaces it one feature at a time, with customers none the wiser.

## The problem

ACME's online banking sits on top of `Mainframe`, a legacy core that has served every balance check, transfer, and statement for three decades. It works, but it is expensive, fragile, and nobody left understands it. The bank wants to move onto `Core`, a modern replacement — but a big-bang cutover means freezing development for a year and betting the whole institution on one release night.

The naive alternative is just as bad: run old and new side by side and ask every channel (web, mobile, ATM, call-centre tooling) to decide for itself which backend to call. Now the migration logic is smeared across a dozen clients, each with its own copy, each drifting out of sync.

## The pattern

The strangler fig grows around its host tree and slowly replaces it. Here the "fig" is the `Facade` container, and it is the *only* thing the outside world talks to. The `Customer` person hits `AcmeBank::Facade.handle(req)` and never addresses a backend directly.

Inside `handle`, one decision does all the work:

1. `handle` asks `self.isMigrated(req.path)` — has this feature moved to the modern core yet?
2. If the result `isOk`, the request goes to `Core.serve(req)`.
3. Otherwise it falls through to `Mainframe.serve(req)`.

That is the whole pattern: a single router in front of both systems, keyed on which slices have migrated. On day one, `isMigrated` returns `NotMigrated` for everything and every request lands on the mainframe. As teams rebuild features in `Core`, they flip them over in the routing config one at a time. `Core` grows feature by feature until it owns everything; `Mainframe` shrinks until the day the last route flips and it is switched off for good.

The `RouteOldOrNew` feature pins the guarantee exactly: a migrated feature is served by the modern core, an unmigrated one by the mainframe, *but the customer cannot tell which served it*. The façade makes the seam invisible.

## When to use it

Reach for the strangler fig whenever you must replace a large, working system you cannot afford to stop. It fits legacy migrations, monolith-to-services decomposition, and platform moves — anywhere a big-bang rewrite is too risky and the system can be sliced along feature or route boundaries. It shines when you can route by a stable key (a URL path, a customer segment) and roll back a single slice without touching the rest.

## When to avoid it

Skip it when the old system is small enough to rewrite outright — the façade and dual-running overhead cost more than they save. Skip it when old and new share mutable state that can't be cleanly partitioned, because then "which backend owns this data" has no clean answer. And avoid it when the seams don't fall on a routable boundary: if every request touches both systems at once, there is nothing to route.

## Trade-offs

The strangler trades a clean cutover for a long coexistence. For months or years you run two cores at once — double the infrastructure, double the on-call, and a `Facade` that is now a single point of failure for *every* channel. The routing config becomes load-bearing and must be tested as carefully as code. The reward is that risk is paid down in small, reversible increments instead of one terrifying lump.
