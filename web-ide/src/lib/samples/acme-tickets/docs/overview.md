# ACME Tickets

A hot show goes on sale at 10:00. At 09:59:59 a queue of ten thousand people is sitting on the event page, fingers over the "buy" button. At 10:00:00 they all press it. There are four thousand seats.

That single second is the entire design problem. Everything in ACME Tickets — a surge-aware event-ticketing platform modelled here as fourteen bounded contexts — exists to turn that thundering herd into a stream the rest of the system can serve correctly: no oversold seats, no double-charged cards, no buyer left holding a charge with no ticket.

## The hard part

Naively, ticketing is a shopping cart. The difficulty is entirely in the corners that only appear under load and failure:

- **Concurrency.** Two people reach for the last seat in the same millisecond. Exactly one must win.
- **Volume.** The request rate at on-sale is orders of magnitude above steady state, and it is spiky and brief. You cannot provision for the peak; you have to *shape* it.
- **Money over a network.** Payment runs through a third party you do not control. Its calls can time out with the charge in an unknown state. A buyer charged but un-ticketed is the worst outcome the system can produce, and a network blip must never cause it.
- **Fairness.** If the price someone was quoted can jump while they type in their card number, surge pricing becomes a bait-and-switch.

## The architecture at a glance

The C4 system context (`context.pds`) names the platform `AcmeTickets`, four kinds of person — `Attendee`, `Organizer`, `GateStaff`, `SupportAgent` — and two external systems it integrates with: a `PaymentProvider` and a `NotificationProvider`. ACME Tickets never sees a card number; only provider tokens and references cross the boundary.

Inside, the work is split into bounded contexts, each its own module:

- **`waitingroom`** — the virtual queue. Admits buyers at a controlled rate.
- **`catalog`** — events, venues, price tiers. The system of record for what is on sale.
- **`identity`** — accounts, sessions, role checks.
- **`inventory`** — finite seat pools and time-boxed holds. The no-oversell guarantee lives here.
- **`pricing`** — demand-based surge pricing, locked to a hold at reservation time.
- **`orders`** — the reserve → price → pay → issue checkout saga.
- **`payments`** — the hardened third-party integration: idempotent charge, webhook settlement, reconciliation.
- **`tickets`** — issuance, delivery, revocation, redemption at the gate.
- **`notifications`** — the moments that matter: you're in, you're confirmed, you're refunded.
- **`gateway`** — the public HTTP edge.
- **`backoffice`** — the staff console (role-gated).
- **`batch`** — the scheduled jobs that keep all of this healthy.
- **`shared`** — the value objects every context uses.

## The headline guarantees

Four promises hold across every failure mode the design anticipates:

1. **No oversell.** Seats are taken from a tier's pool by an *atomic* allocation (`inventory::Pool`). Two requests for the last seat cannot both succeed — this is enforced at the data layer, not by hoping concurrency stays low.
2. **A controlled admission rate.** The `waitingroom::Gatekeeper` only admits as many buyers as the inventory and payment paths currently have headroom for. The herd never reaches checkout all at once.
3. **A locked price.** The surge multiplier is computed from live demand and *frozen to your hold* the moment you reserve (`pricing::Locks`). It cannot climb while you check out.
4. **No charge without a ticket — or no ticket without a charge.** The checkout saga in `orders` gives every money step a compensating action, and a background sweeper reconciles any order stranded by a crash or a provider timeout. A buyer is never failed while their charge might be real.

The rest of these docs walk through how each guarantee is actually built — start with the bounded-context map, then the two deep dives on surge and the saga, then the peer-review of edge cases.
