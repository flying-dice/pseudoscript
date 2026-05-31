# T14 — Core composed-pattern example workspaces (#web-ide content)

## Summary

Author **3–4 new flagship sample workspaces** for the web IDE. Each is a
believable business/application — like the existing `acme-tickets` — that
**composes several textbook design patterns**, in contrast to the ~22
existing single-pattern samples (`saga`, `cqrs`, `outbox`, `bff`,
`circuit_breaker`, …) that each isolate one pattern. The new workspaces
become the headline `Application`-category samples.

Authoring is **content-only**: `.pds` + Markdown + `meta.json` + `pds.toml`.
**No IDE code changes and no build step** — samples are discovered at build
time by a Vite `import.meta.glob` over `src/lib/samples/*/`. Drop a folder,
it appears. Each workspace must resolve clean (`pds doc .`) and its docs
must render.

NOTE: an earlier draft of this refinement assumed a `modules/` subdir, a
`[workspace]`/`[modules]` `pds.toml`, a `kind/featured/tags` `meta.json`,
and a `build-samples.mjs` → generated `samples.js` codegen step. All of
that was wrong. The real, verified shapes below override it.

## Current state (file:line) — acme-tickets shape + registration

The one existing flagship is `acme-tickets`. Its shape is the bar.

**Flat module layout** (no `modules/` subdir). One `.pds` per bounded
context at the workspace root:
`web-ide/src/lib/samples/acme-tickets/` contains `context.pds`,
`shared.pds`, `catalog.pds`, `orders.pds`, `payments.pds`, `gateway.pds`,
`identity.pds`, `inventory.pds`, `notifications.pds`, `pricing.pds`,
`tickets.pds`, `waitingroom.pds`, `backoffice.pds`, `batch.pds` (14
modules), plus `logo.svg`, `pds.toml`, `meta.json`, and `docs/`.

- `web-ide/src/lib/samples/acme-tickets/pds.toml:1` — a `[doc]` table
  (`name`, `out = "target/doc"`, `logo`, `theme`) followed by repeated
  `[[doc.sidebar]]` groups, each `{ title, items = [{ title, path }] }`
  pointing at `docs/*.md`. **Not** `[workspace]`/`[modules]`. The leading
  comment notes module FQNs derive from each file's path
  (LANG.md §8.1, ADR-017) — there is no `modules` map to maintain.
- `web-ide/src/lib/samples/acme-tickets/meta.json:1` — drives the IDE
  list. Fields actually present: `name`, `category` (`"Application"`),
  `order` (acme = `1`), `landing` (`"context"` — the `.pds` whose
  diagram opens first), `description`. **No** `kind`/`title`/`summary`/
  `tags`/`featured` fields exist.
- `.pds` shape — verified from `acme-tickets/saga.pds`-style siblings and
  the `saga` sample (`saga/saga.pds:6-55`): top-level `public data X {…}`,
  `public person X { … }`, `public system X;` (or `X { sig; sig; }`),
  `public container X for System { method(args): Result<T, E> { … } }`,
  and `feature X for Y { given/when/then/but "…" }`. **There is no
  `module` / `import` / `export` keyword** — files link by fully-qualified
  name (e.g. `Voyagr::Planner.book(...)`, `saga/saga.pds:14`), and FQNs
  derive from file path. Cross-file references = call an FQN that exists
  in another file in the same workspace.
- `web-ide/src/lib/samples/acme-tickets/docs/` — six Markdown files
  grouped by the sidebar: `overview.md`, `contexts.md`, `surge.md`,
  `checkout-saga.md`, `payments.md`, `edge-cases.md`. Narrative,
  business-first, each deep-dive doc walks one composed flow.

Single-pattern samples (the catalog to compose from) use a flatter
variant: one root `.pds`, `meta.json` (`category` like `"Transactions"`,
`order` 40+, `landing` = the pattern's `.pds`), `pds.toml` with one
`[[doc.sidebar]]` group, and `docs/the-pattern.md` + `docs/edge-cases.md`
(see `saga/meta.json:1`, `saga/pds.toml:1`, `saga/docs/the-pattern.md:1`).

Available patterns to compose (directory names under `samples/`):
`anti_corruption_layer`, `api_gateway`, `bff`, `bulkhead`, `cache_aside`,
`choreography_saga`, `circuit_breaker`, `claim_check`,
`competing_consumers`, `cqrs`, `event_sourcing`, `idempotent_receiver`,
`leader_election`, `outbox`, `pub_sub`, `rate_limiter`, `retry`, `saga`,
`scatter_gather`, `sharding`, `sidecar`, `strangler_fig`.

**Registration is automatic and build-free.** The catalogue is built by
`web-ide/src/lib/samples.js:13-55`: Vite `import.meta.glob` eagerly pulls
`./samples/*/*.pds`, `./samples/*/meta.json`, `./samples/*/pds.toml`, and
`./samples/*/**/*.md`. A folder is a sample iff it has a `meta.json`
(`samples.js:44` — a `.pds` with no sibling `meta.json` is ignored). Each
`.pds`'s **FQN is its basename** (`samples.js:46`: `fqn = file without
.pds`), so module-name = filename — `context.pds` is module `context`.
`landing` chooses the first diagram (`samples.js:64`); `order` sorts the
list (`samples.js:73`; acme `1`, patterns `40+`, default `999`).
**=> Adding a workspace = create the directory with `meta.json` +
`pds.toml` + `.pds` files + `docs/`. Nothing else. No codegen, no manifest
to edit, no build script to run.** (The dev server hot-reloads the glob;
a fresh `npm run dev` / `vite build` picks it up.)

## Proposed 3–4 examples

Domains chosen to be non-overlapping with acme-tickets (ticketing / saga /
surge) and with each other, each foregrounding a *different* pattern
cluster so the flagships together cover the catalog breadth.

### A. `rideroute` — ride-hailing dispatch
Patterns composed: **saga + outbox + circuit_breaker + rate_limiter + cqrs**.
- Story: match riders to drivers, run the trip, charge a fare, survive a
  flaky maps/ETA provider and a flaky PSP.
- Modules (one `.pds` each, flat):
  - `context.pds` — `person Rider`, `person Driver`, the top-level
    `system RideRoute` and the external systems (Maps, PSP, Push).
  - `shared.pds` — shared `data` (`Trip`, `Fare`, `TripEvent`, …).
  - `dispatch.pds` — **cqrs** (write: ride state; read: driver-location
    projection), **circuit_breaker** wrapping the Maps/ETA system,
    **rate_limiter** on the rider request entrypoint.
  - `trips.pds` — trip-lifecycle **saga** (reserve driver → start →
    charge → settle, compensating in reverse), **outbox** publishing
    `TripCompleted`.
  - `payments.pds` — fare capture + driver payout against the PSP system.

### B. `mercato` — e-commerce order fulfilment
Patterns composed: **outbox + idempotent_receiver + saga + bff + api_gateway**.
- Story: place an order, reserve stock, ship it, keep read models in sync,
  never double-decrement inventory.
- Modules:
  - `context.pds` — `person Shopper`, `person WarehouseOp`, `system Mercato`,
    external PSP + carrier systems.
  - `shared.pds` — `Order`, `LineItem`, `OrderEvent`, `Shipment`.
  - `storefront.pds` — **bff** for web/mobile behind an **api_gateway**
    container; read-side catalog/cart.
  - `ordering.pds` — order **saga** (reserve inventory → charge →
    create shipment → confirm), **outbox** emitting domain events.
  - `inventory.pds` — **idempotent_receiver** of stock-reservation events
    (dedupe by message key); the no-double-decrement guarantee as a `rule`
    + a `feature`.

### C. `clinicore` — clinic / EHR appointments + audit
Patterns composed: **event_sourcing + cqrs + sidecar + strangler_fig**.
- Story: book appointments and keep a tamper-evident clinical audit trail
  while strangling a legacy scheduler.
- Modules:
  - `context.pds` — `person Patient`, `person Clinician`, `system Clinicore`,
    external `system LegacyScheduler`.
  - `shared.pds` — `Appointment`, `ClinicalEvent`, `AuditEntry`.
  - `scheduling.pds` — **strangler_fig** facade routing some traffic to
    `LegacyScheduler`, the rest to the new service.
  - `records.pds` — **event_sourcing** of the clinical record (append-only
    `ClinicalEvent`) with a **cqrs** read projection for the patient
    timeline.
  - `compliance.pds` — a **sidecar** mirroring every state change to an
    immutable audit log (cross-cutting, deployed alongside each service).

### D. (optional 4th) `streamwallet` — fintech wallet / ledger
Patterns composed: **event_sourcing + saga + idempotent_receiver + bulkhead + circuit_breaker**.
- Story: a wallet ledger that must never lose or double-apply a movement.
- Modules:
  - `context.pds` — `person AccountHolder`, `person RiskAnalyst`,
    `system StreamWallet`, external bank/PSP rails.
  - `shared.pds` — `Movement`, `Balance`, `Transfer`.
  - `ledger.pds` — **event_sourcing** of balance movements (source of
    truth); rules: append-only, no negative balance.
  - `transfers.pds` — money-movement **saga** (debit → credit → settle
    with compensation), **idempotent_receiver** on inbound transfers.
  - `gateway.pds` — **bulkhead** isolating each rail into its own pool,
    **circuit_breaker** per downstream rail.

Ship **A, B, C** as the core three; **D** is the stretch fourth (it reuses
`event_sourcing`/`saga`, which A and C already cover — add only if a
fourth flagship is wanted and the overlap is acceptable).

## Per-example deliverable checklist

For each workspace `<name>/` under `web-ide/src/lib/samples/`:

- [ ] 4–5 `.pds` files at the workspace root (flat), authored via the
      **`pseudocode` skill**. Recommended split: `context.pds` (persons +
      top system + external systems), `shared.pds` (shared `data`), one
      `.pds` per bounded context. Each context file: `public container …
      for <System>` with the pattern's methods/`rule`s, FQN calls into
      sibling files, and ≥1 `feature … for …` with `given/when/then/but`.
      Target ~40–90 lines per file (acme modules are in that band).
- [ ] `pds.toml` — a `[doc]` table (`name`, `out = "target/doc"`,
      optional `logo`, `theme`) + `[[doc.sidebar]]` groups pointing at the
      `docs/*.md` below. Mirror `acme-tickets/pds.toml`.
- [ ] `meta.json` — `{ name, category: "Application", order, landing,
      description }`. `landing` = the `.pds` basename whose diagram opens
      first (e.g. `"context"`). `order`: A=2, B=3, C=4, D=5 (acme keeps 1).
- [ ] `docs/overview.md` — the business + where to start.
- [ ] `docs/contexts.md` — the bounded-context map (module table + how
      they fit + a **"Patterns in play"** list naming each composed
      pattern and the module that hosts it).
- [ ] 1–2 `docs/<flow>.md` deep dives — walk one composed flow each
      (e.g. `the-saga.md`, `read-models.md`), matching acme's
      `checkout-saga.md` / `surge.md` style.
- [ ] `docs/edge-cases.md` — failure modes & resilience (acme has one).
- [ ] `logo.svg` — optional; copy/recolour acme's or omit and drop
      `logo` from `pds.toml`.
- [ ] No build/registration step — the Vite glob discovers the folder
      automatically once `meta.json` is present.
- [ ] `pds doc .` over the workspace resolves with no diagnostics; docs
      render.

Authoring effort per example: **M** — 4–5 `.pds` + ~4 docs + 2 metadata
files. Roughly half a focused session each once the pattern→module map
above is fixed.

## Open questions / decisions needed

- **Three or four?** Recommend three core (A/B/C). D only if a 4th
  flagship is explicitly wanted (it reuses event_sourcing/saga).
- **Build/loader command.** RESOLVED — `samples.js:13-55` is a live Vite
  `import.meta.glob`; no codegen, no manifest. Nothing to run after adding
  a folder. The `SAMPLES` map (`samples.js:58-71`) reads `meta.name`,
  `meta.description`, `meta.category`, `meta.landing`, `meta.order` with
  sensible fallbacks (`category` → `"Examples"`, `order` → `999`), so the
  five `meta.json` fields acme uses are the full supported set.
- **`.code-workspace` and `modules/`** — the earlier draft invented both.
  acme-tickets has **neither**; do not add them.
- **No imports — FQN linkage.** Cross-file references are FQN calls
  (`System::Container.method(...)`), not `import`/`export`. Decide each
  workspace's top-level `system` name and per-module container names up
  front so FQNs resolve.
- **Distinct `category`?** All flagships could share `category:
  "Application"` (acme does) or get distinct labels. Recommend keeping
  `"Application"` so they cluster together at the top of the list.
- **Reuse vs. duplicate pattern shapes.** Lift the *modelling shape* from
  the single-pattern samples (e.g. saga's coordinator + reverse
  compensation), but re-author in the business domain. Do not reference
  the sample files. Confirm that's the intent.
- **`PATTERNS.md` is currently sparse** — if these workspaces should
  cross-link to a canonical pattern writeup, that writeup may not yet
  exist. Soft dependency, not a blocker.

## Dependencies on other tasks

- **`pseudocode` skill** (mandatory) — governs authoring every `.pds`
  file so syntax/idioms match the spec (`public data/person/system/
  container`, `feature`, FQN calls, `Result<T,E>`).
- **`spec-style` skill** — applies only if any doc lands under
  `CONFORMANCE/` / `LANG.md` / `PATTERNS.md`. The per-workspace `docs/*.md`
  are sample content, outside that gate — but keep the terse voice and the
  narrative register acme's docs use.
- **T7 / T6 (sharing)** — sharing/exporting these workspaces depends on
  the share mechanism from those tasks; this task only authors content.
  No hard blocker: samples are usable in-IDE the moment they're scanned.
- **Model resolver / `pds doc`** — acceptance needs the resolver to handle
  a multi-file workspace with cross-file FQN calls; acme exercises this
  today, so the capability exists.

## Acceptance criteria

Per example:
- [ ] `pds doc .` over the workspace resolves every file and every
      cross-file FQN reference with **zero diagnostics**.
- [ ] Every FQN call resolves to a `public` declaration in some file of
      the workspace.
- [ ] Each composed pattern is visible in the model (a named
      `container`/`component`/`rule`/`feature`) **and** listed in
      `docs/contexts.md` "Patterns in play".
- [ ] At least one `feature`/scenario per workspace renders in the docs
      view.
- [ ] `meta.json` validates; the workspace appears in the IDE sample list
      under `category: "Application"` at the chosen `order`; its `landing`
      diagram opens.
- [ ] Every `docs/*.md` referenced from `pds.toml`'s `[[doc.sidebar]]`
      exists and renders.

## Rough size + parallel-safe?

| Example | Size | Notes |
| --- | --- | --- |
| A `rideroute` | M | saga+outbox+circuit_breaker+rate_limiter+cqrs; 5 files |
| B `mercato` | M | outbox+idempotent_receiver+saga+bff+api_gateway; 5 files |
| C `clinicore` | M | event_sourcing+cqrs+sidecar+strangler_fig; 5 files |
| D `streamwallet` (stretch) | M | es+saga+idempotent_receiver+bulkhead+cb; 5 files |

**Parallel-safe: yes, fully.** Each workspace is a self-contained
directory under `samples/`; nothing is shared, and the Vite glob means
there is no generated index to merge or regenerate. Three or four authors
can land in parallel with zero coordination. Whole task ≈ **L** (3–4 × M).
