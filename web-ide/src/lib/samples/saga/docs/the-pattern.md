# The Saga Pattern (Orchestrated)

A traveller on **Voyagr** books one trip in a single click. Behind that click, three independent suppliers must all say yes: a **Flights** system, a **Hotels** system, and a **Cars** system. None of them shares a database with the others. None of them will hold a row lock open while you go ask the next one. And yet the traveller must never end up with a flight and a hotel but no car — a half-booked trip nobody will pay for.

This is the saga problem, and Voyagr solves it with an orchestrated saga.

## The problem

A classic database transaction gives you atomicity: a single `COMMIT` makes every write land or none of them land. That guarantee evaporates the moment your "transaction" spans services. `Flights`, `Hotels`, and `Cars` are three separate systems with three separate commits. There is no two-phase lock you can wrap around all of them, and even if there were, you would not want to hold supplier inventory hostage while a slow third party decides.

So you give up atomicity and buy back *consistency* a different way: by making every forward step reversible, and undoing the completed ones when a later step fails.

## The pattern

The `Planner` container is the orchestrator. It runs `book(trip)` and drives the steps in order, checking each result before moving on:

1. `Flights.reserve(trip)` — if it returns `Err`, nothing has happened yet, so `Planner` just returns `Err(StepFailed from { "flight" })`.
2. `Hotels.reserve(trip)` — if this fails, the flight is already booked, so `Planner` calls the **compensating action** `Flights.cancel(trip)` before returning the failure.
3. `Cars.reserve(trip)` — if this fails, two steps are live. `Planner` compensates **in reverse**: `Hotels.cancel(trip)`, then `Flights.cancel(trip)`.

Only when all three reserves succeed does `Planner` return `Ok(Itinerary from { trip.id })`. Each supplier exposes the pair the saga depends on: a forward `reserve` and a compensating `cancel`. The `CompensateOnFailure` feature pins the contract — flight and hotel reserved, car fails, and the saga cancels the hotel then the flight, *in reverse order*, with no distributed lock held across suppliers.

## Orchestration vs choreography

Voyagr's saga is **orchestrated**: one component, `Planner`, knows the whole journey. It calls each step, inspects each result, and decides what to compensate. Read `Planner.book` top to bottom and the entire flow — including rollback — is right there.

The sibling example, **Saga (Choreographed)**, takes the opposite stance: there is no orchestrator. Each service reacts to an event and emits the next, so the flow *emerges* rather than being directed. Orchestration centralises the logic (easy to follow, easy to see the whole saga, but the orchestrator becomes a hub everything depends on). Choreography distributes it (services stay decoupled and independently deployable, but no single place describes the end-to-end journey). Same guarantee, opposite centre of gravity.

## When to use it

- The transaction spans services with no shared commit, and you can define a compensating action for each step.
- The flow has real branching or ordering logic that benefits from living in one readable place.
- You need clear visibility of where a booking is in its lifecycle.

## When to avoid it

- A single service with a real ACID database can just use a transaction — don't reach for a saga.
- A step is genuinely non-compensatable (an irreversible email, a settled payment) with no business-level undo.
- The coupling of an orchestrator that calls every service is unacceptable; choreography may fit better.

## Trade-offs

You trade atomicity for eventual consistency. Between `Flights.reserve` and the final `Ok`, the trip is *partially booked* — other readers can see inventory that may yet be rolled back (no isolation). Compensation is your own code, not the database's, so it can have bugs and can itself fail. In return you get a transaction that crosses service and supplier boundaries without a distributed lock, and — with orchestration — a single component that tells the whole story.
