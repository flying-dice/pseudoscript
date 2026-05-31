# CQRS

Giving someone kudos happens a few times a day. Looking at the leaderboard happens constantly — open the app, glance, refresh, glance again. Those two jobs have nothing in common except the data they share. CQRS is what happens when you stop pretending they do.

## The problem

**Kudos** is a peer-recognition app. A **Teammate** thanks a colleague — a trickle of writes. Then the whole company stares at the **leaderboard** — a flood of reads. If both run off one model, you're stuck. Tune the schema for clean, consistent writes and the leaderboard query gets expensive: it has to aggregate kudos on every page load. Tune it for fast reads — a denormalised, pre-summed table — and every write has to keep that summary correct under contention. One table, two opposing masters. You can't scale the reads without dragging the writes along, and you can't deploy a read optimisation without risking the write path.

## The pattern

CQRS — Command Query Responsibility Segregation — splits the model in two. Writes go through one path, reads through another, and they're allowed to look completely different.

Walk Kudos by name. A `Teammate.thank(mate)` calls `Kudos::Commands.give`, the write side. `Commands.give` is the only thing exposed as `POST /kudos`; it hands the command to `WriteModel.apply`. `WriteModel` is the normalised, consistency-first store — and crucially it *emits an event*, `KudosGiven`, rather than updating any leaderboard itself.

That event is the seam. `Projector` listens for `KudosGiven` (`#[onevent(KudosGiven)]`) and calls `Standings.bump`. `Standings` is the read side: a denormalised leaderboard, shaped for exactly one job, served at `GET /leaderboard` via `board()`. No teammate ever writes to `Standings` directly, and no leaderboard read ever touches `WriteModel`.

So the flow is: write lands on `Commands` → `WriteModel` records it and emits → `Projector` translates the event → `Standings` serves it. Two models, one event stream between them. The `SplitReadAndWrite` feature pins the payoff: the projector updates the leaderboard from the emitted event, the leaderboard absorbs all the read traffic, and "the two sides scale and deploy independently."

## When to use it

When read and write workloads are genuinely lopsided or genuinely different in shape — high read fan-out, expensive aggregations, or a read model that wants a different storage engine than the write model. It pairs naturally with event-driven systems where the write side already emits events.

## When to avoid it

When reads and writes are symmetric and modest. A CRUD admin screen does not need two models, a projector, and an event bus; that's three moving parts solving a problem you don't have. The complexity tax is real — only pay it where the asymmetry is real.

## Trade-offs

You buy independent scaling and two purpose-built models. You pay with eventual consistency (the leaderboard lags the write by a projection hop), more infrastructure, and two schemas to evolve in lockstep. CQRS trades simplicity for the freedom to optimise each side without compromise.
