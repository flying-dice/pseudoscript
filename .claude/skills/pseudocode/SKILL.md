---
name: pseudoscript
description: >
  Author and maintain PseudoScript (.pds) — C4-level architecture-as-code that compiles to
  diagrams and a doc site — as the single source of truth for spec-driven development. Use this
  skill whenever the user wants to model an application in PseudoScript, reverse-map / capture an
  existing app or codebase into a .pds model, drive development from a model (write the spec first,
  implement from it), capture business rules as code while keeping infrastructure (repositories,
  HTTP/controllers, persistence, queues, external APIs) as black boxes, write Gherkin-style
  `feature` behaviour specs, or reconstitute / generate a real implementation from a model. Trigger
  it on mentions of PseudoScript, `.pds`, "architecture as code", "C4 as code", "model the system",
  "spec-driven", "turn this app/codebase into pseudocode", or "what does the model say it should
  do" — even if the user doesn't name the language explicitly.
---

# PseudoScript — authoring & mapping

PseudoScript is a C4-level modelling language where **the model is the source**: business rules and
data provenance are written as high-level pseudocode, infrastructure is declared as signatures, and
the whole thing compiles to C4 diagrams, sequence diagrams, and a doc site (`pds doc`).

This skill is the **method**, not the grammar. For any syntax question — keywords, `Result`
handling, `from` composition, macros, modules, visibility, the EBNF — run **`pds lang`** to print
the full language reference (spec + patterns + the conformance/grammar suite) and follow it exactly.
Do **not** invent syntax; if the spec doesn't support a form (e.g. comparison operators in
conditions — see its Open Questions), model around it with a call that returns a `Result` or `bool`
instead.

Two jobs only:
- **Map** an existing application into a `.pds` model (reverse).
- **Author** a `.pds` model as the spec, then reconstitute the implementation from it (forward).

---

## The central contract

Everything below follows from one split: **essential complexity is disclosed as code; accidental
complexity is a black box.**

- **Disclose** (write the body with `{ }`) the things that decide and derive: use cases, validation,
  authorization rules ("who may do this", "when is this allowed"), state transitions, calculations,
  the order of operations, every error path, and where each value comes from (`from`).
- **Black-box** (declare the signature, end with `;`) the things that merely carry or store:
  repositories/DAOs/ORM, databases and caches, HTTP/serialization, message buses, third-party APIs.
  Their **signature is the contract**; their innards are not architecture.
- **Omit entirely** (model nothing) pure plumbing: status-code mapping, JSON shapes, token parsing,
  retries/timeouts, logging, metrics, DI wiring, connection pools.

Because every decision and every data derivation lives in disclosed bodies, and every boundary is a
typed signature, **the implementation is reconstitutable from the model**. The real app re-adds the
black boxes as adapters and the omitted plumbing as glue — but it invents no new business logic.

Authorization is the classic tell: *who is allowed* to open the account is a business decision →
disclose it as a branch. *How the JWT is parsed* is plumbing → omit it.

---

## Publish the contract on the system, not the container

A system's published API is a `public` callable **on the `system` node**. The container or component
that implements it stays module-private; the system delegates inward to it. Cross-system calls then
land on `system::Operation` — never on an internal container or component.

**Wrong** — the contract sits on the container, so the container is the only public thing and
consumers must name the implementation:

```pds
public system Banking;                          // black box, no contract

public container Mainframe for banking::core::Banking {
  #[http("POST /accounts")]
  public OpenAccount(req: banking::core::OpenRequest)
    : Result<banking::core::BankingInfo, banking::core::OpenError>;
}
// consumer must write:  banking::core::Mainframe.OpenAccount(req)   <- binds to the impl
```

**Right** — the contract sits on the system; the system delegates inward; `Mainframe` is private:

```pds
public system Banking {
  /// Published face: open a retail account.
  #[http("POST /accounts")]
  public OpenAccount(req: banking::core::OpenRequest)
    : Result<banking::core::BankingInfo, banking::core::OpenError> {
    r = Result<banking::core::BankingInfo, banking::core::OpenError>
          from banking::core::Mainframe.openAccount(req)   // delegate inward
    if (r.isErr) { return Err(r.error) }
    return Ok(r.value)
  }
}

container Mainframe for banking::core::Banking {            // NO `public`
  openAccount(req: banking::core::OpenRequest)
    : Result<banking::core::BankingInfo, banking::core::OpenError> { ... }
}
// consumer writes:  banking::core::Banking.OpenAccount(req)   <- lands on the box
```

Why it's enforced, not stylistic:

- **`public` is the boundary.** Leave the implementing container private and a consuming workspace
  *cannot* name it — the only legal target is the system. Privacy and the convention reinforce each
  other.
- **The trigger marks the entry point.** A trigger macro (`#[http]`, `#[onevent]`, …) belongs on the
  published face, because ingress arrives there and then routes inward.
- **Swap test.** Replace `Mainframe` with a Java `Mainframe2` behind the same system callable → zero
  consumer edits. If renaming an internal container forces edits in another workspace, the contract
  was on the wrong node.
- **One reason to change each.** The system owns the promise and the inward routing; the container
  owns the implementation. Two reasons to change → two nodes.

**Gate — only at a real seam.** A system with no cross-boundary API needs no facade; don't wrap a
purely-internal container in a delegating callable just to satisfy the rule. The trigger is a
`public` callable consumed from another module or workspace. No external consumer → no facade.

---

## Concern → construct map

Read it top-to-bottom when mapping an existing app; read it right-to-left when reconstituting code
from a model. (Constructs and call syntax are defined in the language reference — run `pds lang`.)

| App concern | PseudoScript | Disclosed? |
|---|---|---|
| HTTP route / controller / RPC handler | callable with `#[http("VERB /path")]` | body disclosed only for the orchestration that carries meaning; routing/serialization is the macro, not the body |
| Use case / application service / interactor | disclosed callable | **disclose** |
| Domain rule, validation, **authorization** decision | `if (r.isErr) { return Err(...) }` branches | **disclose** |
| Calculation / assembling a result from parts | `x = T from { a, b }` + the calls feeding it | **disclose** (provenance) |
| Repository / DAO / ORM mapper | black-box `component` (or `container`) with `fetch`/`save`/… signatures | black box |
| Database / cache / queue infrastructure | black-box `container`, often tagged `#critical` | black box |
| DTO / entity / domain event / message | `data` record; events as a discriminated union | fields disclosed when they matter; `data X;` otherwise |
| Third-party / external API | black-box `public system` with signature-only callables | black box |
| Scheduled job / cron | callable with `#[schedule = "cron"]` | body disclosed, macro = trigger |
| Event / message consumer | callable with `#[onevent(Event)]` | body disclosed |
| Job started by a person or CLI | callable with `#[manual]` | body disclosed |
| Actors (customer, admin, service) | `person` (may own actions it initiates) | usually black box |

---

## Workflow A — map an existing app into a model

1. **Find the boundaries.** Each deployable/owned service or actor becomes a `system` or `person`.
   External dependencies you don't own become black-box `public system`s.
2. **Pick containers, then components.** Inside a system, the runnable/storage units are `container`s
   (`for <System>`); split a container into `component`s only where the extra granularity helps.
   Tag infrastructure you depend on heavily (`#critical`).
3. **Model the data.** Lift DTOs, entities, events, and messages to `data`. Use a discriminated
   union for event/error families. Black-box (`data X;`) anything whose fields don't yet matter.
4. **Black-box the adapters first.** Repositories, gateways, buses, external APIs become
   signature-only callables. Get the contracts right; resist disclosing them.
5. **Disclose the use cases.** Translate each service/interactor method into a disclosed callable,
   tracing the **business logic line for line**: every guard becomes an `if (…isErr) { return Err }`,
   every assembled value becomes `from { … }`, every dependency call becomes a `Target.method(args)`.
   Every binding states its type through `from` (`x = T from …`). Keep bodies at flow-and-provenance level — never
   field-level arithmetic.
6. **Mark the entry points.** Attach the trigger macro that matches how each callable is actually
   initiated: `#[http]`, `#[onevent]`, `#[schedule]`, `#[manual]`. These become inbound edges and
   sequence-diagram entry points.
7. **Assert behaviour with `feature`s.** For each acceptance behaviour, write a `feature … for
   <Node>` in given/when/then. Steps are prose describing observable behaviour, not model calls.
8. **Check reconstitutability.** Ask: *could someone rebuild the app from this model alone?* If a
   real branch or rule isn't visible in some disclosed body, it's missing — add it. If a body
   contains serialization or status mapping, it leaked — remove it.

---

## Workflow B — author a model as the spec (spec-driven)

1. **Features first.** Write the `feature` scenarios for the behaviour you want, naming the node each
   is *for*. This is the acceptance contract before any structure exists.
2. **Name the structure.** Declare the `system` / `person`, the `container`(s), and `component`(s)
   the features imply.
3. **Declare data and ports.** Add the `data` types the features mention. Declare every repository /
   gateway / external dependency as a **black-box** callable — signatures only.
4. **Disclose the realising callables.** Write the bodies that make each feature true: guards as
   `Result` branches, derivations as `from`, dependency use as calls through the black boxes. Leave
   not-yet-designed steps as black-box callables (`Quote(...): number;`) and disclose them later —
   progressive disclosure is intended.
5. **Wire the triggers.** Add `#[http]` / `#[onevent]` / `#[schedule]` / `#[manual]` to entry points.
6. **Reconstitute the implementation** (next section).

---

## Reconstituting an implementation from a model

The model is the spec; the code is its faithful realisation. Translate, don't reinvent.

- **Disclosed callable → service/interactor method.** Preserve the order of operations and **every**
  `Err` arm. The decision logic is normative.
- **Black-box callable → adapter** against the real infra (SQL, HTTP client, broker). Its signature
  is the contract the adapter must satisfy; test the adapter separately for fidelity.
- **`data` → DTO/entity;** `from { … }` tells you exactly what assembles each value.
- **Trigger macro → wiring:** `#[http]` → route registration; `#[onevent]` → subscription;
  `#[schedule]` → scheduler entry; `#[manual]` → CLI/console entry.
- **`Result<T,E>` `Err` arms → error responses** at the edge. The HTTP **status mapping is added in
  the controller, never in the model.**
- **`feature` → an acceptance/BDD test** in the implementation, one per scenario. The implementation
  is "done" for that behaviour when its feature test passes.

**The invariant:** no business decision lives outside a disclosed body. If, while reconstituting,
you find yourself writing a branch in a controller or repository that isn't pure plumbing, stop —
that decision belongs back in the model.

---

## Worked example — mapping a real slice

A typical "place order" service in a real codebase: a controller, an application service, two
repositories, a payment client, and an event publish.

```ts
// POST /orders  (controller)         — routing + (de)serialization + status codes
const r = await orderService.place(req.body);
return r.isErr ? res.status(409).json(r.error) : res.status(201).json(r.value);

class OrderService {                  // the part that actually decides
  async place(cmd) {
    const reserved = await inventory.reserve(cmd.sku, cmd.qty);   // rule: must be in stock
    if (reserved.isErr) return Err(reserved.error);
    const quote = this.quote(cmd.sku, cmd.qty);                   // rule: pricing
    const order = Order.from(cmd, quote);
    const paid  = await payments.charge(order);                  // rule: must be paid
    if (paid.isErr) return Err(paid.error);
    await orders.save(order);                                     // persistence (infra)
    await bus.publish(new OrderPlaced(order.id));                 // announce
    return Ok(order);
  }
}
```

The model keeps the rules and provenance; the controller, ORM, payment SDK, and broker all collapse
to signatures and a trigger macro:

```pds
//! shop — order placement.

public person Buyer;

public data PlaceOrder  { sku: string, qty: number, card: string }
public data Order       { id: uuid, total: number }
public data OutOfStock  { sku: string }
public data Declined    { reason: string }
public data OrderError  = | OutOfStock | Declined
public data OrderPlaced { orderId: uuid }

public system Shop;

public container Checkout for shop::Shop {

  /// Reserve stock, price it, charge, persist, announce.
  #[http("POST /orders")]
  public PlaceOrder(cmd: shop::PlaceOrder): Result<shop::Order, shop::OrderError> {
    reserved = Result<void, shop::OutOfStock> from shop::Reservations.reserve(cmd.sku, cmd.qty)
    if (reserved.isErr) {
      return Err(reserved.error)
    }
    quote = number from self.Quote(cmd.sku, cmd.qty)
    order = shop::Order from { cmd, quote }
    paid = Result<void, shop::Declined> from shop::Payments.charge(order)
    if (paid.isErr) {
      return Err(paid.error)
    }
    shop::Orders.save(order)
    evt = shop::OrderPlaced from { order }
    shop::Bus.publish(evt)
    return Ok(order)
  }

  /// Pricing rule — disclose later.
  Quote(sku: string, qty: number): number;
}

/// Stock ledger.
/// #critical
public container Inventory for shop::Shop;
component Reservations for shop::Inventory {
  reserve(sku: string, qty: number): Result<void, shop::OutOfStock>;
}

/// Order records.
public container OrderStore for shop::Shop;
component Orders for shop::OrderStore {
  save(order: shop::Order): void;
}

/// Third-party payment processor.
public system Payments {
  charge(order: shop::Order): Result<void, shop::Declined>;
}

/// Domain event bus.
public system Bus {
  publish(event: shop::OrderPlaced): void;
}

/// A buyer places an order that is in stock and paid.
feature PlaceOrder for shop::Checkout {
  given "the SKU is in stock"
  and   "the card is valid"
  when  "the buyer places the order"
  then  "stock is reserved"
  and   "the card is charged"
  and   "an OrderPlaced event is published"
}
```

What happened: the controller became a `#[http]` macro (no body logic), the ORM/repos and the
payment SDK and the broker became black-box signatures, status codes and JSON were omitted — and the
three rules (in stock, priced, paid), their order, their error paths, and the order's provenance all
survive as code. From this slice alone you can rebuild the service.

---

## Review checklist

Before declaring a model done, confirm:

- [ ] **Reconstitutable** — every real branch/rule appears in some disclosed body; nothing essential
  hides in an adapter or controller.
- [ ] **Clean black boxes** — repos, gateways, buses, external systems are signature-only; no
  persistence or wire detail leaked into a disclosed body.
- [ ] **Contracts on the box** — a system's cross-boundary API is a `public` callable on the `system`
  node that delegates inward; the implementing container/component stays private, so no consumer
  names an internal node.
- [ ] **Errors are values** — fallible operations return `Result`; every `Err` arm is handled with an
  explicit `if (…isErr)`.
- [ ] **Provenance shown** — assembled values use `from { … }`; no value appears from nowhere.
- [ ] **Entry points marked** — each externally-initiated callable has the matching trigger macro.
- [ ] **Features cover the behaviour** — one `feature` per acceptance scenario, each `for` a real
  node, given/when/then in order, steps phrased as observable behaviour.
- [ ] **Spec-faithful syntax** — names are fully qualified or aliased; visibility is correct; nothing
  uses a form the language reference (`pds lang`) doesn't define.