# Bounded Contexts

ACME Tickets is fourteen modules, one per bounded context. Each owns a slice of the domain, exposes a narrow interface, and trusts its neighbours only through their published contracts. This is the C4 container map: what each box owns, and how a buy actually threads through them.

## The system context

`context.pds` is the outermost frame. It declares the people — `Attendee`, `Organizer`, `GateStaff`, `SupportAgent` — and the two external systems, `PaymentProvider` and `NotificationProvider`. The actors carry real behaviour: an `Attendee` *queues*, *holds*, and *checks out*, and each of those calls lands on a `gateway` route. Every actor-to-system edge in the diagram is a real call path, not decoration.

## The contexts and what they own

**`gateway`** is the public HTTP edge. Each route (`QueueApi`, `ReservationApi`, `CheckoutApi`, `AccountApi`, `EventApi`, `UserApi`) is a thin entry point that authenticates and delegates straight into a domain service. Browse traffic is deliberately served from a cacheable read model, never the system of record.

**`identity`** owns accounts, sessions, and roles. `Sessions.authenticate` resolves a token to a user; `requireOrganizer` and `requireSupport` add the role gate. Every privileged action in the platform passes through here.

**`catalog`** is the source of truth for the *offer*: venues, `Event`s and their lifecycle (`Draft → OnSale → SoldOut/Closed/Cancelled`), and the price `Tier`s within each event. It splits authoritative writes (`Events`, `Tiers`) from a cacheable `Listings` read model the storefront hits under load. When an organiser cancels an event, `catalog` raises `EventCancelled`, which fans out into bulk refunds.

**`waitingroom`** is the surge-volume valve. Attendees join a `Queue`, and the `Gatekeeper` admits them only as fast as there is headroom, minting an `AdmissionToken`. `Admission.verify` is the gate every reservation and checkout must pass before doing anything expensive.

**`inventory`** owns the seat truth and the no-oversell guarantee. `Holds` reserves, validates, commits, confirms, and releases time-boxed claims against a sharded, atomically-allocated `Pool`. It also publishes a cacheable `Capacity` snapshot the waiting room reads to size its admission budget.

**`pricing`** turns base price into the price you pay. `Quotes` scales a tier's base by a surge multiplier derived from live `Demand`; `Locks` freezes that quote to your hold so it can't move under you. Demand ingests confirmed sales asynchronously, off the checkout hot path.

**`orders`** is the checkout saga and the heart of the platform. `Reservation.hold` takes an admission-gated inventory hold and locks a price; `OrderService.checkout` re-verifies admission, charges, and confirms the hold into tickets, with a compensating action behind every money step. It raises `OrderConfirmed` and `OrderRefunded`, the events the rest of the system reacts to.

**`payments`** is the hardened third-party integration. `PaymentService` runs an idempotent charge, owns the payment ledger, and reconciles settlement against both a push (`WebhookHandler`) and a pull (`reconcile`). `Gateway` is the only component that talks to the external `PaymentProvider`.

**`tickets`** mints credentials for a confirmed allocation, delivers them, revokes them on refund, and redeems them once each at the gate. Issuance that can't complete is quarantined for a human, never retried forever.

**`notifications`** reacts to the three moments that matter — `Admitted`, `OrderConfirmed`, `OrderRefunded` — and sends mail or SMS through the external provider.

**`backoffice`** is the staff console. Organisers create, configure, and launch events; support agents refund orders and review stranded ones. Every action is role-gated through `identity`, so the authorisation decision is disclosed here, not left to the channel.

**`batch`** is the unattended machinery: the `Drainer` (admits at rate), `Curator` (advances event lifecycle), `HoldReaper` (returns abandoned seats), `SagaSweeper` (recovers crashed checkouts), and `Reconciler` (pulls provider truth for missed webhooks). Its unit of work is one event, so one event's backlog never starves another's.

## How a buy threads through

A single purchase touches most of the map in order: `gateway` authenticates and admits, `waitingroom` gates, `orders` reserves through `inventory` and prices through `pricing`, then `orders` charges through `payments`, confirms the `inventory` hold, and raises `OrderConfirmed` — which `tickets` and `notifications` consume independently. `batch` runs underneath the whole thing, keeping the queue draining and any stuck saga moving.

The contexts collaborate two ways: **synchronous calls** down the checkout path (gateway → orders → inventory/pricing/payments), where a result is needed now, and **events** outward from `orders` and `catalog`, where consumers (`tickets`, `notifications`, `pricing::Demand`) react on their own time and own their own idempotency. That split — call where you need an answer, publish where you don't — is what keeps the hot path short.
